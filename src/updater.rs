use gpui::{App, PromptLevel, Window};

use crate::ui::WindowExt;

pub const DISABLE_ENV: &str = "ANGRY_HUB_DISABLE_AUTO_UPDATE";

fn configure() -> self_update::errors::Result<self_update::backends::github::Update> {
    self_update::backends::github::Update::configure()
        .repo_owner("Haberkamp")
        .repo_name("angry-hub")
        .bin_name("angry-hub")
        .current_version(self_update::cargo_crate_version!())
        .bundle_path_in_archive("Angry Hub.app")
        .no_confirm(true)
        .show_output(false)
        .show_download_progress(false)
        .build()
}

fn latest_available() -> Result<Option<String>, String> {
    let updater = configure().map_err(|error| error.to_string())?;
    match updater.is_update_available() {
        Ok(Some(release)) => Ok(Some(release.version().to_string())),
        Ok(None) => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

fn install_update() -> Result<(), String> {
    configure()
        .map_err(|error| error.to_string())?
        .update()
        .map(|_| ())
        .map_err(|error| error.to_string())
}

pub fn disabled() -> bool {
    match std::env::var(DISABLE_ENV) {
        Ok(value) => {
            let value = value.trim();
            value == "1" || value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("yes")
        }
        Err(_) => false,
    }
}

pub fn check_on_launch(window: &mut Window, cx: &mut App) {
    if disabled() {
        return;
    }

    let window = window.window_handle();
    cx.spawn(async move |cx| {
        let result = cx
            .background_executor()
            .spawn(async { latest_available() })
            .await;
        match result {
            Ok(Some(version)) => {
                let Ok(answer) = window.update(cx, |_, window, cx| {
                    window.prompt(
                        PromptLevel::Info,
                        &format!("Angry Hub {version} is available."),
                        Some("Install this update? The app will restart."),
                        &["Update", "Later"],
                        cx,
                    )
                }) else {
                    return;
                };
                if answer.await != Ok(0) {
                    return;
                }
                let install = cx
                    .background_executor()
                    .spawn(async { install_update() })
                    .await;
                match install {
                    Ok(()) => {
                        let error = self_update::restart::restart().unwrap_err();
                        let _ = window.update(cx, |_, window, cx| {
                            window.push_notification(
                                format!("Updated, but restart failed: {error}"),
                                cx,
                            );
                        });
                    }
                    Err(error) => {
                        let _ = window.update(cx, |_, window, cx| {
                            window.push_notification(format!("Update failed: {error}"), cx);
                        });
                    }
                }
            }
            Ok(None) => {}
            Err(error) => {
                let _ = window.update(cx, |_, window, cx| {
                    window.push_notification(format!("Couldn't check for updates: {error}"), cx);
                });
            }
        }
    })
    .detach();
}
