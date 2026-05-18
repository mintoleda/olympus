use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};

use crate::{
    persistence::save_sessions,
    state::{
        now_ms, ChatMessage, ContentPart, PiSession, SessionEvent, SessionStore, SessionUpdateEvent,
    },
};

pub(crate) fn emit_message(app: &AppHandle, session_id: &str, message: ChatMessage) {
    let _ = app.emit(
        "pi://message",
        SessionEvent {
            session_id: session_id.to_string(),
            message,
        },
    );
}

pub(crate) fn emit_session_update(app: &AppHandle, session: PiSession) {
    let _ = app.emit("pi://session", SessionUpdateEvent { session });
}

pub(crate) fn mark_status(app: &AppHandle, session_id: &str, status: &str) {
    let store = app.state::<SessionStore>();
    if let Ok(mut sessions) = store.sessions.lock() {
        if let Some(session) = sessions.get_mut(session_id) {
            session.status = status.into();
        }
        let _ = save_sessions(app, &sessions);
    }
    emit_message(
        app,
        session_id,
        ChatMessage::text(format!("{session_id}-status-{}", now_ms()), "status", status),
    );
}

pub(crate) fn append_assistant_delta(
    app: &AppHandle,
    session_id: &str,
    message_id: &str,
    delta: &str,
) {
    emit_message(
        app,
        session_id,
        ChatMessage::text(message_id, "assistant", delta),
    );
}

pub(crate) fn append_thinking_delta(
    app: &AppHandle,
    session_id: &str,
    message_id: &str,
    delta: &str,
) {
    emit_message(
        app,
        session_id,
        ChatMessage::typed(message_id, "assistant", delta, "thinking"),
    );
}

fn push_persisted(app: &AppHandle, session_id: &str, message: ChatMessage) {
    let store = app.state::<SessionStore>();
    if let Ok(mut sessions) = store.sessions.lock() {
        if let Some(session) = sessions.get_mut(session_id) {
            if let Some(existing) =
                session.messages.iter_mut().find(|m| m.id == message.id)
            {
                *existing = message.clone();
            } else {
                session.messages.push(message.clone());
            }
        }
        let _ = save_sessions(app, &sessions);
    }
    drop(store);
    emit_message(app, session_id, message);
}

/// Defensive fallback path used when Pi does not emit `message_end`.
/// Persists the streamed text as an assistant message.
pub(crate) fn finalize_assistant(
    app: &AppHandle,
    session_id: &str,
    message_id: &str,
    content: String,
) {
    if content.trim().is_empty() {
        return;
    }
    let mut message = ChatMessage::text(message_id, "assistant", content.clone());
    message.content_parts = vec![ContentPart::Text { text: content }];
    push_persisted(app, session_id, message);
}

pub(crate) fn finalize_thinking(
    app: &AppHandle,
    session_id: &str,
    message_id: &str,
    content: String,
) {
    if content.trim().is_empty() {
        return;
    }
    let message = ChatMessage::typed(message_id, "assistant", content, "thinking");
    push_persisted(app, session_id, message);
}

/// Canonical persistence path: build a ChatMessage from Pi's `message_end.message`
/// payload, preserving content parts, custom types, and tool calls/results.
pub(crate) fn finalize_message_end(
    app: &AppHandle,
    session_id: &str,
    message: &Value,
    streamed_message_id: &str,
    streamed_thinking_id: &str,
    streamed_thinking_buffer: &str,
) {
    let (thinking, main) = build_messages_from_pi(session_id, message);

    if let Some(mut thinking_msg) = thinking {
        if streamed_thinking_id.is_empty() && thinking_msg.id.starts_with(session_id) {
            // No streamed id and no Pi id — keep the generated one.
        } else if !streamed_thinking_id.is_empty() {
            thinking_msg.id = streamed_thinking_id.to_string();
        }
        push_persisted(app, session_id, thinking_msg);
    } else if !streamed_thinking_id.is_empty() && !streamed_thinking_buffer.trim().is_empty() {
        // Pi delivered thinking via deltas but not in the canonical content array.
        push_persisted(
            app,
            session_id,
            ChatMessage::typed(
                streamed_thinking_id,
                "assistant",
                streamed_thinking_buffer.to_string(),
                "thinking",
            ),
        );
    }

    if let Some(mut main_msg) = main {
        // Prefer the streamed ID so the frontend can replace the in-progress message.
        // build_messages_from_pi always returns a non-empty id (Pi's or a fallback),
        // so the old `is_empty()` guard was never triggered — hence the duplication.
        if !streamed_message_id.is_empty() {
            main_msg.id = streamed_message_id.to_string();
        }
        push_persisted(app, session_id, main_msg);
    }
}

/// Build (optional thinking ChatMessage, optional main ChatMessage) from Pi's
/// canonical message object. Returns (None, None) for tool-only/no-output turns.
pub(crate) fn build_messages_from_pi(
    session_id: &str,
    message: &Value,
) -> (Option<ChatMessage>, Option<ChatMessage>) {
    let pi_id = message
        .get("id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_default();

    let role = message
        .get("role")
        .and_then(Value::as_str)
        .unwrap_or("assistant")
        .to_string();

    let (parts, thinking_text, flat_text) = parse_content(message.get("content"));

    let custom_type = message
        .get("customType")
        .or_else(|| message.get("custom_type"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let details = message.get("details").cloned();
    let display = message.get("display").and_then(Value::as_bool);

    let thinking_msg = if !thinking_text.trim().is_empty() {
        let id = format!("{session_id}-th-{}", now_ms());
        Some(ChatMessage::typed(id, "assistant", thinking_text, "thinking"))
    } else {
        None
    };

    let has_non_thinking_part = parts
        .iter()
        .any(|p| !matches!(p, ContentPart::Thinking { .. }));
    let main_msg = if has_non_thinking_part || !flat_text.trim().is_empty() || custom_type.is_some()
    {
        let non_thinking_parts: Vec<ContentPart> = parts
            .into_iter()
            .filter(|p| !matches!(p, ContentPart::Thinking { .. }))
            .collect();
        let id = if pi_id.is_empty() {
            format!("{session_id}-a-{}", now_ms())
        } else {
            pi_id
        };
        let mut msg = ChatMessage::text(id, role.as_str(), flat_text);
        msg.content_parts = non_thinking_parts;
        msg.custom_type = custom_type;
        msg.details = details;
        msg.display = display;
        Some(msg)
    } else {
        None
    };

    (thinking_msg, main_msg)
}

/// Replace a session's cached transcript with messages parsed from Pi's
/// `get_messages` response payload.
pub(crate) fn replace_session_transcript(
    app: &AppHandle,
    session_id: &str,
    messages: &Value,
) {
    let Some(items) = messages.as_array() else {
        return;
    };

    let mut rebuilt: Vec<ChatMessage> = Vec::new();
    for entry in items {
        // Pi may wrap as { "message": {...} } (matches message_end shape) or emit
        // the message object directly. Tolerate both.
        let inner = entry.get("message").unwrap_or(entry);
        let role = inner
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or("");
        if role == "user" {
            let text = inner
                .get("content")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| {
                    inner
                        .get("content")
                        .and_then(Value::as_array)
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|p| {
                                    if p.get("type").and_then(Value::as_str) == Some("text") {
                                        p.get("text").and_then(Value::as_str).map(str::to_string)
                                    } else {
                                        None
                                    }
                                })
                                .collect::<Vec<_>>()
                                .join("\n\n")
                        })
                })
                .unwrap_or_default();
            if text.trim().is_empty() {
                continue;
            }
            let id = inner
                .get("id")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| format!("{session_id}-u-{}", rebuilt.len()));
            let mut msg = ChatMessage::text(id, "user", text);
            if let Some(ts) = inner.get("timestamp").and_then(Value::as_u64) {
                msg.timestamp = ts;
            }
            rebuilt.push(msg);
        } else {
            let (thinking, main) = build_messages_from_pi(session_id, inner);
            if let Some(t) = thinking {
                rebuilt.push(t);
            }
            if let Some(m) = main {
                rebuilt.push(m);
            }
        }
    }

    let store = app.state::<SessionStore>();
    let mut updated_session = None;
    if let Ok(mut sessions) = store.sessions.lock() {
        if let Some(session) = sessions.get_mut(session_id) {
            session.messages = rebuilt;
            updated_session = Some(session.clone());
        }
        let _ = save_sessions(app, &sessions);
    }
    drop(store);
    if let Some(session) = updated_session {
        emit_session_update(app, session);
    }
}

/// Parse Pi's `message.content` into (parts, thinking text, flat text).
fn parse_content(content: Option<&Value>) -> (Vec<ContentPart>, String, String) {
    let mut parts: Vec<ContentPart> = Vec::new();
    let mut thinking_text = String::new();
    let mut flat_text = String::new();

    let Some(content) = content else {
        return (parts, thinking_text, flat_text);
    };

    if let Some(text) = content.as_str() {
        flat_text.push_str(text);
        parts.push(ContentPart::Text {
            text: text.to_string(),
        });
        return (parts, thinking_text, flat_text);
    }

    let Some(arr) = content.as_array() else {
        return (parts, thinking_text, flat_text);
    };

    for part in arr {
        let kind = part.get("type").and_then(Value::as_str).unwrap_or("");
        match kind {
            "text" => {
                let text = part
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                if !flat_text.is_empty() {
                    flat_text.push_str("\n\n");
                }
                flat_text.push_str(&text);
                parts.push(ContentPart::Text { text });
            }
            "thinking" | "thinking_block" => {
                let text = part
                    .get("thinking")
                    .or_else(|| part.get("text"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                if !thinking_text.is_empty() {
                    thinking_text.push_str("\n\n");
                }
                thinking_text.push_str(&text);
                parts.push(ContentPart::Thinking { text });
            }
            "tool_use" => {
                let id = part
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let name = part
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("tool")
                    .to_string();
                let input = part.get("input").cloned().unwrap_or(Value::Null);
                parts.push(ContentPart::ToolUse { id, name, input });
            }
            "tool_result" => {
                let tool_use_id = part
                    .get("tool_use_id")
                    .or_else(|| part.get("toolUseId"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let result_content = part.get("content").cloned().unwrap_or(Value::Null);
                let is_error = part.get("is_error").and_then(Value::as_bool);
                parts.push(ContentPart::ToolResult {
                    tool_use_id,
                    content: result_content,
                    is_error,
                });
            }
            "custom" => {
                let custom_type = part
                    .get("customType")
                    .or_else(|| part.get("custom_type"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let details = part.get("details").cloned().unwrap_or(Value::Null);
                parts.push(ContentPart::Custom {
                    custom_type,
                    details,
                });
            }
            _ => {}
        }
    }

    (parts, thinking_text, flat_text)
}
