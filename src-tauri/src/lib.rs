use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    fs,
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread,
};
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Clone, Serialize, Deserialize)]
struct ChatMessage {
    id: String,
    role: String,
    content: String,
    timestamp: u64,
}

#[derive(Clone, Serialize, Deserialize)]
struct PiSession {
    id: String,
    name: String,
    project_path: String,
    status: String,
    messages: Vec<ChatMessage>,
    session_dir: String,
    pi_session_id: Option<String>,
    pi_session_file: Option<String>,
    model: Option<String>,
    model_id: Option<String>,
    provider: Option<String>,
    thinking_level: Option<String>,
}

#[derive(Clone, Serialize)]
struct SessionEvent {
    session_id: String,
    message: ChatMessage,
}

#[derive(Clone, Serialize)]
struct SessionUpdateEvent {
    session: PiSession,
}

#[derive(Clone, Serialize)]
struct PiModelOption {
    provider: String,
    id: String,
    context: String,
    max_output: String,
    reasoning: bool,
    images: bool,
}

#[derive(Clone)]
struct RunningSession {
    child: Arc<Mutex<Child>>,
    stdin: Arc<Mutex<ChildStdin>>,
}

#[derive(Default)]
struct SessionStore {
    sessions: Mutex<HashMap<String, PiSession>>,
    runtimes: Mutex<HashMap<String, RunningSession>>,
    active: Mutex<Option<String>>,
    counter: AtomicU64,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn project_name(path: &str) -> String {
    PathBuf::from(path)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("workspace")
        .to_string()
}

fn sessions_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|err| err.to_string())?;
    fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    Ok(dir.join("sessions.json"))
}

fn save_sessions(app: &AppHandle, sessions: &HashMap<String, PiSession>) -> Result<(), String> {
    let path = sessions_path(app)?;
    let sessions: Vec<_> = sessions.values().cloned().collect();
    let json = serde_json::to_string_pretty(&sessions).map_err(|err| err.to_string())?;
    fs::write(path, json).map_err(|err| err.to_string())
}

fn persist_store(app: &AppHandle, store: &SessionStore) {
    if let Ok(sessions) = store.sessions.lock() {
        let _ = save_sessions(app, &sessions);
    }
}

fn load_sessions(app: &AppHandle, store: &SessionStore) {
    let Ok(path) = sessions_path(app) else { return };
    let Ok(json) = fs::read_to_string(path) else {
        return;
    };
    let Ok(sessions) = serde_json::from_str::<Vec<PiSession>>(&json) else {
        return;
    };
    let mut max_suffix: u64 = 0;
    if let Ok(mut map) = store.sessions.lock() {
        for mut session in sessions {
            if let Some(suffix) = session
                .id
                .strip_prefix("session-")
                .and_then(|s| s.parse::<u64>().ok())
            {
                if suffix > max_suffix {
                    max_suffix = suffix;
                }
            }
            session.status = "idle".into();
            map.insert(session.id.clone(), session);
        }
    }
    store.counter.store(max_suffix, Ordering::Relaxed);
}

fn emit_message(app: &AppHandle, session_id: &str, message: ChatMessage) {
    let _ = app.emit(
        "pi://message",
        SessionEvent {
            session_id: session_id.to_string(),
            message,
        },
    );
}

fn emit_session_update(app: &AppHandle, session: PiSession) {
    let _ = app.emit("pi://session", SessionUpdateEvent { session });
}

fn mark_status(app: &AppHandle, session_id: &str, status: &str) {
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
        },
    );
}

fn append_assistant_delta(app: &AppHandle, session_id: &str, message_id: &str, delta: &str) {
    emit_message(
        app,
        session_id,
        ChatMessage {
            id: message_id.into(),
            role: "assistant".into(),
            content: delta.into(),
            timestamp: now_ms(),
        },
    );
}

fn finalize_assistant(app: &AppHandle, session_id: &str, message_id: &str, content: String) {
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
        },
    );
}

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

fn write_rpc(runtime: &RunningSession, request: Value) -> Result<(), String> {
    let mut stdin = runtime.stdin.lock().map_err(|_| "Pi stdin lock poisoned")?;
    writeln!(stdin, "{request}").map_err(|err| err.to_string())?;
    stdin.flush().map_err(|err| err.to_string())
}

fn request_pi_state(runtime: &RunningSession, session_id: &str) -> Result<(), String> {
    write_rpc(
        runtime,
        serde_json::json!({"id": format!("{session_id}-state-{}", now_ms()), "type": "get_state"}),
    )
}

fn spawn_pi(app: AppHandle, session_id: String) -> Result<RunningSession, String> {
    spawn_pi_inner(app, session_id, true)
}

fn spawn_pi_inner(
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
    thread::spawn(move || {
        let mut current_message_id = String::new();
        let mut full_response = String::new();
        let reader = BufReader::new(stdout);

        for line in reader.lines().map_while(Result::ok) {
            let Ok(event) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            match event.get("type").and_then(Value::as_str) {
                Some("response") => {
                    if event.get("command").and_then(Value::as_str) == Some("get_state") {
                        if let Some(data) = event.get("data") {
                            handle_state_response(&reader_app, &reader_session_id, data);
                        }
                    } else if event.get("command").and_then(Value::as_str) == Some("set_model")
                        && event.get("success").and_then(Value::as_bool) == Some(true)
                    {
                        if let Some(data) = event.get("data") {
                            handle_set_model_response(&reader_app, &reader_session_id, data);
                        }
                    } else if event.get("success").and_then(Value::as_bool) == Some(false) {
                        let text = event
                            .get("error")
                            .map(Value::to_string)
                            .unwrap_or_else(|| "Pi rejected command".into());
                        emit_message(
                            &reader_app,
                            &reader_session_id,
                            ChatMessage {
                                id: format!("{reader_session_id}-err-{}", now_ms()),
                                role: "assistant".into(),
                                content: text,
                                timestamp: now_ms(),
                            },
                        );
                    }
                }
                Some("agent_start") => {
                    current_message_id = format!("{reader_session_id}-a-{}", now_ms());
                    full_response.clear();
                    mark_status(&reader_app, &reader_session_id, "streaming");
                }
                Some("message_update") => {
                    let delta_event = &event["assistantMessageEvent"];
                    if delta_event.get("type").and_then(Value::as_str) == Some("text_delta") {
                        if let Some(delta) = delta_event.get("delta").and_then(Value::as_str) {
                            if current_message_id.is_empty() {
                                current_message_id = format!("{reader_session_id}-a-{}", now_ms());
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
                }
                Some("agent_end") => {
                    if current_message_id.is_empty() {
                        current_message_id = format!("{reader_session_id}-a-{}", now_ms());
                    }
                    finalize_assistant(
                        &reader_app,
                        &reader_session_id,
                        &current_message_id,
                        full_response.clone(),
                    );
                    current_message_id.clear();
                    full_response.clear();
                }
                _ => {}
            }
        }

        let store = reader_app.state::<SessionStore>();
        if let Ok(mut runtimes) = store.runtimes.lock() {
            runtimes.remove(&reader_session_id);
        }
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
    }

    Ok(runtime)
}

#[tauri::command]
fn create_session(
    project_path: Option<String>,
    app: AppHandle,
    store: State<'_, SessionStore>,
) -> Result<PiSession, String> {
    let id_num = store.counter.fetch_add(1, Ordering::Relaxed) + 1;
    let project_path = project_path.unwrap_or_else(|| {
        std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "~".into())
    });
    let id = format!("session-{id_num}");
    let session = PiSession {
        id: id.clone(),
        name: project_name(&project_path),
        project_path,
        status: "starting".into(),
        messages: vec![ChatMessage {
            id: format!("session-{id_num}-hello"),
            role: "assistant".into(),
            content:
                "Pi session ready. Olympus will resume this conversation by its Pi session id."
                    .into(),
            timestamp: now_ms(),
        }],
        session_dir: String::new(),
        pi_session_id: None,
        pi_session_file: None,
        model: None,
        model_id: None,
        provider: None,
        thinking_level: None,
    };

    {
        let mut sessions = store
            .sessions
            .lock()
            .map_err(|_| "session store poisoned")?;
        sessions.insert(session.id.clone(), session.clone());
        save_sessions(&app, &sessions)?;
    }
    *store
        .active
        .lock()
        .map_err(|_| "active session lock poisoned")? = Some(session.id.clone());
    spawn_pi(app, session.id.clone())?;
    Ok(session)
}

fn spawn_pi_unit(app: AppHandle, session_id: String) -> Result<(), String> {
    spawn_pi(app, session_id).map(|_| ())
}

#[tauri::command]
fn list_sessions(app: AppHandle, store: State<'_, SessionStore>) -> Result<Vec<PiSession>, String> {
    if store
        .sessions
        .lock()
        .map_err(|_| "session store poisoned")?
        .is_empty()
    {
        load_sessions(&app, &store);
    }

    let mut sessions: Vec<_> = store
        .sessions
        .lock()
        .map_err(|_| "session store poisoned")?
        .values()
        .cloned()
        .collect();
    let active = store
        .active
        .lock()
        .map_err(|_| "active session lock poisoned")?
        .clone();
    sessions.sort_by(|a, b| {
        a.project_path
            .cmp(&b.project_path)
            .then(a.name.cmp(&b.name))
    });
    for session in &mut sessions {
        if session.status != "streaming"
            && session.status != "starting"
            && session.status != "error"
        {
            session.status = if Some(&session.id) == active.as_ref() {
                "active"
            } else {
                "idle"
            }
            .into();
        }
    }
    Ok(sessions)
}

#[tauri::command]
fn switch_session(
    id: String,
    app: AppHandle,
    store: State<'_, SessionStore>,
) -> Result<(), String> {
    if !store
        .sessions
        .lock()
        .map_err(|_| "session store poisoned")?
        .contains_key(&id)
    {
        return Err("session not found".into());
    }
    *store
        .active
        .lock()
        .map_err(|_| "active session lock poisoned")? = Some(id.clone());
    spawn_pi_unit(app, id)
}

#[tauri::command]
fn list_pi_models(
    id: String,
    store: State<'_, SessionStore>,
) -> Result<Vec<PiModelOption>, String> {
    let project_path = {
        let sessions = store
            .sessions
            .lock()
            .map_err(|_| "session store poisoned")?;
        sessions
            .get(&id)
            .map(|session| session.project_path.clone())
    };

    let mut command = Command::new("pi");
    command
        .arg("--list-models")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(project_path) = project_path {
        command.current_dir(project_path);
    }

    let output = command
        .output()
        .map_err(|err| format!("Could not list Pi models: {err}"))?;
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let mut models = Vec::new();
    for line in text.lines() {
        let columns: Vec<_> = line.split_whitespace().collect();
        if columns.len() < 6 || columns[0] == "provider" {
            continue;
        }
        models.push(PiModelOption {
            provider: columns[0].to_string(),
            id: columns[1].to_string(),
            context: columns[2].to_string(),
            max_output: columns[3].to_string(),
            reasoning: columns[4] == "yes",
            images: columns[5] == "yes",
        });
    }

    if models.is_empty() {
        return Err("Pi returned no available models".into());
    }
    Ok(models)
}

#[tauri::command]
fn set_pi_model(
    id: String,
    provider: String,
    model_id: String,
    app: AppHandle,
) -> Result<(), String> {
    let runtime = spawn_pi_inner(app, id.clone(), false)?;
    write_rpc(
        &runtime,
        serde_json::json!({"id": format!("{id}-set-model-{}", now_ms()), "type": "set_model", "provider": provider, "modelId": model_id}),
    )?;
    Ok(())
}

#[tauri::command]
fn set_pi_thinking_level(id: String, level: String, app: AppHandle) -> Result<(), String> {
    let runtime = spawn_pi_inner(app.clone(), id.clone(), false)?;
    write_rpc(
        &runtime,
        serde_json::json!({"id": format!("{id}-set-thinking-{}", now_ms()), "type": "set_thinking_level", "level": level}),
    )?;

    let store = app.state::<SessionStore>();
    let mut updated_session = None;
    if let Ok(mut sessions) = store.sessions.lock() {
        if let Some(session) = sessions.get_mut(&id) {
            session.thinking_level = Some(level);
            if session.status == "updating" || session.status == "starting" {
                session.status = "idle".into();
            }
            updated_session = Some(session.clone());
        }
        let _ = save_sessions(&app, &sessions);
    }
    emit_updated_session(&app, updated_session);
    Ok(())
}

#[tauri::command]
fn compact_session(
    id: String,
    custom_instructions: Option<String>,
    app: AppHandle,
) -> Result<(), String> {
    let runtime = spawn_pi_inner(app, id.clone(), false)?;
    write_rpc(
        &runtime,
        serde_json::json!({"id": format!("{id}-compact-{}", now_ms()), "type": "compact", "customInstructions": custom_instructions}),
    )
}

#[tauri::command]
fn rename_pi_session(id: String, name: String, app: AppHandle) -> Result<(), String> {
    let runtime = spawn_pi_inner(app.clone(), id.clone(), false)?;
    write_rpc(
        &runtime,
        serde_json::json!({"id": format!("{id}-name-{}", now_ms()), "type": "set_session_name", "name": name}),
    )?;

    let store = app.state::<SessionStore>();
    let mut updated_session = None;
    if let Ok(mut sessions) = store.sessions.lock() {
        if let Some(session) = sessions.get_mut(&id) {
            session.name = name;
            updated_session = Some(session.clone());
        }
        let _ = save_sessions(&app, &sessions);
    }
    emit_updated_session(&app, updated_session);
    Ok(())
}

#[tauri::command]
fn close_session(id: String, app: AppHandle, store: State<'_, SessionStore>) -> Result<(), String> {
    if let Some(runtime) = store
        .runtimes
        .lock()
        .map_err(|_| "runtime store poisoned")?
        .remove(&id)
    {
        if let Ok(mut child) = runtime.child.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
    if let Ok(mut sessions) = store.sessions.lock() {
        sessions.remove(&id);
        save_sessions(&app, &sessions)?;
    }
    let mut active = store
        .active
        .lock()
        .map_err(|_| "active session lock poisoned")?;
    if active.as_ref() == Some(&id) {
        *active = None;
    }
    Ok(())
}

#[tauri::command]
fn send_message(
    id: String,
    content: String,
    app: AppHandle,
    store: State<'_, SessionStore>,
) -> Result<(), String> {
    let user_message = ChatMessage {
        id: format!("{id}-u-{}", now_ms()),
        role: "user".into(),
        content: content.clone(),
        timestamp: now_ms(),
    };

    {
        let mut sessions = store
            .sessions
            .lock()
            .map_err(|_| "session store poisoned")?;
        let session = sessions.get_mut(&id).ok_or("session not found")?;
        if session.status == "streaming" {
            return Err("session is already streaming".into());
        }
        session.messages.push(user_message.clone());
        session.status = "streaming".into();
        save_sessions(&app, &sessions)?;
    }
    emit_message(&app, &id, user_message);

    let runtime = spawn_pi(app, id.clone())?;

    write_rpc(
        &runtime,
        serde_json::json!({"id": format!("{id}-prompt-{}", now_ms()), "type": "prompt", "message": content}),
    )?;
    Ok(())
}

fn shutdown_all_runtimes(app: &AppHandle) {
    let store = app.state::<SessionStore>();
    let runtimes: Vec<RunningSession> = match store.runtimes.lock() {
        Ok(mut map) => map.drain().map(|(_, runtime)| runtime).collect(),
        Err(_) => return,
    };
    for runtime in runtimes {
        if let Ok(mut child) = runtime.child.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(SessionStore::default())
        .invoke_handler(tauri::generate_handler![
            create_session,
            list_sessions,
            switch_session,
            list_pi_models,
            set_pi_model,
            set_pi_thinking_level,
            compact_session,
            rename_pi_session,
            close_session,
            send_message
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
