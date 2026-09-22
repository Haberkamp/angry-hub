use gpui::prelude::*;
use gpui::{
    Bounds, Context, IntoElement, MouseButton, Render, TitlebarOptions, Window, WindowBounds,
    WindowOptions, div, point, px, rgb, size,
};
use gpui_base::{Button, StyledExt as _};

mod color;

struct Root {
    count: i32,
}

impl Root {
    fn drag_area(&self) -> impl IntoElement {
        div()
            .w_full()
            .h(px(28.0))
            .on_mouse_down(MouseButton::Left, |_, window, _| {
                window.start_window_move();
            })
    }
}

impl Render for Root {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();

        div()
            .v_flex()
            .size_full()
            .text_color(rgb(0xffffff))
            .child(self.drag_area())
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

        cx.spawn(async move |cx| {
            let options = cx.update(|cx| WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(800.0), px(600.0)),
                    cx,
                ))),
                window_min_size: Some(size(px(480.0), px(600.0))),
                titlebar: Some(TitlebarOptions {
                    appears_transparent: true,
                    traffic_light_position: Some(point(px(12.0), px(16.0))),
                    ..Default::default()
                }),
                app_owns_titlebar_drag: true,
                ..Default::default()
            });

            cx.open_window(options, |_window, cx| cx.new(|_| Root { count: 0 }))
                .expect("Failed to open window");
        })
        .detach();
    });
}
