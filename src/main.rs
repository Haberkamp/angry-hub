use std::borrow::Cow;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{
    App, Application, AssetSource, Bounds, Context, Entity, KeyBinding, MouseDownEvent, Render,
    Result, SharedString, Subscription, Timer, TitlebarOptions, Window, WindowBounds,
    WindowOptions, actions, div, ease_in_out, prelude::*, px, rgb, size,
};

mod datasource;
mod github;
mod http;
mod layout;
mod model;
mod prefs;
mod route;
mod router;
mod session;
mod ui;
mod views;

use route::Route;
use router::Router;
use session::Session;
use views::{ActivityView, LoginView, PullRequestsView};

actions!(nav, [GoBack, GoForward]);

use datasource::CodeHost;

const DEFAULT_WINDOW_SIZE: gpui::Size<gpui::Pixels> = size(px(800.0), px(600.0));
const RESTORE_ANIMATION: Duration = Duration::from_millis(250);

#[cfg(target_os = "macos")]
fn running_from_app_bundle() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|p| p.to_path_buf()))
        .is_some_and(|macos_dir| macos_dir.file_name().and_then(|s| s.to_str()) == Some("MacOS"))
}

fn init_desktop_notifications() {
    #[cfg(target_os = "macos")]
    {
        if !running_from_app_bundle() {
            return;
        }
        // UNUserNotificationCenter is required on current macOS; the old
        // NSUserNotification path can prompt for permission and still deliver nothing.
        std::thread::spawn(|| {
            let _ = notify_rust::request_auth_blocking();
        });
    }
}

pub(crate) fn show_desktop_notification(summary: impl Into<String>, body: impl Into<String>) {
    let summary = summary.into();
    let body = body.into();
    std::thread::spawn(move || {
        let result = notify_rust::Notification::new()
            .summary(&summary)
            .body(&body)
            .sound_name("default")
            .show();
        if let Err(err) = result {
            eprintln!("failed to show notification: {err}");
        }
    });
}

pub(crate) fn code_host() -> std::sync::Arc<dyn CodeHost> {
    std::sync::Arc::new(github::GithubApi::new())
}

struct Assets {
    base: PathBuf,
}

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        fs::read(self.base.join(path))
            .map(|data| Some(Cow::Owned(data)))
            .map_err(|err| err.into())
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        fs::read_dir(self.base.join(path))
            .map(|entries| {
                entries
                    .filter_map(|entry| {
                        entry
                            .ok()
                            .and_then(|entry| entry.file_name().into_string().ok())
                            .map(SharedString::from)
                    })
                    .collect()
            })
            .map_err(|err| err.into())
    }
}

struct AngryHub {
    session: Entity<Session>,
    router: Entity<Router<Route>>,
    login: Entity<LoginView>,
    prs: Entity<PullRequestsView>,
    activity: Entity<ActivityView>,
    resize_generation: u64,
    _subscriptions: Vec<Subscription>,
}

impl AngryHub {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let logged_in = code_host().has_saved_session();
        let session = cx.new(|_| Session::new(logged_in));
        let initial = if logged_in {
            Route::pull_requests()
        } else {
            Route::Login
        };
        let router = cx.new(|_| Router::new(initial, Session::allow(logged_in)));
        let login = cx.new(|_| LoginView::new(session.clone(), router.clone()));
        let prs = cx.new(|cx| PullRequestsView::new(session.clone(), router.clone(), window, cx));
        let activity = cx.new(|cx| ActivityView::new(session.clone(), router.clone(), window, cx));

        let mut view = Self {
            session,
            router,
            login,
            prs,
            activity,
            resize_generation: 0,
            _subscriptions: Vec::new(),
        };

        view._subscriptions
            .push(cx.observe(&view.session, |this, _, cx| {
                let logged_in = this.session.read(cx).logged_in;
                this.router.update(cx, |router, cx| {
                    router.enforce(Session::allow(logged_in), cx);
                });
            }));
        view._subscriptions
            .push(cx.observe(&view.router, |_, _, cx| cx.notify()));
        view
    }

    fn animate_restore_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let from = window.viewport_size();
        let to = DEFAULT_WINDOW_SIZE;
        if from == to {
            return;
        }

        self.resize_generation = self.resize_generation.wrapping_add(1);
        let generation = self.resize_generation;
        let started = Instant::now();

        cx.spawn_in(window, async move |this, cx| {
            loop {
                let t =
                    (started.elapsed().as_secs_f32() / RESTORE_ANIMATION.as_secs_f32()).min(1.0);
                let e = ease_in_out(t);
                let width = from.width + (to.width - from.width) * e;
                let height = from.height + (to.height - from.height) * e;
                let keep_going = this
                    .update_in(cx, |this, window, _cx| {
                        if this.resize_generation != generation {
                            false
                        } else {
                            window.resize(size(width, height));
                            true
                        }
                    })
                    .unwrap_or(false);
                if !keep_going || t >= 1.0 {
                    break;
                }
                Timer::after(Duration::from_millis(8)).await;
            }
        })
        .detach();
    }
}

impl Render for AngryHub {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let page = match self.router.read(cx).current() {
            Route::Login => self.login.clone().into_any_element(),
            Route::PullRequests(_) => self.prs.clone().into_any_element(),
            Route::Activity => self.activity.clone().into_any_element(),
        };

        div()
            .id("root")
            .size_full()
            .flex()
            .flex_col()
            .relative()
            .bg(rgb(0x1e1e1e))
            .text_color(rgb(0xffffff))
            .child(page)
            .on_action(cx.listener(|this, _: &GoBack, _, cx| {
                let logged_in = this.session.read(cx).logged_in;
                this.router.update(cx, |router, cx| {
                    router.back(Session::allow(logged_in), cx);
                });
            }))
            .on_action(cx.listener(|this, _: &GoForward, _, cx| {
                let logged_in = this.session.read(cx).logged_in;
                this.router.update(cx, |router, cx| {
                    router.forward(Session::allow(logged_in), cx);
                });
            }))
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    const TITLEBAR_HEIGHT: gpui::Pixels = px(28.0);
                    if event.click_count == 2 && event.position.y < TITLEBAR_HEIGHT {
                        this.animate_restore_window(window, cx);
                    }
                }),
            )
    }
}

fn asset_base() -> PathBuf {
    if let Ok(exe) = std::env::current_exe()
        && let Some(macos_dir) = exe.parent()
        && macos_dir.file_name().and_then(|s| s.to_str()) == Some("MacOS")
        && let Some(contents) = macos_dir.parent()
    {
        let bundled = contents.join("Resources").join("assets");
        if bundled.exists() {
            return bundled;
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets")
}

fn main() {
    init_desktop_notifications();
    Application::new()
        .with_assets(Assets { base: asset_base() })
        .run(|cx: &mut App| {
            if let Ok(http) = crate::http::GpuiReqwestClient::new() {
                cx.set_http_client(Arc::new(http));
            }
            gpui_selectable_text::register_keyboard_bridge(cx).detach();
            cx.bind_keys([
                KeyBinding::new("cmd-[", GoBack, None),
                KeyBinding::new("cmd-]", GoForward, None),
            ]);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        DEFAULT_WINDOW_SIZE,
                        cx,
                    ))),
                    window_min_size: Some(size(px(480.0), px(600.0))),
                    titlebar: Some(TitlebarOptions {
                        appears_transparent: true,
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| AngryHub::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
}
