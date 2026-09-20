use std::collections::HashSet;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::color::ThemePreference;

#[derive(Default, Deserialize, Serialize)]
struct StoredPrefs {
    #[serde(default)]
    visible_tab_repos: Vec<String>,
    #[serde(default)]
    theme: ThemePreference,
}

fn prefs_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("angry-hub")
        .join("prefs.json")
}

fn load() -> StoredPrefs {
    std::fs::read_to_string(prefs_path())
        .ok()
        .and_then(|contents| serde_json::from_str(&contents).ok())
        .unwrap_or_default()
}

fn try_load() -> Option<StoredPrefs> {
    let contents = std::fs::read_to_string(prefs_path()).ok()?;
    serde_json::from_str(&contents).ok()
}

fn save(stored: &StoredPrefs) {
    let path = prefs_path();
    let _ = std::fs::create_dir_all(path.parent().unwrap());
    if let Ok(json) = serde_json::to_string_pretty(stored) {
        let _ = std::fs::write(path, json);
    }
}

pub struct Prefs;

impl Prefs {
    pub fn load_visible_tab_repos() -> Option<HashSet<String>> {
        let stored = try_load()?;
        Some(stored.visible_tab_repos.into_iter().collect())
    }

    pub fn save_visible_tab_repos(repos: &HashSet<String>) {
        let mut stored = load();
        stored.visible_tab_repos = {
            let mut repos: Vec<String> = repos.iter().cloned().collect();
            repos.sort();
            repos
        };
        save(&stored);
    }

    pub fn load_theme() -> ThemePreference {
        load().theme
    }

    pub fn save_theme(theme: ThemePreference) {
        let mut stored = load();
        stored.theme = theme;
        save(&stored);
    }
}
