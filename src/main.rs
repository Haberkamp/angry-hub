use std::borrow::Cow;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{
    App, Application, AssetSource, Context, FocusHandle, MouseDownEvent, Render, Result,
    SharedString, Subscription, TitlebarOptions, Window, WindowOptions, div, ease_in_out,
    prelude::*, px, size,
};
use rooter::Router;

mod app_menus;
mod color;
mod datasource;
mod github;
mod http;
mod layout;
mod model;
mod prefs;
mod routes;
mod session;
mod ui;
mod updater;
mod views;
mod window_frame;

use datasource::code_host;
use session::Session;
use ui::NotificationList;
use views::activity::Activity;
use views::login::Login;
use views::pull_requests::PullRequests;
use views::settings::Settings;

const RESTORE_ANIMATION: Duration = Duration::from_millis(250);

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

struct AppView {
    router: gpui::Entity<Router>,
    notifications: gpui::Entity<NotificationList>,
    focus_handle: FocusHandle,
    resize_generation: u64,
    _subscriptions: Vec<Subscription>,
}

impl AppView {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        color::init_theme(window, cx);
        let appearance_sub = cx.observe_window_appearance(window, |_, window, cx| {
            color::sync_system_appearance(window, cx);
            cx.notify();
        });
        let bounds_sub = cx.observe_window_bounds(window, |_, window, cx| {
            window_frame::persist(window, cx);
        });
        let theme_sub = cx.observe_global::<color::Theme>(|_, cx| cx.notify());
        let login = cx.new(|_| Login::new());
        let chrome = cx.new(|_| layout::Chrome::new());
        let pull_requests = cx.new(|cx| PullRequests::new(chrome.clone(), window, cx));
        let activity = cx.new(|cx| Activity::new(window, cx));
        let settings = cx.new(|_| Settings::new());
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window);
        let this = Self {
            router: Router::attach(
                window,
                cx,
                routes::routes(login, pull_requests, activity, settings, chrome),
            ),
            notifications: NotificationList::init(cx),
            focus_handle,
            resize_generation: 0,
            _subscriptions: vec![appearance_sub, bounds_sub, theme_sub],
        };
        crate::updater::check_on_launch(window, cx);
        this
    }

    fn animate_restore_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let from = window.viewport_size();
        let to = window_frame::DEFAULT_WINDOW_SIZE;
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
                gpui::Timer::after(Duration::from_millis(8)).await;
            }
        })
        .detach();
    }
}

impl Render for AppView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("root")
            .track_focus(&self.focus_handle)
            .size_full()
            .flex()
            .flex_col()
            .relative()
            .bg(color::surface::default(cx))
            .text_color(color::text::primary(cx))
            .child(self.router.clone())
            .child(self.notifications.clone())
            .on_action(|_: &app_menus::Minimize, window, _| window.minimize_window())
            .on_action(|_: &app_menus::Zoom, window, _| window.zoom_window())
            .on_action(|_: &app_menus::ToggleFullScreen, window, _| window.toggle_fullscreen())
            .on_action(|_: &app_menus::ShowPullRequests, window, cx| {
                Router::navigate_window(window, cx, "/");
            })
            .on_action(|_: &app_menus::ShowActivity, window, cx| {
                Router::navigate_window(window, cx, "/activity");
            })
            .on_action(|_: &app_menus::CloseWindow, window, cx| {
                window_frame::persist(window, cx);
                window.remove_window()
            })
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

pub(crate) fn open_main_window(cx: &mut App) {
    let restored = window_frame::restore(cx);
    cx.open_window(
        WindowOptions {
            window_bounds: Some(restored.bounds),
            window_min_size: Some(window_frame::MIN_WINDOW_SIZE),
            display_id: restored.display_id,
            titlebar: Some(TitlebarOptions {
                appears_transparent: true,
                ..Default::default()
            }),
            ..Default::default()
        },
        |window, cx| cx.new(|cx| AppView::new(window, cx)),
    )
    .ok();
}

fn main() {
    let app = Application::new().with_assets(Assets { base: asset_base() });
    app.on_reopen(|cx| {
        if cx.windows().is_empty() {
            open_main_window(cx);
        }
    });
    app.run(|cx: &mut App| {
        if let Ok(http) = crate::http::GpuiReqwestClient::new() {
            cx.set_http_client(Arc::new(http));
        }
        cx.set_global(Session {
            logged_in: code_host().has_saved_session(),
        });
        app_menus::init(cx);
        open_main_window(cx);
        cx.activate(true);
    });
}
