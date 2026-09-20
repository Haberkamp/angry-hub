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
    #[cfg(target_os = "macos")]
    {
        install_macos_bundle()
    }
    #[cfg(not(target_os = "macos"))]
    {
        configure()
            .map_err(|error| error.to_string())?
            .update()
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

#[cfg(target_os = "macos")]
fn current_app_bundle() -> Result<std::path::PathBuf, String> {
    let exe = std::env::current_exe().map_err(|error| error.to_string())?;
    exe.ancestors()
        .find(|path| path.extension().and_then(|ext| ext.to_str()) == Some("app"))
        .map(std::path::Path::to_path_buf)
        .ok_or_else(|| "Angry Hub is not running from an .app bundle".into())
}

#[cfg(target_os = "macos")]
fn run(command: &str, args: &[&str]) -> Result<(), String> {
    let output = std::process::Command::new(command)
        .args(args)
        .output()
        .map_err(|error| format!("{command} failed to start: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("{command} failed: {stderr}"))
    }
}

#[cfg(target_os = "macos")]
fn install_macos_bundle() -> Result<(), String> {
    let updater = configure().map_err(|error| error.to_string())?;
    let release = updater
        .is_update_available()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "no update available".to_string())?;
    let asset = release
        .asset_for(self_update::get_target(), None)
        .ok_or_else(|| "no macOS release asset for this Mac".to_string())?;

    let bundle = current_app_bundle()?;
    let parent = bundle
        .parent()
        .ok_or_else(|| "could not find the folder that contains Angry Hub.app".to_string())?;

    let staging = tempfile::TempDir::new_in(parent).map_err(|error| error.to_string())?;
    let stash = tempfile::TempDir::new_in(parent).map_err(|error| error.to_string())?;
    let archive_path = staging.path().join(asset.name());
    {
        let mut file = std::fs::File::create(&archive_path).map_err(|error| error.to_string())?;
        self_update::Download::from_url(asset.download_url())
            .request_header(
                self_update::http::header::ACCEPT,
                "application/octet-stream",
            )
            .download_to(&mut file)
            .map_err(|error| error.to_string())?;
    }

    run(
        "/usr/bin/ditto",
        &[
            "-x",
            "-k",
            archive_path.to_str().ok_or("archive path is not UTF-8")?,
            staging.path().to_str().ok_or("staging path is not UTF-8")?,
        ],
    )?;

    let staged = staging.path().join("Angry Hub.app");
    if !staged.is_dir() {
        return Err("the release zip did not contain Angry Hub.app".into());
    }

    let staged_str = staged.to_str().ok_or("staged app path is not UTF-8")?;
    let _ = run(
        "/usr/bin/xattr",
        &["-dr", "com.apple.quarantine", staged_str],
    );
    run("/usr/bin/codesign", &["--verify", "--strict", staged_str])?;

    let exe = std::env::current_exe().map_err(|error| error.to_string())?;
    let stashed_exe = stash.path().join("exe-aside");
    let stashed_old = stash.path().join("old");
    std::fs::rename(&exe, &stashed_exe).map_err(|error| error.to_string())?;
    if let Err(error) = std::fs::rename(&bundle, &stashed_old) {
        let _ = std::fs::rename(&stashed_exe, &exe);
        return Err(error.to_string());
    }
    if let Err(error) = std::fs::rename(&staged, &bundle) {
        let _ = std::fs::rename(&stashed_old, &bundle);
        let _ = std::fs::rename(&stashed_exe, &exe);
        return Err(error.to_string());
    }

    if let Some(installed) = bundle.to_str() {
        let _ = run(
            "/usr/bin/xattr",
            &["-dr", "com.apple.quarantine", installed],
        );
    }
    Ok(())
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
