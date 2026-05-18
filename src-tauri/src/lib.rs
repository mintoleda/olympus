mod commands;
mod persistence;
mod pi_events;
mod pi_import;
mod state;

use persistence::{load_sessions, persist_store, save_sessions};
use pi_events::{
    append_assistant_delta, append_thinking_delta, emit_message, emit_session_update,
    finalize_assistant, finalize_message_end, finalize_thinking, mark_status,
    replace_session_transcript,
};
use serde_json::Value;
use state::{
    now_ms, ChatMessage, EditorTextEvent, ExtensionUiRequest, NotifyEvent, PiCommandOption,
    PiModelOption, PiSession, RunningSession, SessionStore, StatusEntry, StatusEvent, TitleEvent,
    WidgetEntry, WidgetEvent,
};
use std::{
    io::{BufRead, BufReader, Write},
    process::{ChildStdin, Command, Stdio},
    sync::{Arc, Mutex},
    thread,
};
use tauri::{AppHandle, Emitter, Manager};

/// Olympus-local slash commands. Each is handled entirely in the frontend
/// by `handleSlashCommand` in `App.svelte` — they never reach Pi.
/// Pi-side commands (`/fork`, `/tree`, `/resume`, `/login`, `/logout`, …)
/// are populated dynamically via `get_commands` and forwarded to Pi as prompts.
const BUILTIN_COMMANDS: &[(&str, &str)] = &[
    ("model", "Switch model"),
    ("scoped-models", "Enable/disable models for cycling"),
    ("settings", "Open Olympus settings"),
    ("hotkeys", "Show keyboard shortcuts"),
    ("new", "Start a new Olympus session"),
    ("clear", "Reset the current Pi session"),
    ("compact", "Compact session context"),
    ("name", "Set Olympus session display name"),
    ("session", "Show Olympus session info"),
    ("stop", "Stop active Pi runtime and mark idle"),
    ("quit", "Close the current Olympus session"),
];

fn value_string(value: Option<&Value>) -> Option<String> {
    value.and_then(|value| match value {
        Value::String(text) if !text.trim().is_empty() => Some(text.to_string()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    })
}

fn model_label(data: &Value) -> Option<String> {
    if let Some(model) = value_string(data.get("model").or_else(|| data.pointer("/config/model"))) {
        return Some(model);
    }

    let model = data
        .get("model")
        .or_else(|| data.pointer("/config/model"))?;
    value_string(model.get("name"))
        .or_else(|| value_string(model.get("id")))
        .or_else(|| value_string(model.get("model")))
}

fn model_id_label(data: &Value) -> Option<String> {
    if let Some(model) = value_string(data.get("model").or_else(|| data.pointer("/config/model"))) {
        return Some(model);
    }

    let model = data
        .get("model")
        .or_else(|| data.pointer("/config/model"))?;
    value_string(model.get("id"))
        .or_else(|| value_string(model.get("model")))
        .or_else(|| value_string(model.get("name")))
}

fn provider_label(data: &Value) -> Option<String> {
    value_string(
        data.get("provider")
            .or_else(|| data.pointer("/config/provider")),
    )
    .or_else(|| value_string(data.pointer("/model/provider")))
    .or_else(|| value_string(data.pointer("/config/model/provider")))
    .or_else(|| value_string(data.pointer("/model/api")))
}

fn emit_updated_session(app: &AppHandle, session: Option<PiSession>) {
    if let Some(session) = session {
        emit_session_update(app, session);
    }
}

fn handle_set_model_response(app: &AppHandle, session_id: &str, data: &Value) {
    let provider = value_string(data.get("provider"));
    let model = value_string(data.get("name")).or_else(|| value_string(data.get("id")));
    let model_id = value_string(data.get("id")).or_else(|| value_string(data.get("name")));

    let store = app.state::<SessionStore>();
    let mut updated_session = None;
    if let Ok(mut sessions) = store.sessions.lock() {
        if let Some(session) = sessions.get_mut(session_id) {
            if let Some(provider) = provider {
                session.provider = Some(provider);
            }
            if let Some(model) = model {
                session.model = Some(model);
            }
            if let Some(model_id) = model_id {
                session.model_id = Some(model_id);
            }
            if session.status == "updating" || session.status == "starting" {
                session.status = "idle".into();
            }
            updated_session = Some(session.clone());
        }
        let _ = save_sessions(app, &sessions);
    }
    emit_updated_session(app, updated_session);
}

fn parse_commands(event: &Value) -> Vec<PiCommandOption> {
    let Some(items) = event.pointer("/data/commands").and_then(Value::as_array) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let name = item.get("name").and_then(Value::as_str)?.to_string();
            Some(PiCommandOption {
                name,
                description: item
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                source: item
                    .get("source")
                    .and_then(Value::as_str)
                    .unwrap_or("extension")
                    .to_string(),
                location: item
                    .get("location")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                path: item
                    .get("path")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            })
        })
        .collect()
}

fn handle_commands_response(app: &AppHandle, session_id: &str, event: &Value) {
    let commands = parse_commands(event);
    let store = app.state::<SessionStore>();
    if let Ok(mut cache) = store.last_commands.lock() {
        cache.insert(session_id.to_string(), commands.clone());
    }
    let request_id = event.get("id").and_then(Value::as_str).map(str::to_string);
    if let Some(request_id) = request_id {
        let sender = store
            .pending_commands
            .lock()
            .ok()
            .and_then(|mut map| map.remove(&request_id));
        if let Some(sender) = sender {
            let _ = sender.try_send(commands);
        }
    }
}

fn parse_models(event: &Value) -> Vec<PiModelOption> {
    let items = event
        .pointer("/data/models")
        .or_else(|| event.pointer("/data"))
        .and_then(Value::as_array);
    let Some(items) = items else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let provider = item
                .get("provider")
                .and_then(Value::as_str)?
                .to_string();
            let id = item
                .get("id")
                .or_else(|| item.get("modelId"))
                .or_else(|| item.get("name"))
                .and_then(Value::as_str)?
                .to_string();
            let context = item
                .get("context")
                .or_else(|| item.get("contextWindow"))
                .map(|v| {
                    v.as_str()
                        .map(str::to_string)
                        .unwrap_or_else(|| v.to_string())
                })
                .unwrap_or_default();
            let max_output = item
                .get("maxOutput")
                .or_else(|| item.get("max_output"))
                .map(|v| {
                    v.as_str()
                        .map(str::to_string)
                        .unwrap_or_else(|| v.to_string())
                })
                .unwrap_or_default();
            let reasoning = item
                .get("reasoning")
                .or_else(|| item.get("supportsThinking"))
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let images = item
                .get("images")
                .or_else(|| item.get("supportsImages"))
                .and_then(Value::as_bool)
                .unwrap_or(false);
            Some(PiModelOption {
                provider,
                id,
                context,
                max_output,
                reasoning,
                images,
            })
        })
        .collect()
}

fn handle_models_response(app: &AppHandle, event: &Value) {
    let models = parse_models(event);
    let request_id = event.get("id").and_then(Value::as_str).map(str::to_string);
    if let Some(request_id) = request_id {
        let store = app.state::<SessionStore>();
        let sender = store
            .pending_models
            .lock()
            .ok()
            .and_then(|mut map| map.remove(&request_id));
        if let Some(sender) = sender {
            let _ = sender.try_send(models);
        }
    }
}

pub(crate) fn merge_with_builtins(commands: Vec<PiCommandOption>) -> Vec<PiCommandOption> {
    let mut merged: Vec<PiCommandOption> = BUILTIN_COMMANDS
        .iter()
        .map(|(name, description)| PiCommandOption {
            name: (*name).to_string(),
            description: (*description).to_string(),
            source: "builtin".into(),
            location: None,
            path: None,
        })
        .collect();
    for command in commands {
        if let Some(existing) = merged.iter_mut().find(|item| item.name == command.name) {
            *existing = command;
        } else {
            merged.push(command);
        }
    }
    merged.sort_by(|a, b| a.name.cmp(&b.name));
    merged
}

pub(crate) fn emit_statuses(app: &AppHandle, session_id: &str, statuses: Vec<StatusEntry>) {
    let _ = app.emit(
        "pi://status",
        StatusEvent {
            session_id: session_id.to_string(),
            statuses,
        },
    );
}

pub(crate) fn emit_widgets(app: &AppHandle, session_id: &str, widgets: Vec<WidgetEntry>) {
    let _ = app.emit(
        "pi://widget",
        WidgetEvent {
            session_id: session_id.to_string(),
            widgets,
        },
    );
}

fn handle_set_status(app: &AppHandle, session_id: &str, event: &Value) {
    let key = match event.get("statusKey").and_then(Value::as_str) {
        Some(key) if !key.is_empty() => key.to_string(),
        _ => return,
    };
    let text = event
        .get("statusText")
        .and_then(Value::as_str)
        .map(str::to_string);
    let snapshot = {
        let store = app.state::<SessionStore>();
        let mut map = match store.session_statuses.lock() {
            Ok(map) => map,
            Err(_) => return,
        };
        let entries = map.entry(session_id.to_string()).or_default();
        let was_present = entries.iter().any(|e| e.key == key);
        entries.retain(|entry| entry.key != key);
        if let Some(ref text) = text {
            if !was_present {
                emit_message(
                    app,
                    session_id,
                    ChatMessage::typed(
                        format!("{session_id}-tool-{key}-{}", now_ms()),
                        "assistant",
                        format!("{key}: {text}"),
                        "tool",
                    ),
                );
            }
            entries.push(StatusEntry { key, text: text.clone() });
        }
        entries.clone()
    };
    emit_statuses(app, session_id, snapshot);
}

fn handle_set_widget(app: &AppHandle, session_id: &str, event: &Value) {
    let key = match event.get("widgetKey").and_then(Value::as_str) {
        Some(key) if !key.is_empty() => key.to_string(),
        _ => return,
    };
    let lines = event.get("widgetLines").and_then(Value::as_array).map(|arr| {
        arr.iter()
            .map(|line| line.as_str().unwrap_or("").to_string())
            .collect::<Vec<_>>()
    });
    let placement = event
        .get("widgetPlacement")
        .and_then(Value::as_str)
        .unwrap_or("aboveEditor")
        .to_string();
    let snapshot = {
        let store = app.state::<SessionStore>();
        let mut map = match store.session_widgets.lock() {
            Ok(map) => map,
            Err(_) => return,
        };
        let entries = map.entry(session_id.to_string()).or_default();
        entries.retain(|entry| entry.key != key);
        if let Some(lines) = lines {
            entries.push(WidgetEntry { key, lines, placement });
        }
        entries.clone()
    };
    emit_widgets(app, session_id, snapshot);
}

fn handle_notify(app: &AppHandle, session_id: &str, event: &Value) {
    let message = event
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if message.is_empty() {
        return;
    }
    let level = event
        .get("notifyType")
        .and_then(Value::as_str)
        .unwrap_or("info")
        .to_string();
    let _ = app.emit(
        "pi://notify",
        NotifyEvent {
            session_id: session_id.to_string(),
            message,
            level,
        },
    );
}

fn handle_set_title(app: &AppHandle, session_id: &str, event: &Value) {
    let title = event
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let _ = app.emit(
        "pi://title",
        TitleEvent {
            session_id: session_id.to_string(),
            title,
        },
    );
}

fn handle_set_editor_text(app: &AppHandle, session_id: &str, event: &Value) {
    let text = event
        .get("text")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let _ = app.emit(
        "pi://editor-text",
        EditorTextEvent {
            session_id: session_id.to_string(),
            text,
        },
    );
}

fn handle_state_response(app: &AppHandle, session_id: &str, data: &Value) {
    let pi_session_id = value_string(data.get("sessionId").or_else(|| data.get("session_id")));
    let pi_session_file =
        value_string(data.get("sessionFile").or_else(|| data.get("session_file")));
    let session_name = value_string(data.get("sessionName").or_else(|| data.get("session_name")));
    let model = model_label(data);
    let model_id = model_id_label(data);
    let provider = provider_label(data);
    let thinking_level = value_string(
        data.get("thinkingLevel")
            .or_else(|| data.get("thinking_level"))
            .or_else(|| data.pointer("/config/thinkingLevel"))
            .or_else(|| data.pointer("/config/thinking_level")),
    );

    let store = app.state::<SessionStore>();
    let mut updated_session = None;
    if let Ok(mut sessions) = store.sessions.lock() {
        if let Some(session) = sessions.get_mut(session_id) {
            if let Some(id) = pi_session_id {
                session.pi_session_id = Some(id);
            }
            if let Some(file) = pi_session_file {
                session.pi_session_file = Some(file);
            }
            if let Some(name) = session_name {
                session.name = name;
            }
            if let Some(model) = model {
                session.model = Some(model);
            }
            if let Some(model_id) = model_id {
                session.model_id = Some(model_id);
            }
            if let Some(provider) = provider {
                session.provider = Some(provider);
            }
            if let Some(thinking_level) = thinking_level {
                session.thinking_level = Some(thinking_level);
            }
            if session.status == "starting" {
                session.status = "idle".into();
            }
            updated_session = Some(session.clone());
        }
        let _ = save_sessions(app, &sessions);
    };
    emit_updated_session(app, updated_session);
}

pub(crate) fn write_rpc(runtime: &RunningSession, request: Value) -> Result<(), String> {
    let mut stdin = runtime.stdin.lock().map_err(|_| "Pi stdin lock poisoned")?;
    writeln!(stdin, "{request}").map_err(|err| err.to_string())?;
    stdin.flush().map_err(|err| err.to_string())
}

/// Send `session_shutdown` and give Pi a bounded window to flush extension
/// hooks before SIGKILLing. Returns once the child has exited (cleanly or via kill).
pub(crate) fn graceful_shutdown(
    runtime: &RunningSession,
    session_id: &str,
    timeout: std::time::Duration,
) {
    let _ = write_rpc(
        runtime,
        serde_json::json!({
            "id": format!("{session_id}-shutdown-{}", now_ms()),
            "type": "session_shutdown",
        }),
    );

    let deadline = std::time::Instant::now() + timeout;
    if let Ok(mut child) = runtime.child.lock() {
        loop {
            match child.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) => {
                    if std::time::Instant::now() >= deadline {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                Err(_) => break,
            }
        }
        let _ = child.kill();
        let _ = child.wait();
    }
}

fn write_get_state_via_stdin(stdin: &Arc<Mutex<ChildStdin>>, session_id: &str) {
    if let Ok(mut guard) = stdin.lock() {
        let msg = serde_json::json!({"id": format!("{session_id}-state-{}", now_ms()), "type": "get_state"});
        let _ = writeln!(guard, "{msg}");
        let _ = guard.flush();
    }
}

fn request_pi_state(runtime: &RunningSession, session_id: &str) -> Result<(), String> {
    write_rpc(
        runtime,
        serde_json::json!({"id": format!("{session_id}-state-{}", now_ms()), "type": "get_state"}),
    )
}

fn request_pi_messages(runtime: &RunningSession, session_id: &str) -> Result<(), String> {
    write_rpc(
        runtime,
        serde_json::json!({"id": format!("{session_id}-msgs-{}", now_ms()), "type": "get_messages"}),
    )
}

pub(crate) fn spawn_pi(app: AppHandle, session_id: String) -> Result<RunningSession, String> {
    spawn_pi_inner(app, session_id, true)
}

pub(crate) fn spawn_pi_inner(
    app: AppHandle,
    session_id: String,
    request_initial_state: bool,
) -> Result<RunningSession, String> {
    let store = app.state::<SessionStore>();
    if let Some(existing) = store
        .runtimes
        .lock()
        .map_err(|_| "runtime store poisoned")?
        .get(&session_id)
        .cloned()
    {
        if request_initial_state {
            request_pi_state(&existing, &session_id)?;
        }
        return Ok(existing);
    }

    let (project_path, resume_target) = {
        let mut sessions = store
            .sessions
            .lock()
            .map_err(|_| "session store poisoned")?;
        let session = sessions.get_mut(&session_id).ok_or("session not found")?;
        session.status = "starting".into();
        (
            session.project_path.clone(),
            session
                .pi_session_file
                .clone()
                .or_else(|| session.pi_session_id.clone()),
        )
    };

    let mut command = Command::new("pi");
    command.current_dir(&project_path).arg("--mode").arg("rpc");
    if let Some(target) = resume_target {
        command.arg("--session").arg(target);
    }
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|err| format!("Could not start Pi RPC: {err}"))?;
    let stdin = child.stdin.take().ok_or("Pi stdin unavailable")?;
    let stdout = child.stdout.take().ok_or("Pi stdout unavailable")?;
    let stderr = child.stderr.take();

    let runtime = RunningSession {
        child: Arc::new(Mutex::new(child)),
        stdin: Arc::new(Mutex::new(stdin)),
    };

    store
        .runtimes
        .lock()
        .map_err(|_| "runtime store poisoned")?
        .insert(session_id.clone(), runtime.clone());
    persist_store(&app, &store);

    let reader_app = app.clone();
    let reader_session_id = session_id.clone();
    let reader_stdin = runtime.stdin.clone();
    thread::spawn(move || {
        let mut current_message_id = String::new();
        let mut full_response = String::new();
        let mut current_thinking_id = String::new();
        let mut thinking_response = String::new();
        let mut message_persisted = false;
        let reader = BufReader::new(stdout);

        for line in reader.lines().map_while(Result::ok) {
            let Ok(event) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            match event.get("type").and_then(Value::as_str) {
                Some("response") => {
                    let command = event.get("command").and_then(Value::as_str);
                    let success = event.get("success").and_then(Value::as_bool).unwrap_or(false);
                    match command {
                        Some("get_state") => {
                            if let Some(data) = event.get("data") {
                                handle_state_response(&reader_app, &reader_session_id, data);
                            }
                        }
                        Some("get_messages") if success => {
                            if let Some(data) = event.get("data") {
                                let messages = data.get("messages").unwrap_or(data);
                                replace_session_transcript(
                                    &reader_app,
                                    &reader_session_id,
                                    messages,
                                );
                            }
                        }
                        Some("get_messages") => {
                            // Pi doesn't support get_messages — keep cached transcript.
                        }
                        Some("set_model") if success => {
                            if let Some(data) = event.get("data") {
                                handle_set_model_response(&reader_app, &reader_session_id, data);
                            }
                        }
                        Some("get_commands") if success => {
                            handle_commands_response(&reader_app, &reader_session_id, &event);
                        }
                        Some("list_models") if success => {
                            handle_models_response(&reader_app, &event);
                        }
                        Some("list_models") => {
                            // Pi doesn't support list_models RPC — drop the pending sender so
                            // the caller can fall back to the CLI parser.
                            if let Some(request_id) =
                                event.get("id").and_then(Value::as_str).map(str::to_string)
                            {
                                let store = reader_app.state::<SessionStore>();
                                if let Ok(mut map) = store.pending_models.lock() {
                                    map.remove(&request_id);
                                };
                                drop(store);
                            }
                        }
                        Some("new_session") if success => {
                            // Pi created a new session; clean up stream state, add separator, sync state
                            current_message_id.clear();
                            full_response.clear();
                            current_thinking_id.clear();
                            thinking_response.clear();
                            emit_message(
                                &reader_app,
                                &reader_session_id,
                                ChatMessage::typed(
                                    format!("{reader_session_id}-reset-{}", now_ms()),
                                    "system",
                                    "── session reset ──",
                                    "separator",
                                ),
                            );
                            mark_status(&reader_app, &reader_session_id, "idle");
                            write_get_state_via_stdin(&reader_stdin, &reader_session_id);
                        }
                        Some("compact") if success => {
                            mark_status(&reader_app, &reader_session_id, "idle");
                            write_get_state_via_stdin(&reader_stdin, &reader_session_id);
                        }
                        _ if !success => {
                            let text = event
                                .get("error")
                                .map(Value::to_string)
                                .unwrap_or_else(|| "Pi rejected command".into());
                            emit_message(
                                &reader_app,
                                &reader_session_id,
                                ChatMessage::text(
                                    format!("{reader_session_id}-err-{}", now_ms()),
                                    "assistant",
                                    text,
                                ),
                            );
                            mark_status(&reader_app, &reader_session_id, "idle");
                        }
                        _ => {}
                    }
                }
                Some("extension_ui_request") => {
                    let method = event
                        .get("method")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    match method {
                        "select" | "confirm" | "input" | "editor" | "custom" => {
                            let mut request = event.clone();
                            if let Some(object) = request.as_object_mut() {
                                object.remove("type");
                            }
                            let _ = reader_app.emit(
                                "pi://extension-ui-request",
                                ExtensionUiRequest {
                                    session_id: reader_session_id.clone(),
                                    request,
                                },
                            );
                        }
                        "setStatus" => {
                            handle_set_status(&reader_app, &reader_session_id, &event);
                        }
                        "setWidget" => {
                            handle_set_widget(&reader_app, &reader_session_id, &event);
                        }
                        "notify" => {
                            handle_notify(&reader_app, &reader_session_id, &event);
                        }
                        "setTitle" => {
                            handle_set_title(&reader_app, &reader_session_id, &event);
                        }
                        "set_editor_text" => {
                            handle_set_editor_text(&reader_app, &reader_session_id, &event);
                        }
                        _ => {}
                    }
                }
                Some("extension_error") => {
                    let msg = event
                        .get("error")
                        .and_then(Value::as_str)
                        .unwrap_or("Pi extension error");
                    emit_message(
                        &reader_app,
                        &reader_session_id,
                        ChatMessage::text(
                            format!("{reader_session_id}-ext-err-{}", now_ms()),
                            "assistant",
                            format!("Extension error: {msg}"),
                        ),
                    );
                    mark_status(&reader_app, &reader_session_id, "idle");
                }
                Some("compaction_start") => {
                    mark_status(&reader_app, &reader_session_id, "compacting");
                }
                Some("compaction_end") => {
                    current_message_id.clear();
                    full_response.clear();
                    mark_status(&reader_app, &reader_session_id, "idle");
                    write_get_state_via_stdin(&reader_stdin, &reader_session_id);
                }
                Some("tool_execution_start") => {
                    let tool_name = event
                        .get("toolName")
                        .or_else(|| event.get("tool_name"))
                        .and_then(Value::as_str)
                        .unwrap_or("tool");
                    mark_status(&reader_app, &reader_session_id, &format!("running:{tool_name}"));
                }
                Some("tool_execution_end") => {
                    mark_status(&reader_app, &reader_session_id, "streaming");
                }
                Some("auto_retry_start") => {
                    mark_status(&reader_app, &reader_session_id, "retrying");
                }
                Some("auto_retry_end") => {
                    mark_status(&reader_app, &reader_session_id, "streaming");
                }
                Some("agent_start") => {
                    current_message_id.clear();
                    full_response.clear();
                    current_thinking_id.clear();
                    thinking_response.clear();
                    message_persisted = false;
                    mark_status(&reader_app, &reader_session_id, "streaming");
                }
                Some("message_start") => {
                    let pi_id = event
                        .pointer("/message/id")
                        .and_then(Value::as_str)
                        .or_else(|| event.get("messageId").and_then(Value::as_str))
                        .or_else(|| event.get("id").and_then(Value::as_str))
                        .map(str::to_string)
                        .unwrap_or_else(|| format!("{reader_session_id}-a-{}", now_ms()));
                    current_message_id = pi_id;
                    full_response.clear();
                    message_persisted = false;
                }
                Some("message_update") => {
                    let delta_event = &event["assistantMessageEvent"];
                    match delta_event.get("type").and_then(Value::as_str) {
                        Some("thinking_delta" | "thinking_block_delta") => {
                            let thinking = delta_event
                                .get("delta")
                                .and_then(|d| {
                                    d.as_str()
                                        .or_else(|| d.get("thinking").and_then(Value::as_str))
                                })
                                .unwrap_or("");
                            if !thinking.is_empty() {
                                if current_thinking_id.is_empty() {
                                    current_thinking_id =
                                        format!("{reader_session_id}-th-{}", now_ms());
                                    mark_status(&reader_app, &reader_session_id, "thinking");
                                }
                                thinking_response.push_str(thinking);
                                append_thinking_delta(
                                    &reader_app,
                                    &reader_session_id,
                                    &current_thinking_id,
                                    thinking,
                                );
                            }
                        }
                        Some("text_delta") => {
                            if let Some(delta) =
                                delta_event.get("delta").and_then(Value::as_str)
                            {
                                if current_message_id.is_empty() {
                                    current_message_id =
                                        format!("{reader_session_id}-a-{}", now_ms());
                                }
                                if full_response.is_empty() {
                                    mark_status(
                                        &reader_app,
                                        &reader_session_id,
                                        "generating",
                                    );
                                }
                                full_response.push_str(delta);
                                append_assistant_delta(
                                    &reader_app,
                                    &reader_session_id,
                                    &current_message_id,
                                    delta,
                                );
                            }
                        }
                        Some("done") => {
                            // Streaming for this message is done; message_end will persist.
                            // No-op here to avoid double-finalize.
                        }
                        Some("error") => {
                            let msg = delta_event
                                .get("error")
                                .and_then(Value::as_str)
                                .unwrap_or("stream error");
                            emit_message(
                                &reader_app,
                                &reader_session_id,
                                ChatMessage::text(
                                    format!("{reader_session_id}-serr-{}", now_ms()),
                                    "assistant",
                                    format!("Stream error: {msg}"),
                                ),
                            );
                            current_message_id.clear();
                            full_response.clear();
                            mark_status(&reader_app, &reader_session_id, "idle");
                        }
                        _ => {}
                    }
                }
                Some("message_end" | "message_stop") => {
                    if let Some(message) = event.get("message") {
                        let role = message.get("role").and_then(Value::as_str).unwrap_or("assistant");
                        // Skip user-role message_end: send_message already added the user
                        // message optimistically. Pi echoing it back would create a duplicate.
                        if role != "user" {
                            finalize_message_end(
                                &reader_app,
                                &reader_session_id,
                                message,
                                &current_message_id,
                                &current_thinking_id,
                                &thinking_response,
                            );
                        }
                        message_persisted = true;
                        current_message_id.clear();
                        full_response.clear();
                        current_thinking_id.clear();
                        thinking_response.clear();
                    }
                }
                Some("agent_end") => {
                    // Fallback path: Pi did not emit message_end. Persist what we streamed.
                    if !message_persisted {
                        if !current_thinking_id.is_empty() {
                            finalize_thinking(
                                &reader_app,
                                &reader_session_id,
                                &current_thinking_id,
                                thinking_response.clone(),
                            );
                        }
                        if !current_message_id.is_empty() && !full_response.trim().is_empty() {
                            finalize_assistant(
                                &reader_app,
                                &reader_session_id,
                                &current_message_id,
                                full_response.clone(),
                            );
                        }
                    }
                    current_message_id.clear();
                    full_response.clear();
                    current_thinking_id.clear();
                    thinking_response.clear();
                    message_persisted = false;
                    mark_status(&reader_app, &reader_session_id, "idle");
                }
                _ => {}
            }
        }

        let store = reader_app.state::<SessionStore>();
        if let Ok(mut runtimes) = store.runtimes.lock() {
            runtimes.remove(&reader_session_id);
        }
        drop(store);
        let _ = reader_app.emit(
            "pi://session-closed",
            serde_json::json!({ "session_id": reader_session_id }),
        );
        mark_status(&reader_app, &reader_session_id, "idle");
    });

    if let Some(stderr) = stderr {
        let err_app = app.clone();
        let err_session_id = session_id.clone();
        thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                if !line.trim().is_empty() {
                    eprintln!("pi[{err_session_id}]: {line}");
                }
            }
            let _ = err_app;
        });
    }

    if request_initial_state {
        request_pi_state(&runtime, &session_id)?;
        // When resuming an existing Pi session, ask Pi for the canonical transcript
        // and replace Olympus's cached messages. New sessions skip this — Pi has nothing.
        let is_resume = {
            let sessions = store
                .sessions
                .lock()
                .map_err(|_| "session store poisoned")?;
            sessions
                .get(&session_id)
                .map(|s| s.pi_session_file.is_some() || s.pi_session_id.is_some())
                .unwrap_or(false)
        };
        if is_resume {
            request_pi_messages(&runtime, &session_id)?;
        }
    }

    Ok(runtime)
}


fn shutdown_all_runtimes(app: &AppHandle) {
    let store = app.state::<SessionStore>();
    let entries: Vec<(String, RunningSession)> = match store.runtimes.lock() {
        Ok(mut map) => map.drain().collect(),
        Err(_) => return,
    };

    let timeout = std::time::Duration::from_secs(2);
    let mut handles = Vec::with_capacity(entries.len());
    for (session_id, runtime) in entries {
        handles.push(thread::spawn(move || {
            graceful_shutdown(&runtime, &session_id, timeout);
        }));
    }
    for handle in handles {
        let _ = handle.join();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(SessionStore::default())
        .invoke_handler(tauri::generate_handler![
            commands::create_session,
            commands::list_sessions,
            commands::switch_session,
            commands::list_pi_models,
            commands::set_pi_model,
            commands::set_pi_thinking_level,
            commands::list_pi_commands,
            commands::respond_extension_ui,
            commands::compact_session,
            commands::rename_pi_session,
            commands::stop_session,
            commands::close_session,
            commands::send_message,
            commands::send_pi_command,
            commands::list_pi_imports,
            commands::import_pi_session,
            commands::reset_pi_session
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            let store = handle.state::<SessionStore>();
            load_sessions(&handle, &store);
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building olympus")
        .run(|app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                shutdown_all_runtimes(app_handle);
            }
        });
}
