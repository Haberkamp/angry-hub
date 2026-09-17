use gpui::{
    div, prelude::*, px, rgb, App, ClickEvent, Hsla, InteractiveElement, IntoElement,
    ParentElement, RenderOnce, SharedString, Styled, Window,
};

fn h(c: u32) -> Hsla {
    rgb(c).into()
}

#[derive(IntoElement)]
pub struct Tab {
    id: SharedString,
    label: SharedString,
    selected: bool,
    on_click: Option<Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>>,
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

    fn colors(&self) -> (Hsla, Hsla) {
        if self.selected {
            (h(0x3d3d3d), h(0xffffff))
        } else {
            (h(0x2a2a2a), h(0xaaaaaa))
        }
    }
}

impl RenderOnce for Tab {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let (bg, text) = self.colors();
        let label = self.label.clone();
        let mut element = div()
            .id(self.id)
            .px_3()
            .py_1()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .bg(bg)
            .hover(|this| this.bg(h(0x3d3d3d)))
            .rounded_full()
            .cursor_pointer()
            .text_size(px(13.0))
            .text_color(text)
            .child(label);

        if let Some(on_click) = self.on_click {
            element = element.on_click(move |event: &ClickEvent, window, cx| {
                on_click(event, window, cx)
            });
        }

        element
    }
}