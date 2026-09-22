use gpui::prelude::*;
use gpui::{Context, IntoElement, Render, Window, WindowOptions, div, px, rgb};
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
            cx.open_window(WindowOptions::default(), |_window, cx| {
                cx.new(|_| Counter { count: 0 })
            })
            .expect("Failed to open window");
        })
        .detach();
    });
}
