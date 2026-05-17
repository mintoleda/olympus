use serde::Serialize;
use serde_json::Value;
use std::{
    fs,
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
};

use crate::ChatMessage;

const META_SCAN_LINES: usize = 200;
const META_SCAN_BYTES: usize = 64 * 1024;
const PREVIEW_CHARS: usize = 120;

pub struct PiDefaults {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub thinking_level: Option<String>,
}

#[derive(Clone, Serialize)]
pub struct PiSessionMeta {
    pub session_id: String,
    pub session_file: String,
    pub project_path: String,
    pub started_at: String,
    pub last_activity_ms: u64,
    pub message_count: u32,
    pub preview: String,
    pub provider: Option<String>,
    pub model_id: Option<String>,
    pub thinking_level: Option<String>,
}

pub fn pi_data_dir() -> Option<PathBuf> {
    if let Ok(custom) = std::env::var("PI_DATA_DIR") {
        let path = PathBuf::from(custom);
        if path.is_dir() {
            return Some(path);
        }
    }
    let home = std::env::var("HOME").ok()?;
    let path = PathBuf::from(home).join(".pi").join("agent");
    if path.is_dir() {
        Some(path)
    } else {
        None
    }
}

pub fn sessions_root() -> Option<PathBuf> {
    let root = pi_data_dir()?.join("sessions");
    if root.is_dir() {
        Some(root)
    } else {
        None
    }
}

pub fn read_pi_defaults() -> Option<PiDefaults> {
    let path = pi_data_dir()?.join("settings.json");
    let raw = fs::read_to_string(path).ok()?;
    let value: Value = serde_json::from_str(&raw).ok()?;
    let provider = value
        .get("defaultProvider")
        .and_then(Value::as_str)
        .map(str::to_string);
    let model = value
        .get("defaultModel")
        .and_then(Value::as_str)
        .map(str::to_string);
    let thinking_level = value
        .get("defaultThinkingLevel")
        .and_then(Value::as_str)
        .map(str::to_string);
    if provider.is_none() && model.is_none() && thinking_level.is_none() {
        return None;
    }
    Some(PiDefaults {
        provider,
        model,
        thinking_level,
    })
}

pub fn encode_project_path(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    let inner = trimmed.replace('/', "-");
    format!("--{inner}--")
}

pub fn decode_project_path(encoded: &str) -> String {
    let stripped = encoded
        .strip_prefix("--")
        .and_then(|s| s.strip_suffix("--"))
        .unwrap_or(encoded);
    format!("/{}", stripped.replace('-', "/"))
}

pub fn discover_pi_sessions(project_path: Option<&str>) -> Vec<PiSessionMeta> {
    let Some(root) = sessions_root() else {
        return Vec::new();
    };

    let dirs: Vec<PathBuf> = match project_path {
        Some(p) => {
            let dir = root.join(encode_project_path(p));
            if dir.is_dir() {
                vec![dir]
            } else {
                Vec::new()
            }
        }
        None => fs::read_dir(&root)
            .ok()
            .into_iter()
            .flatten()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|p| p.is_dir())
            .collect(),
    };

    let mut metas: Vec<PiSessionMeta> = Vec::new();
    for dir in dirs {
        let Ok(read_dir) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in read_dir.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            if let Some(meta) = read_session_meta(&path) {
                metas.push(meta);
            }
        }
    }

    metas.sort_by(|a, b| b.last_activity_ms.cmp(&a.last_activity_ms));
    metas
}

pub fn read_session_meta(path: &Path) -> Option<PiSessionMeta> {
    let file = fs::File::open(path).ok()?;
    let metadata = file.metadata().ok()?;
    let mut reader = BufReader::new(file).take(META_SCAN_BYTES as u64);

    let mut session_id: Option<String> = None;
    let mut started_at = String::new();
    let mut project_path = String::new();
    let mut provider: Option<String> = None;
    let mut model_id: Option<String> = None;
    let mut thinking_level: Option<String> = None;
    let mut message_count: u32 = 0;
    let mut preview = String::new();
    let mut last_event_ms: u64 = 0;

    let mut lines_scanned = 0usize;
    let mut buf = String::new();
    while lines_scanned < META_SCAN_LINES {
        buf.clear();
        match reader.read_line(&mut buf) {
            Ok(0) => break,
            Ok(_) => {}
            Err(_) => break,
        }
        lines_scanned += 1;
        let line = buf.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let kind = event.get("type").and_then(Value::as_str).unwrap_or("");
        if let Some(ts) = event.get("timestamp").and_then(Value::as_str) {
            if let Some(ms) = parse_iso_ms(ts) {
                if ms > last_event_ms {
                    last_event_ms = ms;
                }
            }
        }
        match kind {
            "session" => {
                session_id = event
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                if let Some(ts) = event.get("timestamp").and_then(Value::as_str) {
                    started_at = ts.to_string();
                }
                if let Some(cwd) = event.get("cwd").and_then(Value::as_str) {
                    project_path = cwd.to_string();
                }
            }
            "model_change" => {
                if let Some(p) = event.get("provider").and_then(Value::as_str) {
                    provider = Some(p.to_string());
                }
                if let Some(m) = event.get("modelId").and_then(Value::as_str) {
                    model_id = Some(m.to_string());
                }
            }
            "thinking_level_change" => {
                if let Some(t) = event.get("thinkingLevel").and_then(Value::as_str) {
                    thinking_level = Some(t.to_string());
                }
            }
            "message" => {
                message_count = message_count.saturating_add(1);
                if preview.is_empty() {
                    if let Some(text) = first_text_part(&event) {
                        preview = truncate_chars(&text, PREVIEW_CHARS);
                    }
                }
            }
            _ => {}
        }
    }

    if project_path.is_empty() {
        let file_name = path.parent().and_then(|p| p.file_name()).and_then(|n| n.to_str()).unwrap_or("");
        project_path = decode_project_path(file_name);
    }

    let mtime_ms = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let last_activity_ms = std::cmp::max(last_event_ms, mtime_ms);

    Some(PiSessionMeta {
        session_id: session_id.unwrap_or_default(),
        session_file: path.to_string_lossy().to_string(),
        project_path,
        started_at,
        last_activity_ms,
        message_count,
        preview,
        provider,
        model_id,
        thinking_level,
    })
}

pub fn parse_pi_messages(session_file: &str) -> Vec<ChatMessage> {
    let Ok(file) = fs::File::open(session_file) else {
        return Vec::new();
    };
    let reader = BufReader::new(file);
    let mut messages: Vec<ChatMessage> = Vec::new();
    for line in reader.lines().map_while(Result::ok) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(event) = serde_json::from_str::<Value>(trimmed) else {
            continue;
        };
        if event.get("type").and_then(Value::as_str) != Some("message") {
            continue;
        }
        let Some(msg) = event.get("message") else {
            continue;
        };
        let role = msg
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or("user")
            .to_string();
        let content = flatten_content(msg.get("content"));
        if content.is_empty() {
            continue;
        }
        let id = event
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| format!("pi-{}", messages.len()));
        let timestamp = msg
            .get("timestamp")
            .and_then(Value::as_u64)
            .or_else(|| {
                event
                    .get("timestamp")
                    .and_then(Value::as_str)
                    .and_then(parse_iso_ms)
            })
            .unwrap_or(0);
        messages.push(ChatMessage {
            id,
            role,
            content,
            timestamp,
            msg_type: None,
        });
    }
    messages
}

fn first_text_part(event: &Value) -> Option<String> {
    let content = event.get("message")?.get("content")?;
    if let Some(arr) = content.as_array() {
        for part in arr {
            if part.get("type").and_then(Value::as_str) == Some("text") {
                if let Some(text) = part.get("text").and_then(Value::as_str) {
                    let t = text.trim();
                    if !t.is_empty() {
                        return Some(t.to_string());
                    }
                }
            }
        }
        None
    } else if let Some(s) = content.as_str() {
        Some(s.trim().to_string())
    } else {
        None
    }
}

fn flatten_content(content: Option<&Value>) -> String {
    let Some(content) = content else {
        return String::new();
    };
    if let Some(s) = content.as_str() {
        return s.to_string();
    }
    let Some(arr) = content.as_array() else {
        return String::new();
    };
    let mut out = String::new();
    for part in arr {
        let kind = part.get("type").and_then(Value::as_str).unwrap_or("");
        match kind {
            "text" => {
                if let Some(text) = part.get("text").and_then(Value::as_str) {
                    if !out.is_empty() {
                        out.push_str("\n\n");
                    }
                    out.push_str(text);
                }
            }
            "tool_use" => {
                let name = part.get("name").and_then(Value::as_str).unwrap_or("tool");
                if !out.is_empty() {
                    out.push_str("\n\n");
                }
                out.push_str(&format!("```tool:{name}\n[tool call]\n```"));
            }
            "tool_result" => {}
            _ => {}
        }
    }
    out
}

fn truncate_chars(s: &str, n: usize) -> String {
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i >= n {
            out.push('…');
            break;
        }
        if ch == '\n' || ch == '\r' {
            out.push(' ');
        } else {
            out.push(ch);
        }
    }
    out
}

fn parse_iso_ms(ts: &str) -> Option<u64> {
    // Minimal ISO-8601 parser for "YYYY-MM-DDTHH:MM:SS(.fff)?Z" used by pi.
    // Returns Unix epoch milliseconds. Returns None on any parse mismatch.
    let bytes = ts.as_bytes();
    if bytes.len() < 20 || !ts.ends_with('Z') {
        return None;
    }
    let year: i64 = ts.get(0..4)?.parse().ok()?;
    let month: u32 = ts.get(5..7)?.parse().ok()?;
    let day: u32 = ts.get(8..10)?.parse().ok()?;
    let hour: u32 = ts.get(11..13)?.parse().ok()?;
    let minute: u32 = ts.get(14..16)?.parse().ok()?;
    let second: u32 = ts.get(17..19)?.parse().ok()?;
    let ms: u32 = if let Some(rest) = ts.get(19..ts.len() - 1) {
        if let Some(frac) = rest.strip_prefix('.') {
            let take: String = frac.chars().take(3).collect();
            let padded = format!("{:0<3}", take);
            padded.parse().unwrap_or(0)
        } else {
            0
        }
    } else {
        0
    };

    let days = days_from_civil(year, month as i32, day as i32);
    let secs = days * 86_400 + (hour as i64) * 3600 + (minute as i64) * 60 + (second as i64);
    if secs < 0 {
        return None;
    }
    Some((secs as u64) * 1000 + ms as u64)
}

fn days_from_civil(y: i64, m: i32, d: i32) -> i64 {
    // Howard Hinnant's date algorithm: days since 1970-01-01.
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as i64;
    let mp = if m > 2 { m - 3 } else { m + 9 } as i64;
    let doy = (153 * mp + 2) / 5 + (d as i64) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}
