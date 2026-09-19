use gpui::{
    App, ClickEvent, Hsla, InteractiveElement, IntoElement, ParentElement, RenderOnce, Styled,
    Window, div, prelude::*, rgb,
};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum ButtonVariant {
    #[default]
    Primary,
    Tertiary,
}

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct Button {
    id: gpui::SharedString,
    label: Option<gpui::SharedString>,
    icon: Option<super::icon::Icon>,
    variant: ButtonVariant,
    is_loading: bool,
    on_click: Option<ClickHandler>,
}

struct ButtonStyle {
    bg: Hsla,
    hover_bg: Hsla,
    active_bg: Hsla,
    border: Option<Hsla>,
    text: Hsla,
}

fn h(c: u32) -> Hsla {
    rgb(c).into()
}

impl ButtonVariant {
    fn style(self) -> ButtonStyle {
        match self {
            ButtonVariant::Primary => ButtonStyle {
                bg: h(0x2d2d2d),
                hover_bg: h(0x3d3d3d),
                active_bg: h(0x444444),
                border: Some(h(0x555555)),
                text: h(0xffffff),
            },
            ButtonVariant::Tertiary => ButtonStyle {
                bg: h(0x1e1e1e),
                hover_bg: h(0x333333),
                active_bg: h(0x3a3a3a),
                border: None,
                text: h(0xaaaaaa),
            },
        }
    }
}

impl Button {
    pub fn new(id: impl Into<gpui::SharedString>, label: impl Into<gpui::SharedString>) -> Self {
        let label: gpui::SharedString = label.into();
        Self {
            id: id.into(),
            label: if label.is_empty() { None } else { Some(label) },
            icon: None,
            variant: ButtonVariant::Primary,
            is_loading: false,
            on_click: None,
        }
    }

    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn tertiary(self) -> Self {
        self.variant(ButtonVariant::Tertiary)
    }

    pub fn icon(mut self, icon: super::icon::Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn loading(mut self, is_loading: bool) -> Self {
        self.is_loading = is_loading;
        self
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
        let style = self.variant.style();
        let label = self.label.clone();
        let icon = self.icon.clone();
        let is_loading = self.is_loading;
        let id = self.id.clone();

        let mut element = div()
            .id(self.id)
            .px_4()
            .py_2()
            .flex()
            .items_center()
            .justify_center()
            .gap_2()
            .bg(style.bg)
            .when(!is_loading, |this| {
                this.hover(move |this| this.bg(style.hover_bg))
                    .active(move |this| this.bg(style.active_bg))
                    .cursor_pointer()
            })
            .when(is_loading, |this| this.cursor_default().opacity(0.6))
            .rounded_md()
            .text_color(style.text)
            .children(icon)
            .when(is_loading, |this| {
                this.child(super::spinner::Spinner::new(format!("{}-spinner", id)))
            })
            .children(label);

        if let Some(border) = style.border {
            element = element.border_1().border_color(border);
        }

        if let Some(on_click) = self.on_click
            && !is_loading
        {
            element =
                element.on_click(move |event: &ClickEvent, window, cx| on_click(event, window, cx));
        }

        element
    }
}
