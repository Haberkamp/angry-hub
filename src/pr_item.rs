use crate::model::{CiStatus, PrStatus, PullRequest};
use crate::pr_status::{CiStatusIcon, PrStatusIcon};
use gpui::{
    App, ElementId, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    Styled, Window, div, prelude::*, px, rgb,
};

#[derive(IntoElement)]
pub struct PrItem {
    id: ElementId,
    title: SharedString,
    repo: SharedString,
    number: SharedString,
    url: SharedString,
    status: PrStatus,
    ci: CiStatus,
}

fn pr_number_label(pr: &PullRequest) -> String {
    let number = if pr.number != 0 {
        pr.number
    } else {
        pr.url
            .rsplit('/')
            .next()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0)
    };
    format!("#{number}")
}

impl PrItem {
    pub fn new(ix: usize, pr: &PullRequest) -> Self {
        Self {
            id: ElementId::from(("pr", ix)),
            title: pr.title.clone().into(),
            repo: pr.repo.clone().into(),
            number: pr_number_label(pr).into(),
            url: pr.url.clone().into(),
            status: pr.status().clone(),
            ci: pr.ci,
        }
    }
}

impl RenderOnce for PrItem {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let url = self.url.clone();
        div()
            .id(self.id)
            .flex()
            .flex_col()
            .gap_1()
            .py_2()
            .px_3()
            .rounded_md()
            .hover(|this| this.bg(rgb(0x2a2a2a)))
            .cursor_pointer()
            .on_click(move |_, _window, cx| cx.open_url(&url))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .items_center()
                    .child(PrStatusIcon::new(self.status.clone()))
                    .child(div().child(self.title)),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .items_center()
                    .ml(px(28.0))
                    .text_size(px(12.0))
                    .text_color(rgb(0x8b949e))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .child(self.repo)
                            .child("·")
                            .child(self.number),
                    )
                    .child(CiStatusIcon::new(self.ci)),
            )
    }
}
