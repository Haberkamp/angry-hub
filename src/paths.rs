use std::path::PathBuf;

/// Application Support directory for this build.
///
/// Local builds (`just run`, `just sign`) compile with `ANGRY_HUB_DATA_DIR` and
/// keep their database and window state out of the production folder. A
/// published release leaves that variable unset and uses `angry-hub`.
pub fn support_dir() -> PathBuf {
    let name = if cfg!(debug_assertions) {
        "angry-hub-dev"
    } else {
        option_env!("ANGRY_HUB_DATA_DIR").unwrap_or("angry-hub")
    };
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(name)
}
