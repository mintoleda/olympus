use std::{
    path::Path,
    process::{Command, Stdio},
    sync::{atomic::Ordering, mpsc::sync_channel},
    time::Duration,
};

use serde_json::Value;
use tauri::{AppHandle, Manager, State};

use crate::{
    emit_statuses, emit_widgets, merge_with_builtins, pi_import,
    persistence::{load_sessions, save_sessions},
    pi_events::{emit_message, emit_session_update, mark_status},
    spawn_pi, spawn_pi_inner, write_rpc,
    state::{now_ms, project_name, ChatMessage, PiCommandOption, PiModelOption, PiSession, SessionStore},
};

#[tauri::command]
pub(crate) fn create_session(
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
    let defaults = pi_import::read_pi_defaults();
    let (provider, model_id_default, thinking_level) = match defaults {
        Some(d) => (d.provider, d.model, d.thinking_level),
        None => (None, None, None),
    };
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
            msg_type: None,
        }],
        session_dir: String::new(),
        pi_session_id: None,
        pi_session_file: None,
        model: model_id_default.clone(),
        model_id: model_id_default,
        provider,
        thinking_level,
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
pub(crate) fn reset_pi_session(
    id: String,
    app: AppHandle,
    store: State<'_, SessionStore>,
) -> Result<(), String> {
    let _ = store;
    mark_status(&app, &id, "resetting");
    let runtime = spawn_pi(app, id.clone())?;
    write_rpc(
        &runtime,
        serde_json::json!({"id": format!("{id}-reset-{}", now_ms()), "type": "new_session"}),
    )
}

#[tauri::command]
pub(crate) fn list_pi_imports(
    project_path: Option<String>,
) -> Result<Vec<pi_import::PiSessionMeta>, String> {
    Ok(pi_import::discover_pi_sessions(project_path.as_deref()))
}

#[tauri::command]
pub(crate) fn import_pi_session(
    session_file: String,
    app: AppHandle,
    store: State<'_, SessionStore>,
) -> Result<PiSession, String> {
    let sessions_root = pi_import::sessions_root().ok_or("Pi sessions directory not found")?;
    let candidate = Path::new(&session_file)
        .canonicalize()
        .map_err(|err| format!("Invalid session file: {err}"))?;
    let root_canonical = sessions_root
        .canonicalize()
        .map_err(|err| format!("Invalid pi sessions root: {err}"))?;
    if !candidate.starts_with(&root_canonical) {
        return Err("Session file is outside the pi sessions directory".into());
    }
    let canonical_str = candidate.to_string_lossy().to_string();

    {
        let sessions = store
            .sessions
            .lock()
            .map_err(|_| "session store poisoned")?;
        for existing in sessions.values() {
            if existing.pi_session_file.as_deref() == Some(canonical_str.as_str()) {
                let existing_id = existing.id.clone();
                drop(sessions);
                *store
                    .active
                    .lock()
                    .map_err(|_| "active session lock poisoned")? = Some(existing_id.clone());
                let session = store
                    .sessions
                    .lock()
                    .map_err(|_| "session store poisoned")?
                    .get(&existing_id)
                    .cloned()
                    .ok_or("session vanished")?;
                spawn_pi(app, existing_id)?;
                return Ok(session);
            }
        }
    }

    let meta = pi_import::read_session_meta(&candidate)
        .ok_or("Could not read pi session metadata")?;

    let id_num = store.counter.fetch_add(1, Ordering::Relaxed) + 1;
    let id = format!("session-{id_num}");

    let cwd_exists = Path::new(&meta.project_path).is_dir();
    let mut messages = pi_import::parse_pi_messages(&canonical_str);
    let project_path = if cwd_exists {
        meta.project_path.clone()
    } else {
        let fallback = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| meta.project_path.clone());
        messages.push(ChatMessage {
            id: format!("session-{id_num}-cwd-warning"),
            role: "assistant".into(),
            content: format!(
                "⚠ Original project path `{}` no longer exists; running from `{}`.",
                meta.project_path, fallback
            ),
            timestamp: now_ms(),
            msg_type: None,
        });
        fallback
    };

    let session = PiSession {
        id: id.clone(),
        name: project_name(&project_path),
        project_path,
        status: "starting".into(),
        messages,
        session_dir: String::new(),
        pi_session_id: if meta.session_id.is_empty() {
            None
        } else {
            Some(meta.session_id.clone())
        },
        pi_session_file: Some(canonical_str.clone()),
        model: meta.model_id.clone(),
        model_id: meta.model_id.clone(),
        provider: meta.provider.clone(),
        thinking_level: meta.thinking_level.clone(),
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

#[tauri::command]
pub(crate) fn list_sessions(app: AppHandle, store: State<'_, SessionStore>) -> Result<Vec<PiSession>, String> {
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
    let running_session_ids = store
        .runtimes
        .lock()
        .map_err(|_| "runtime store poisoned")?
        .keys()
        .cloned()
        .collect::<std::collections::HashSet<_>>();
    for session in &mut sessions {
        if session.status == "streaming" && !running_session_ids.contains(&session.id) {
            session.status = "idle".into();
        }
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
pub(crate) fn switch_session(
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
    let cached_statuses = store
        .session_statuses
        .lock()
        .ok()
        .and_then(|map| map.get(&id).cloned())
        .unwrap_or_default();
    emit_statuses(&app, &id, cached_statuses);
    let cached_widgets = store
        .session_widgets
        .lock()
        .ok()
        .and_then(|map| map.get(&id).cloned())
        .unwrap_or_default();
    emit_widgets(&app, &id, cached_widgets);
    spawn_pi_unit(app, id)
}

#[tauri::command]
pub(crate) fn send_pi_command(id: String, content: String, app: AppHandle) -> Result<(), String> {
    let runtime = spawn_pi(app, id.clone())?;
    write_rpc(
        &runtime,
        serde_json::json!({"id": format!("{id}-cmd-{}", now_ms()), "type": "prompt", "message": content}),
    )
}

#[tauri::command]
pub(crate) fn list_pi_models(
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
pub(crate) fn set_pi_model(
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
pub(crate) fn set_pi_thinking_level(id: String, level: String, app: AppHandle) -> Result<(), String> {
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
    if let Some(session) = updated_session { emit_session_update(&app, session); }
    Ok(())
}

#[tauri::command]
pub(crate) fn list_pi_commands(id: String, app: AppHandle) -> Result<Vec<PiCommandOption>, String> {
    let runtime = spawn_pi_inner(app.clone(), id.clone(), false)?;
    let request_id = format!("{id}-commands-{}", now_ms());

    let (tx, rx) = sync_channel::<Vec<PiCommandOption>>(1);
    {
        let store = app.state::<SessionStore>();
        let mut pending = store
            .pending_commands
            .lock()
            .map_err(|_| "pending commands lock poisoned")?;
        pending.insert(request_id.clone(), tx);
    }

    let write_result = write_rpc(
        &runtime,
        serde_json::json!({"id": request_id, "type": "get_commands"}),
    );

    if let Err(err) = write_result {
        let store = app.state::<SessionStore>();
        if let Ok(mut pending) = store.pending_commands.lock() {
            pending.remove(&request_id);
        }
        return Err(err);
    }

    let commands = match rx.recv_timeout(Duration::from_secs(5)) {
        Ok(commands) => commands,
        Err(_) => {
            let store = app.state::<SessionStore>();
            if let Ok(mut pending) = store.pending_commands.lock() {
                pending.remove(&request_id);
            }
            store
                .last_commands
                .lock()
                .ok()
                .and_then(|cache| cache.get(&id).cloned())
                .unwrap_or_default()
        }
    };

    Ok(merge_with_builtins(commands))
}

#[tauri::command]
pub(crate) fn respond_extension_ui(
    id: String,
    request_id: String,
    response: Value,
    app: AppHandle,
) -> Result<(), String> {
    let runtime = spawn_pi_inner(app, id, false)?;
    let mut payload = response;
    if let Some(object) = payload.as_object_mut() {
        object.insert("type".into(), Value::String("extension_ui_response".into()));
        object.insert("id".into(), Value::String(request_id));
    }
    write_rpc(&runtime, payload)
}

#[tauri::command]
pub(crate) fn compact_session(
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
pub(crate) fn rename_pi_session(id: String, name: String, app: AppHandle) -> Result<(), String> {
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
    if let Some(session) = updated_session { emit_session_update(&app, session); }
    Ok(())
}

#[tauri::command]
pub(crate) fn stop_session(id: String, app: AppHandle, store: State<'_, SessionStore>) -> Result<(), String> {
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
    mark_status(&app, &id, "idle");
    Ok(())
}

#[tauri::command]
pub(crate) fn close_session(id: String, app: AppHandle, store: State<'_, SessionStore>) -> Result<(), String> {
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
pub(crate) fn send_message(
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
        msg_type: None,
    };

    {
        let mut sessions = store
            .sessions
            .lock()
            .map_err(|_| "session store poisoned")?;
        let session = sessions.get_mut(&id).ok_or("session not found")?;
        if session.status == "streaming" || session.status == "waiting" {
            return Err("session is already streaming".into());
        }
        session.messages.push(user_message.clone());
        // Slash commands that are session-control ops won't emit agent_start/agent_end,
        // so don't pre-set "streaming" — let agent_start set it. Use "waiting" so
        // the reader thread's new_session/idle responses can clear it without getting stuck.
        session.status = if content.trim_start().starts_with('/') {
            "waiting".into()
        } else {
            "streaming".into()
        };
        save_sessions(&app, &sessions)?;
    }
    emit_message(&app, &id, user_message);

    let runtime = match spawn_pi(app.clone(), id.clone()) {
        Ok(runtime) => runtime,
        Err(err) => {
            mark_status(&app, &id, "idle");
            return Err(err);
        }
    };

    if let Err(err) = write_rpc(
        &runtime,
        serde_json::json!({"id": format!("{id}-prompt-{}", now_ms()), "type": "prompt", "message": content}),
    ) {
        mark_status(&app, &id, "idle");
        return Err(err);
    }
    Ok(())
}
