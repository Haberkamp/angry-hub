use gpui::{Pixels, SharedString, Window, px};

pub fn list_text_max_width(window: &Window, extra_reserved: Pixels) -> Pixels {
    let list_width = window.viewport_size().width.min(px(560.0));
    let reserved = px(16.0) // pr-list px_4
        + px(16.0)
        + px(12.0) // item px_3
        + px(12.0)
        + extra_reserved;
    (list_width - reserved).max(px(48.0))
}

pub fn measure_line(text: &str, window: &mut Window) -> Pixels {
    let text_style = window.text_style();
    let font_size = text_style.font_size.to_pixels(window.rem_size());
    let run = text_style.to_run(text.len());
    window
        .text_system()
        .layout_line(text, font_size, &[run], None)
        .width
}

pub fn truncate_line(
    text: impl Into<SharedString>,
    max_width: Pixels,
    window: &mut Window,
) -> SharedString {
    let text = text.into();
    let text_style = window.text_style();
    let font_size = text_style.font_size.to_pixels(window.rem_size());
    let mut runs = vec![text_style.to_run(text.len())];
    window
        .text_system()
        .line_wrapper(text_style.font(), font_size)
        .truncate_line(text, max_width, "...", &mut runs)
}
