use std::{collections::HashMap, fs, path::PathBuf, sync::atomic::Ordering};

use tauri::{AppHandle, Manager};

use crate::state::{PiSession, SessionStore};

pub(crate) fn sessions_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|err| err.to_string())?;
    fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    Ok(dir.join("sessions.json"))
}

pub(crate) fn save_sessions(
    app: &AppHandle,
    sessions: &HashMap<String, PiSession>,
) -> Result<(), String> {
    let path = sessions_path(app)?;
    let sessions: Vec<_> = sessions.values().cloned().collect();
    let json = serde_json::to_string_pretty(&sessions).map_err(|err| err.to_string())?;
    fs::write(path, json).map_err(|err| err.to_string())
}

pub(crate) fn persist_store(app: &AppHandle, store: &SessionStore) {
    if let Ok(sessions) = store.sessions.lock() {
        let _ = save_sessions(app, &sessions);
    }
}

pub(crate) fn load_sessions(app: &AppHandle, store: &SessionStore) {
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
