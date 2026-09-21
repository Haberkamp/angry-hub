use std::path::PathBuf;

use serde::Serialize;
use serde::de::DeserializeOwned;

fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("angry-hub")
}

pub fn load<T: DeserializeOwned>(file_name: &str) -> Option<T> {
    let contents = std::fs::read_to_string(config_dir().join(file_name)).ok()?;
    serde_json::from_str(&contents).ok()
}

pub fn save<T: Serialize>(file_name: &str, value: &T) {
    let path = config_dir().join(file_name);
    let _ = std::fs::create_dir_all(path.parent().unwrap());
    if let Ok(json) = serde_json::to_string_pretty(value) {
        let _ = std::fs::write(path, json);
    }
}

pub fn remove(file_name: &str) {
    let _ = std::fs::remove_file(config_dir().join(file_name));
}
