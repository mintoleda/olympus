use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::PathBuf,
    io::{BufRead, BufReader, Write},
    process::{Command, Stdio},
    sync::{atomic::{AtomicU64, Ordering}, Mutex},
    thread,
};
use serde_json::Value;
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
}

#[derive(Clone, Serialize)]
struct SessionEvent {
    session_id: String,
    message: ChatMessage,
}

#[derive(Default)]
struct SessionStore {
    sessions: Mutex<HashMap<String, PiSession>>,
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

#[tauri::command]
fn create_session(project_path: Option<String>, store: State<'_, SessionStore>) -> Result<PiSession, String> {
    let id_num = store.counter.fetch_add(1, Ordering::Relaxed) + 1;
    let project_path = project_path.unwrap_or_else(|| std::env::current_dir().map(|p| p.display().to_string()).unwrap_or_else(|_| "~".into()));
    let id = format!("session-{id_num}");
    let session_dir = String::new();
    let session = PiSession {
        id: id.clone(),
        name: project_name(&project_path),
        project_path,
        status: "active".into(),
        messages: vec![ChatMessage {
            id: format!("session-{id_num}-hello"),
            role: "assistant".into(),
            content: "Real Pi session ready. Olympus is using your normal `$HOME/.pi` config and session storage.".into(),
            timestamp: now_ms(),
        }],
        session_dir,
    };

    store.sessions.lock().map_err(|_| "session store poisoned")?.insert(session.id.clone(), session.clone());
    *store.active.lock().map_err(|_| "active session lock poisoned")? = Some(session.id.clone());
    Ok(session)
}

#[tauri::command]
fn list_sessions(store: State<'_, SessionStore>) -> Result<Vec<PiSession>, String> {
    let mut sessions: Vec<_> = store.sessions.lock().map_err(|_| "session store poisoned")?.values().cloned().collect();
    let active = store.active.lock().map_err(|_| "active session lock poisoned")?.clone();
    sessions.sort_by(|a, b| a.project_path.cmp(&b.project_path).then(a.name.cmp(&b.name)));
    for session in &mut sessions {
        if session.status != "streaming" {
            session.status = if Some(&session.id) == active.as_ref() { "active" } else { "idle" }.into();
        }
    }
    Ok(sessions)
}

#[tauri::command]
fn switch_session(id: String, store: State<'_, SessionStore>) -> Result<(), String> {
    if !store.sessions.lock().map_err(|_| "session store poisoned")?.contains_key(&id) {
        return Err("session not found".into());
    }
    *store.active.lock().map_err(|_| "active session lock poisoned")? = Some(id);
    Ok(())
}

#[tauri::command]
fn close_session(id: String, store: State<'_, SessionStore>) -> Result<(), String> {
    store.sessions.lock().map_err(|_| "session store poisoned")?.remove(&id);
    let mut active = store.active.lock().map_err(|_| "active session lock poisoned")?;
    if active.as_ref() == Some(&id) { *active = None; }
    Ok(())
}

#[tauri::command]
fn send_message(id: String, content: String, app: AppHandle, store: State<'_, SessionStore>) -> Result<(), String> {
    let user_message = ChatMessage { id: format!("{id}-u-{}", now_ms()), role: "user".into(), content: content.clone(), timestamp: now_ms() };
    let (project_path, should_continue) = {
        let mut sessions = store.sessions.lock().map_err(|_| "session store poisoned")?;
        let session = sessions.get_mut(&id).ok_or("session not found")?;
        let should_continue = session.messages.iter().any(|message| message.role == "user");
        session.messages.push(user_message.clone());
        session.status = "streaming".into();
        (session.project_path.clone(), should_continue)
    };
    let _ = app.emit("pi://message", SessionEvent { session_id: id.clone(), message: user_message });

    thread::spawn(move || {
        let mut command = Command::new("bash");
        let pi_command = if should_continue {
            "exec pi --mode rpc --continue"
        } else {
            "exec pi --mode rpc"
        };
        command
            .current_dir(&project_path)
            .arg("-lc")
            .arg(pi_command)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(err) => {
                let message = ChatMessage {
                    id: format!("{id}-a-{}", now_ms()),
                    role: "assistant".into(),
                    content: format!("Could not start shell for Pi RPC.\n{err}"),
                    timestamp: now_ms(),
                };
                let _ = app.emit("pi://message", SessionEvent { session_id: id.clone(), message });
                return;
            }
        };

        let mut rpc_stdin = child.stdin.take();
        if let Some(stdin) = rpc_stdin.as_mut() {
            let request = serde_json::json!({"id": format!("{id}-prompt-{}", now_ms()), "type": "prompt", "message": content});
            let _ = writeln!(stdin, "{}", request);
            let _ = stdin.flush();
        }

        let mut full_response = String::new();
        let mut saw_agent_end = false;
        let assistant_message_id = format!("{id}-a-{}", now_ms());
        if let Some(stdout) = child.stdout.take() {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                let Ok(event) = serde_json::from_str::<Value>(&line) else { continue };
                match event.get("type").and_then(Value::as_str) {
                    Some("message_update") => {
                        let delta_event = &event["assistantMessageEvent"];
                        let kind = delta_event.get("type").and_then(Value::as_str).unwrap_or("");
                        if kind == "text_delta" {
                            if let Some(delta) = delta_event.get("delta").and_then(Value::as_str) {
                                full_response.push_str(delta);
                                let message = ChatMessage {
                                    id: assistant_message_id.clone(),
                                    role: "assistant".into(),
                                    content: delta.to_string(),
                                    timestamp: now_ms(),
                                };
                                let _ = app.emit("pi://message", SessionEvent { session_id: id.clone(), message });
                            }
                        }
                    }
                    Some("agent_end") => {
                        saw_agent_end = true;
                        break;
                    }
                    Some("response") if event.get("success").and_then(Value::as_bool) == Some(false) => {
                        let text = event.get("error").map(Value::to_string).unwrap_or_else(|| "Pi rejected prompt".into());
                        let message = ChatMessage {
                            id: assistant_message_id.clone(),
                            role: "assistant".into(),
                            content: text,
                            timestamp: now_ms(),
                        };
                        let _ = app.emit("pi://message", SessionEvent { session_id: id.clone(), message });
                    }
                    _ => {}
                }
            }
        }

        drop(rpc_stdin);
        let _ = child.kill();
        let status = child.wait();
        if !saw_agent_end {
            if let Ok(exit) = status {
                if !exit.success() {
                    if let Some(stderr) = child.stderr.take() {
                        let mut err_text = String::new();
                        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                            err_text.push_str(&line);
                            err_text.push('\n');
                        }
                        let message = ChatMessage {
                            id: format!("{id}-err-{}", now_ms()),
                            role: "assistant".into(),
                            content: format!("Pi exited before completing ({exit}).\n{}", err_text.trim()),
                            timestamp: now_ms(),
                        };
                        let _ = app.emit("pi://message", SessionEvent { session_id: id.clone(), message });
                    }
                }
            }
        }

        let store = app.state::<SessionStore>();
        if let Ok(mut sessions) = store.sessions.lock() {
            if let Some(session) = sessions.get_mut(&id) {
                if full_response.trim().is_empty() {
                    session.messages.push(ChatMessage { id: assistant_message_id, role: "assistant".into(), content: "Pi returned no output.".into(), timestamp: now_ms() });
                } else {
                    session.messages.push(ChatMessage { id: assistant_message_id, role: "assistant".into(), content: full_response, timestamp: now_ms() });
                }
                session.status = "idle".into();
            }
        }
        let done = ChatMessage { id: format!("{id}-done-{}", now_ms()), role: "status".into(), content: "done".into(), timestamp: now_ms() };
        let _ = app.emit("pi://message", SessionEvent { session_id: id, message: done });
    });
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(SessionStore::default())
        .invoke_handler(tauri::generate_handler![create_session, list_sessions, switch_session, close_session, send_message])
        .run(tauri::generate_context!())
        .expect("error while running olympus");
}
