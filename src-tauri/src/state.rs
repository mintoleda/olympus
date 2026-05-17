use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    path::PathBuf,
    process::{Child, ChildStdin},
    sync::{
        atomic::AtomicU64,
        mpsc::SyncSender,
        Arc, Mutex,
    },
};

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct ChatMessage {
    pub(crate) id: String,
    pub(crate) role: String,
    pub(crate) content: String,
    pub(crate) timestamp: u64,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub(crate) msg_type: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct PiSession {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) project_path: String,
    pub(crate) status: String,
    pub(crate) messages: Vec<ChatMessage>,
    pub(crate) session_dir: String,
    pub(crate) pi_session_id: Option<String>,
    pub(crate) pi_session_file: Option<String>,
    pub(crate) model: Option<String>,
    pub(crate) model_id: Option<String>,
    pub(crate) provider: Option<String>,
    pub(crate) thinking_level: Option<String>,
}

#[derive(Clone, Serialize)]
pub(crate) struct SessionEvent {
    pub(crate) session_id: String,
    pub(crate) message: ChatMessage,
}

#[derive(Clone, Serialize)]
pub(crate) struct SessionUpdateEvent {
    pub(crate) session: PiSession,
}

#[derive(Clone, Serialize)]
pub(crate) struct PiModelOption {
    pub(crate) provider: String,
    pub(crate) id: String,
    pub(crate) context: String,
    pub(crate) max_output: String,
    pub(crate) reasoning: bool,
    pub(crate) images: bool,
}

#[derive(Clone, Serialize)]
pub(crate) struct PiCommandOption {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) path: Option<String>,
}

#[derive(Clone, Serialize)]
pub(crate) struct ExtensionUiRequest {
    pub(crate) session_id: String,
    pub(crate) request: Value,
}

#[derive(Clone, Serialize)]
pub(crate) struct StatusEntry {
    pub(crate) key: String,
    pub(crate) text: String,
}

#[derive(Clone, Serialize)]
pub(crate) struct StatusEvent {
    pub(crate) session_id: String,
    pub(crate) statuses: Vec<StatusEntry>,
}

#[derive(Clone, Serialize)]
pub(crate) struct WidgetEntry {
    pub(crate) key: String,
    pub(crate) lines: Vec<String>,
    pub(crate) placement: String,
}

#[derive(Clone, Serialize)]
pub(crate) struct WidgetEvent {
    pub(crate) session_id: String,
    pub(crate) widgets: Vec<WidgetEntry>,
}

#[derive(Clone, Serialize)]
pub(crate) struct NotifyEvent {
    pub(crate) session_id: String,
    pub(crate) message: String,
    pub(crate) level: String,
}

#[derive(Clone, Serialize)]
pub(crate) struct TitleEvent {
    pub(crate) session_id: String,
    pub(crate) title: String,
}

#[derive(Clone, Serialize)]
pub(crate) struct EditorTextEvent {
    pub(crate) session_id: String,
    pub(crate) text: String,
}

#[derive(Clone)]
pub(crate) struct RunningSession {
    pub(crate) child: Arc<Mutex<Child>>,
    pub(crate) stdin: Arc<Mutex<ChildStdin>>,
}

#[derive(Default)]
pub(crate) struct SessionStore {
    pub(crate) sessions: Mutex<HashMap<String, PiSession>>,
    pub(crate) runtimes: Mutex<HashMap<String, RunningSession>>,
    pub(crate) active: Mutex<Option<String>>,
    pub(crate) counter: AtomicU64,
    pub(crate) pending_commands: Mutex<HashMap<String, SyncSender<Vec<PiCommandOption>>>>,
    pub(crate) last_commands: Mutex<HashMap<String, Vec<PiCommandOption>>>,
    pub(crate) session_statuses: Mutex<HashMap<String, Vec<StatusEntry>>>,
    pub(crate) session_widgets: Mutex<HashMap<String, Vec<WidgetEntry>>>,
}

pub(crate) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub(crate) fn project_name(path: &str) -> String {
    PathBuf::from(path)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("workspace")
        .to_string()
}
