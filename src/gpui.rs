use gpui::prelude::*;
use gpui::{
    Context, Entity, IntoElement, Pixels, Render, Size, TitlebarOptions, Window, WindowBounds,
    WindowOptions, div, point, px, rgb, size,
};
use gpui_base::StyledExt as _;

mod auth;
mod button;
mod chrome;
mod color;
mod frame;
mod github;
mod icon;
mod keychain;
mod session;
mod tooltip;
mod views;

use chrome::Chrome;
use views::{Home, LoggedIn, LoggedOut, Login, logout_button};

const DEFAULT_WINDOW_SIZE: Size<Pixels> = size(px(800.0), px(600.0));
const MIN_WINDOW_SIZE: Size<Pixels> = size(px(480.0), px(600.0));

enum Page {
    Login(Entity<Login>),
    Home(Entity<Home>),
}

struct Root {
    chrome: Entity<Chrome>,
    page: Page,
    _logged_in: Option<gpui::Subscription>,
}

impl Root {
    fn new(window_id: u64, window: &mut Window, cx: &mut Context<Self>) -> Self {
        frame::observe(window_id, window, cx);
        let chrome = cx.new(|_| Chrome::new(DEFAULT_WINDOW_SIZE));
        if auth::Auth::load(&keychain::KeychainStore).check() {
            let mut root = Self {
                chrome,
                page: Page::Home(cx.new(|_| Home)),
                _logged_in: None,
            };
            root.watch_page(cx);
            return root;
        }

        let mut root = Self {
            chrome,
            page: Page::Login(cx.new(|_| Login::new())),
            _logged_in: None,
        };
        root.watch_page(cx);
        root
    }

    fn watch_page(&mut self, cx: &mut Context<Self>) {
        self._logged_in = Some(match &self.page {
            Page::Login(login) => cx.subscribe(login, |this, _, _: &LoggedIn, cx| {
                this.page = Page::Home(cx.new(|_| Home));
                this.watch_page(cx);
                cx.notify();
            }),
            Page::Home(home) => cx.subscribe(home, |this, _, _: &LoggedOut, cx| {
                this.page = Page::Login(cx.new(|_| Login::new()));
                this.watch_page(cx);
                cx.notify();
            }),
        });
    }
}

impl Render for Root {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let (page, logout) = match &self.page {
            Page::Login(login) => (login.clone().into_any_element(), None),
            Page::Home(home) => (
                home.clone().into_any_element(),
                Some(logout_button(home.clone())),
            ),
        };
        div()
            .relative()
            .v_flex()
            .size_full()
            .text_color(rgb(0xffffff))
            .child(self.chrome.clone())
            .child(page)
            .children(logout)
    }
}

fn main() {
    let app = gpui_platform::application();

    app.run(|cx| {
        gpui_base::init(cx);

        for placement in frame::restore(DEFAULT_WINDOW_SIZE, cx) {
            let window_id = placement.window_id;
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(placement.bounds)),
                display_id: placement.display_id,
                window_min_size: Some(MIN_WINDOW_SIZE),
                titlebar: Some(TitlebarOptions {
                    appears_transparent: true,
                    traffic_light_position: Some(point(px(12.0), px(16.0))),
                    ..Default::default()
                }),
                app_owns_titlebar_drag: true,
                ..Default::default()
            };
            cx.open_window(options, move |window, cx| {
                cx.new(|cx| Root::new(window_id, window, cx))
            })
            .expect("Failed to open window");
        }
    });
}
