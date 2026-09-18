use std::collections::HashSet;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Default, Deserialize, Serialize)]
struct StoredPrefs {
    visible_tab_repos: Vec<String>,
}

fn prefs_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("angry-hub")
        .join("prefs.json")
}

pub struct Prefs;

impl Prefs {
    pub fn load_visible_tab_repos() -> Option<HashSet<String>> {
        let contents = std::fs::read_to_string(prefs_path()).ok()?;
        let stored: StoredPrefs = serde_json::from_str(&contents).ok()?;
        Some(stored.visible_tab_repos.into_iter().collect())
    }

    pub fn save_visible_tab_repos(repos: &HashSet<String>) {
        let path = prefs_path();
        let _ = std::fs::create_dir_all(path.parent().unwrap());
        let stored = StoredPrefs {
            visible_tab_repos: {
                let mut repos: Vec<String> = repos.iter().cloned().collect();
                repos.sort();
                repos
            },
        };
        if let Ok(json) = serde_json::to_string_pretty(&stored) {
            let _ = std::fs::write(path, json);
        }
    }
}
