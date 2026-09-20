use crate::color;
use gpui::{
    App, ClickEvent, Hsla, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    SharedString, Styled, Window, div, prelude::*, px,
};

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct Tab {
    id: SharedString,
    label: SharedString,
    selected: bool,
    on_click: Option<ClickHandler>,
}

impl Tab {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            selected: false,
            on_click: None,
        }
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn on_click(
        mut self,
        listener: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(listener));
        self
    }

    fn colors(&self, cx: &App) -> (Hsla, Hsla) {
        if self.selected {
            (color::interaction::pressed(cx), color::text::primary(cx))
        } else {
            (color::surface::sunken(cx), color::text::secondary(cx))
        }
    }
}

impl RenderOnce for Tab {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let (bg, text) = self.colors(cx);
        let hover = color::interaction::pressed(cx);
        let label = self.label.clone();
        let mut element = div()
            .id(self.id)
            .px_3()
            .py_1()
            .h(px(28.0))
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .bg(bg)
            .hover(|this| this.bg(hover))
            .rounded_full()
            .cursor_pointer()
            .text_size(px(13.0))
            .text_color(text)
            .child(label);

        if let Some(on_click) = self.on_click {
            element =
                element.on_click(move |event: &ClickEvent, window, cx| on_click(event, window, cx));
        }

        element
    }
}
