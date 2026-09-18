use gpui::{px, rgb, svg, App, Hsla, IntoElement, Pixels, RenderOnce, Styled, Window};

#[derive(Clone, Copy)]
pub enum IconName {
    ChevronDown,
    Logout,
    PullRequest,
    PrClosed,
    PrDraft,
    CiCheck,
    CiX,
    CiPending,
    NoCi,
}

impl IconName {
    fn path(self) -> &'static str {
        match self {
            IconName::ChevronDown => "icons/chevron_down.svg",
            IconName::Logout => "icons/logout.svg",
            IconName::PullRequest => "icons/pull_request.svg",
            IconName::PrClosed => "icons/pr_closed.svg",
            IconName::PrDraft => "icons/pr_draft.svg",
            IconName::CiCheck => "icons/check.svg",
            IconName::CiX => "icons/x.svg",
            IconName::CiPending => "icons/pending.svg",
            IconName::NoCi => "icons/pending.svg",
        }
    }
}

#[derive(Clone, IntoElement)]
pub struct Icon {
    name: IconName,
    size: Option<Pixels>,
    color: Option<Hsla>,
}

impl Icon {
    pub fn new(name: IconName) -> Self {
        Self {
            name,
            size: None,
            color: None,
        }
    }

    pub fn size(mut self, size: impl Into<Pixels>) -> Self {
        self.size = Some(size.into());
        self
    }

    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.color = Some(color.into());
        self
    }
}

impl RenderOnce for Icon {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let size = self.size.unwrap_or_else(|| px(16.0));
        // GPUI only paints an `svg()` when `style.text.color` is set on that
        // element itself — parent `text_color` is not enough.
        let color = self.color.unwrap_or_else(|| rgb(0xffffff).into());
        svg()
            .path(self.name.path())
            .size(size)
            .flex_none()
            .text_color(color)
            .into_any_element()
    }
}