use crate::icon::{Icon, IconName};
use crate::model::{CiStatus, PrStatus};
use gpui::{px, IntoElement, ParentElement, RenderOnce, Styled, Window};

#[derive(Clone, IntoElement)]
pub struct PrStatusIcon {
    status: PrStatus,
}

impl PrStatusIcon {
    pub fn new(status: PrStatus) -> Self {
        Self { status }
    }

    fn icon(&self) -> IconName {
        match self.status {
            PrStatus::Open => IconName::PullRequest,
            PrStatus::Draft => IconName::PrDraft,
            PrStatus::Merged => IconName::PullRequest,
            PrStatus::Closed => IconName::PrClosed,
        }
    }
}

impl RenderOnce for PrStatusIcon {
    fn render(self, _window: &mut Window, _cx: &mut gpui::App) -> impl IntoElement {
        let color = self.status.color();
        Icon::new(self.icon()).size(px(20.0)).color(color)
    }
}

#[derive(Clone, IntoElement)]
pub struct PrStatusLabel {
    status: PrStatus,
}

impl PrStatusLabel {
    pub fn new(status: PrStatus) -> Self {
        Self { status }
    }
}

impl RenderOnce for PrStatusLabel {
    fn render(self, _window: &mut Window, _cx: &mut gpui::App) -> impl IntoElement {
        gpui::div()
            .text_color(self.status.color())
            .child(self.status.label())
    }
}

#[derive(Clone, IntoElement)]
pub struct CiStatusLabel {
    status: CiStatus,
}

impl CiStatusLabel {
    pub fn new(status: CiStatus) -> Self {
        Self { status }
    }

    fn icon(&self) -> IconName {
        match self.status {
            CiStatus::Success => IconName::CiCheck,
            CiStatus::Failure => IconName::CiX,
            CiStatus::Pending => IconName::CiPending,
            CiStatus::None => IconName::NoCi,
        }
    }
}

impl RenderOnce for CiStatusLabel {
    fn render(self, _window: &mut Window, _cx: &mut gpui::App) -> impl IntoElement {
        if self.status == CiStatus::None {
            return gpui::div().into_any_element();
        }
        gpui::div()
            .flex()
            .items_center()
            .gap_1()
            .text_color(gpui::rgb(0x8b949e))
            .child(
                Icon::new(self.icon())
                    .size(px(14.0))
                    .color(self.status.color()),
            )
            .child(self.status.label())
            .into_any_element()
    }
}