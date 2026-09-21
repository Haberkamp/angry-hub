use crate::ui::color;
use gpui::{
    AnyElement, App, IntoElement, ParentElement, Pixels, RenderOnce, Styled, Window, div, img,
    prelude::*, px,
};

use super::icon::{Icon, IconName};

#[derive(IntoElement)]
pub struct Avatar {
    url: Option<String>,
    size: Pixels,
}

impl Avatar {
    pub fn new(url: impl Into<Option<String>>) -> Self {
        Self {
            url: url.into(),
            size: px(20.0),
        }
    }

    pub fn size(mut self, size: impl Into<Pixels>) -> Self {
        self.size = size.into();
        self
    }

    fn placeholder(size: Pixels, bg: gpui::Hsla, icon_color: gpui::Hsla) -> AnyElement {
        let icon_size = px(f32::from(size) * 0.6);
        div()
            .size(size)
            .rounded_full()
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .bg(bg)
            .child(Icon::new(IconName::User).size(icon_size).color(icon_color))
            .into_any_element()
    }
}

impl RenderOnce for Avatar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let size = self.size;
        let bg = color::interaction::pressed(cx);
        let icon_color = color::icon::subtle(cx);
        match self.url {
            Some(url) => img(url)
                .size(size)
                .rounded_full()
                .flex_none()
                .with_loading(move || Avatar::placeholder(size, bg, icon_color))
                .with_fallback(move || Avatar::placeholder(size, bg, icon_color))
                .into_any_element(),
            None => Avatar::placeholder(size, bg, icon_color),
        }
    }
}
