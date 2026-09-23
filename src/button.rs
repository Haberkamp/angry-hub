use gpui::prelude::*;
use gpui::{App, ClickEvent, Hsla, IntoElement, RenderOnce, Window, div, px, rgb, svg};

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct Button {
    id: &'static str,
    primary: bool,
    label: &'static str,
    icon: Option<&'static [u8]>,
    full: bool,
    on_click: Option<ClickHandler>,
}

impl Button {
    pub fn primary(id: &'static str) -> Self {
        Self::new(id, true)
    }

    pub fn secondary(id: &'static str) -> Self {
        Self::new(id, false)
    }

    fn new(id: &'static str, primary: bool) -> Self {
        Self {
            id,
            primary,
            label: "",
            icon: None,
            full: false,
            on_click: None,
        }
    }

    pub fn label(mut self, label: &'static str) -> Self {
        self.label = label;
        self
    }

    pub fn icon(mut self, icon: &'static [u8]) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn full(mut self) -> Self {
        self.full = true;
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for Button {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let primary = self.primary;
        let text = if primary { gray(1) } else { gray(9) };
        let hover_bg = if primary { gray(11) } else { gray(3) };
        let active_bg = if primary { gray(10) } else { gray(5) };
        let icon = self.icon;
        let id = self.id;
        let label = self.label;
        let on_click = self.on_click;
        let full = self.full;

        let mut button = div()
            .id(id)
            .group(id)
            .px_4()
            .py_2()
            .when(full, |this| this.w_full())
            .flex()
            .items_center()
            .justify_center()
            .gap_2()
            .rounded_md()
            .text_color(text)
            .when(primary, |this| this.bg(gray(12)))
            .hover(move |style| {
                let style = style.bg(hover_bg);
                if primary {
                    style
                } else {
                    style.text_color(rgb(0xffffff))
                }
            })
            .active(move |style| style.bg(active_bg))
            .children(icon.map(|icon| {
                svg()
                    .data(icon)
                    .size(px(16.))
                    .text_color(text)
                    .when(!primary, |icon| {
                        icon.group_hover(id, |style| style.text_color(rgb(0xffffff)))
                    })
            }))
            .child(label);

        if let Some(on_click) = on_click {
            button = button.on_click(move |event, window, cx| on_click(event, window, cx));
        }
        button
    }
}

fn gray(step: usize) -> Hsla {
    const DARK: [u32; 12] = [
        0x111111, 0x191919, 0x222222, 0x2A2A2A, 0x313131, 0x3A3A3A, 0x484848, 0x606060, 0x6E6E6E,
        0x7B7B7B, 0xB4B4B4, 0xEEEEEE,
    ];
    let index = step.saturating_sub(1).min(11);
    rgb(DARK[index]).into()
}
