use gpui::{
    div, prelude::*, rgb, App, ClickEvent, InteractiveElement, IntoElement, ParentElement,
    RenderOnce, Styled, Window,
};

#[derive(IntoElement)]
pub struct Button {
    id: gpui::SharedString,
    label: gpui::SharedString,
    on_click: Option<
        Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>,
    >,
}

impl Button {
    pub fn new(id: impl Into<gpui::SharedString>, label: impl Into<gpui::SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            on_click: None,
        }
    }

    pub fn on_click(
        mut self,
        listener: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(listener));
        self
    }
}

impl RenderOnce for Button {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let label = self.label.clone();
        let mut element = div()
            .id(self.id)
            .px_4()
            .py_2()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgb(0x2d2d2d))
            .hover(|this| this.bg(rgb(0x3d3d3d)))
            .active(|this| this.bg(rgb(0x444444)))
            .border_1()
            .border_color(rgb(0x555555))
            .rounded_md()
            .cursor_pointer()
            .text_color(rgb(0xffffff))
            .child(label);

        if let Some(on_click) = self.on_click {
            element = element.on_click(move |event: &ClickEvent, window, cx| {
                on_click(event, window, cx)
            });
        }

        element
    }
}