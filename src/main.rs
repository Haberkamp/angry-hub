use std::borrow::Cow;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{
    App, Application, AssetSource, Bounds, Context, MouseDownEvent, Render, Result, SharedString,
    TitlebarOptions, Window, WindowBounds, WindowOptions, div, ease_in_out, prelude::*, px, size,
};
use rooter::Router;

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
mod views;

use datasource::code_host;
use session::Session;
use ui::NotificationList;
use views::activity::Activity;
use views::login::Login;
use views::pull_requests::PullRequests;
use views::settings::Settings;

const DEFAULT_WINDOW_SIZE: gpui::Size<gpui::Pixels> = size(px(800.0), px(600.0));
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
    resize_generation: u64,
}

impl AppView {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let login = cx.new(|_| Login::new());
        let chrome = cx.new(|_| layout::Chrome::new());
        let pull_requests = cx.new(|cx| PullRequests::new(chrome.clone(), window, cx));
        let activity = cx.new(|cx| Activity::new(window, cx));
        let settings = cx.new(|_| Settings::new());
        Self {
            router: Router::attach(
                window,
                cx,
                routes::routes(login, pull_requests, activity, settings, chrome),
            ),
            notifications: NotificationList::init(cx),
            resize_generation: 0,
        }
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
            .size_full()
            .flex()
            .flex_col()
            .relative()
            .bg(color::gray::s1())
            .text_color(color::gray::s12())
            .child(self.router.clone())
            .child(self.notifications.clone())
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
    Application::new()
        .with_assets(Assets { base: asset_base() })
        .run(|cx: &mut App| {
            if let Ok(http) = crate::http::GpuiReqwestClient::new() {
                cx.set_http_client(Arc::new(http));
            }
            cx.set_global(Session {
                logged_in: code_host().has_saved_session(),
            });
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
                |window, cx| cx.new(|cx| AppView::new(window, cx)),
            )
            .unwrap();
            cx.activate(true);
        });
}
