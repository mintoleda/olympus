use tauri::{AppHandle, Emitter, Manager};

use crate::{
    persistence::save_sessions,
    state::{now_ms, ChatMessage, PiSession, SessionEvent, SessionStore, SessionUpdateEvent},
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
        ChatMessage {
            id: format!("{session_id}-status-{}", now_ms()),
            role: "status".into(),
            content: status.into(),
            timestamp: now_ms(),
            msg_type: None,
        },
    );
}

pub(crate) fn append_assistant_delta(app: &AppHandle, session_id: &str, message_id: &str, delta: &str) {
    emit_message(
        app,
        session_id,
        ChatMessage {
            id: message_id.into(),
            role: "assistant".into(),
            content: delta.into(),
            timestamp: now_ms(),
            msg_type: None,
        },
    );
}

pub(crate) fn append_thinking_delta(app: &AppHandle, session_id: &str, message_id: &str, delta: &str) {
    emit_message(
        app,
        session_id,
        ChatMessage {
            id: message_id.into(),
            role: "assistant".into(),
            content: delta.into(),
            timestamp: now_ms(),
            msg_type: Some("thinking".into()),
        },
    );
}

pub(crate) fn finalize_assistant(app: &AppHandle, session_id: &str, message_id: &str, content: String) {
    let store = app.state::<SessionStore>();
    if let Ok(mut sessions) = store.sessions.lock() {
        if let Some(session) = sessions.get_mut(session_id) {
            session.messages.push(ChatMessage {
                id: message_id.into(),
                role: "assistant".into(),
                content: if content.trim().is_empty() {
                    "Pi returned no output.".into()
                } else {
                    content
                },
                timestamp: now_ms(),
                msg_type: None,
            });
            session.status = "idle".into();
        }
        let _ = save_sessions(app, &sessions);
    }
    emit_message(
        app,
        session_id,
        ChatMessage {
            id: format!("{session_id}-done-{}", now_ms()),
            role: "status".into(),
            content: "idle".into(),
            timestamp: now_ms(),
            msg_type: None,
        },
    );
}

pub(crate) fn finalize_thinking(app: &AppHandle, session_id: &str, message_id: &str, content: String) {
    if content.trim().is_empty() {
        return;
    }
    {
        let store = app.state::<SessionStore>();
        if let Ok(mut sessions) = store.sessions.lock() {
            if let Some(session) = sessions.get_mut(session_id) {
                session.messages.push(ChatMessage {
                    id: message_id.into(),
                    role: "assistant".into(),
                    content,
                    timestamp: now_ms(),
                    msg_type: Some("thinking".into()),
                });
            }
            let _ = save_sessions(app, &sessions);
        };
    }
}
