use std::path::PathBuf;

/// Application Support directory for this build.
///
/// Local builds (`just run`, `just sign`) compile with `ANGRY_HUB_DATA_DIR` and
/// keep their database and window state out of the production folder. A
/// published release leaves that variable unset and uses `Angry Hub`.
pub fn support_dir() -> PathBuf {
    let name = if cfg!(debug_assertions) {
        "Angry Hub Dev"
    } else {
        option_env!("ANGRY_HUB_DATA_DIR").unwrap_or("Angry Hub")
    };
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(name)
}
