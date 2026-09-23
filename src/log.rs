//! Structured file logging: one JSON object per line (NDJSON), written to
//! `angry-hub.log` in the application support directory. In debug builds
//! lines are also mirrored to stderr in a readable key=value format.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;
use std::sync::OnceLock;

static SINK: OnceLock<Option<Mutex<std::fs::File>>> = OnceLock::new();

fn sink() -> &'static Option<Mutex<std::fs::File>> {
    SINK.get_or_init(|| open(&crate::paths::support_dir()))
}

fn open(dir: &Path) -> Option<Mutex<std::fs::File>> {
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("angry-hub.log"))
        .ok()?;
    Some(Mutex::new(file))
}

/// A field value for a log event.
pub enum Value {
    Str(String),
    Int(i64),
    Bool(bool),
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Value::Str(value.to_string())
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Value::Str(value)
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Value::Int(value)
    }
}

impl From<usize> for Value {
    fn from(value: usize) -> Self {
        Value::Int(value as i64)
    }
}

impl From<u64> for Value {
    fn from(value: u64) -> Self {
        Value::Int(value as i64)
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Value::Bool(value)
    }
}

fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

pub fn log(level: &str, event: &str, fields: &[(&str, Value)]) {
    let time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let mut json = format!(r#"{{"time":{time},"level":"{level}","event":"{event}""#);
    let mut readable = format!("time={time} level={level} event={event}");
    for (key, value) in fields {
        match value {
            Value::Str(value) => {
                json.push_str(&format!(",\"{key}\":{}", escape(value)));
                readable.push_str(&format!(" {key}={value}"));
            }
            Value::Int(value) => {
                json.push_str(&format!(",\"{key}\":{value}"));
                readable.push_str(&format!(" {key}={value}"));
            }
            Value::Bool(value) => {
                json.push_str(&format!(",\"{key}\":{value}"));
                readable.push_str(&format!(" {key}={value}"));
            }
        }
    }
    json.push('}');
    readable.push('\n');
    json.push('\n');
    if let Some(file) = sink() {
        if let Ok(mut file) = file.lock() {
            let _ = file.write_all(json.as_bytes());
        }
    }
    if cfg!(debug_assertions) {
        eprint!("{readable}");
    }
}

#[macro_export]
macro_rules! log_info {
    ($event:literal $(, $key:literal => $value:expr)* $(,)?) => {
        $crate::log::log("info", $event, &[$(($key, $value.into())),*])
    };
}

#[macro_export]
macro_rules! log_error {
    ($event:literal $(, $key:literal => $value:expr)* $(,)?) => {
        $crate::log::log("error", $event, &[$(($key, $value.into())),*])
    };
}