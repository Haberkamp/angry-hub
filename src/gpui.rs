use gpui::prelude::*;
use gpui::{
    Bounds, Context, IntoElement, Render, Window, WindowBounds, WindowOptions, div, px, rgb, size,
};
use gpui_base::{Button, StyledExt as _};

struct Counter {
    count: i32,
}

impl Render for Counter {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();

        div()
            .v_flex()
            .gap_2()
            .size_full()
            .items_center()
            .justify_center()
            .text_color(rgb(0xffffff))
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
            )
    }
}

fn main() {
    let app = gpui_platform::application();

    app.run(|cx| {
        gpui_base::init(cx);

        cx.spawn(async move |cx| {
            let options = cx.update(|cx| WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(800.0), px(600.0)),
                    cx,
                ))),
                window_min_size: Some(size(px(480.0), px(600.0))),
                ..Default::default()
            });

            cx.open_window(options, |_window, cx| cx.new(|_| Counter { count: 0 }))
                .expect("Failed to open window");
        })
        .detach();
    });
}
