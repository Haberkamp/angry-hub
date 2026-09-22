use gpui::prelude::*;
use gpui::{
    Context, Entity, IntoElement, Pixels, Render, Size, TitlebarOptions, Window, WindowBounds,
    WindowOptions, div, point, px, rgb, size,
};
use gpui_base::{Button, StyledExt as _};

mod chrome;
mod color;
mod frame;

use chrome::Chrome;

const DEFAULT_WINDOW_SIZE: Size<Pixels> = size(px(800.0), px(600.0));
const MIN_WINDOW_SIZE: Size<Pixels> = size(px(480.0), px(600.0));

struct Root {
    count: i32,
    chrome: Entity<Chrome>,
}

impl Root {
    fn new(window_id: u64, window: &mut Window, cx: &mut Context<Self>) -> Self {
        frame::observe(window_id, window, cx);
        Self {
            count: 0,
            chrome: cx.new(|_| Chrome::new(DEFAULT_WINDOW_SIZE)),
        }
    }
}

impl Render for Root {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();

        div()
            .v_flex()
            .size_full()
            .text_color(rgb(0xffffff))
            .child(self.chrome.clone())
            .child(
                div()
                    .flex_1()
                    .v_flex()
                    .gap_2()
                    .items_center()
                    .justify_center()
                    .child(format!("Count: {}", self.count))
                    .child(
                        Button::new("increment")
                            .px_3()
                            .py_2()
                            .rounded(px(6.))
                            .bg(rgb(0x2563eb))
                            .text_color(rgb(0xffffff))
                            .on_click(move |_, _, cx| {
                                entity.update(cx, |this, cx| {
                                    this.count += 1;
                                    cx.notify();
                                });
                            })
                            .child("Increment"),
                    ),
            )
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
