use crate::color;
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

    fn placeholder(size: Pixels) -> AnyElement {
        let icon_size = px(f32::from(size) * 0.6);
        div()
            .size(size)
            .rounded_full()
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .bg(color::surface_hover())
            .child(
                Icon::new(IconName::User)
                    .size(icon_size)
                    .color(color::text_muted()),
            )
            .into_any_element()
    }
}

impl RenderOnce for Avatar {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let size = self.size;
        match self.url {
            Some(url) => img(url)
                .size(size)
                .rounded_full()
                .flex_none()
                .with_loading(move || Avatar::placeholder(size))
                .with_fallback(move || Avatar::placeholder(size))
                .into_any_element(),
            None => Avatar::placeholder(size),
        }
    }
}
