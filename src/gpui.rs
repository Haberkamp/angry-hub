use gpui::prelude::*;
use gpui::{
    Context, Entity, FocusHandle, IntoElement, Pixels, Render, Size, TitlebarOptions, Window,
    WindowBounds, WindowOptions, div, point, px, rgb, size,
};
use gpui_base::StyledExt as _;

mod auth;
mod button;
mod chrome;
mod dropdown;
mod color;
mod frame;
mod github;
mod icon;
mod paths;
mod pulls;
mod sync;
mod toast;
mod keychain;
mod menu;
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
    focus_handle: FocusHandle,
    chrome: Entity<Chrome>,
    page: Page,
    _logged_in: Option<gpui::Subscription>,
}

impl Root {
    fn new(window_id: u64, window: &mut Window, cx: &mut Context<Self>) -> Self {
        frame::observe(window_id, window, cx);
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        let chrome = cx.new(|_| Chrome::new(DEFAULT_WINDOW_SIZE));
        if auth::Auth::load(&keychain::CredentialStore).check() {
            let mut root = Self {
                focus_handle,
                chrome,
                page: Page::Home(cx.new(|cx| Home::new(cx))),
                _logged_in: None,
            };
            root.watch_page(cx);
            return root;
        }

        let mut root = Self {
            focus_handle,
            chrome,
            page: Page::Login(cx.new(|_| Login::new())),
            _logged_in: None,
        };
        root.watch_page(cx);
        root
    }

    fn quit(&mut self, _: &menu::Quit, _: &mut Window, cx: &mut Context<Self>) {
        cx.quit();
    }

    fn hide_app(&mut self, _: &menu::HideApp, _: &mut Window, cx: &mut Context<Self>) {
        cx.hide();
    }

    fn hide_others(&mut self, _: &menu::HideOthers, _: &mut Window, cx: &mut Context<Self>) {
        cx.hide_other_apps();
    }

    fn show_all(&mut self, _: &menu::ShowAll, _: &mut Window, cx: &mut Context<Self>) {
        cx.unhide_other_apps();
    }

    fn close_window(&mut self, _: &menu::CloseWindow, window: &mut Window, _: &mut Context<Self>) {
        window.remove_window();
    }

    fn minimize_window(
        &mut self,
        _: &menu::MinimizeWindow,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        window.minimize_window();
    }

    fn zoom_window(&mut self, _: &menu::ZoomWindow, window: &mut Window, _: &mut Context<Self>) {
        window.zoom_window();
    }

    fn toggle_full_screen(
        &mut self,
        _: &menu::ToggleFullScreen,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        window.toggle_fullscreen();
    }

    fn watch_page(&mut self, cx: &mut Context<Self>) {
        self._logged_in = Some(match &self.page {
            Page::Login(login) => cx.subscribe(login, |this, _, _: &LoggedIn, cx| {
                this.page = Page::Home(cx.new(|cx| Home::new(cx)));
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
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (page, logout) = match &self.page {
            Page::Login(login) => (login.clone().into_any_element(), None),
            Page::Home(home) => (
                home.clone().into_any_element(),
                Some(logout_button(home.clone())),
            ),
        };
        div()
            .id("root")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::quit))
            .on_action(cx.listener(Self::hide_app))
            .on_action(cx.listener(Self::hide_others))
            .on_action(cx.listener(Self::show_all))
            .on_action(cx.listener(Self::close_window))
            .on_action(cx.listener(Self::minimize_window))
            .on_action(cx.listener(Self::zoom_window))
            .on_action(cx.listener(Self::toggle_full_screen))
            .relative()
            .v_flex()
            .size_full()
            .bg(color::gray(1, cx))
            .text_color(rgb(0xffffff))
            .child(page)
            .child(self.chrome.clone())
            .children(logout)
    }
}

fn open_window(cx: &mut gpui::App) {
    let Some(placement) = frame::restore(DEFAULT_WINDOW_SIZE, cx).into_iter().next() else {
        return;
    };
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

fn main() {
    let app = gpui_platform::application();

    app.on_reopen(|cx| {
        if cx.windows().is_empty() {
            open_window(cx);
        }
    });
    app.run(|cx| {
        gpui_base::init(cx);
        menu::init(cx);
        open_window(cx);
    });
}
