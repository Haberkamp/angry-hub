use super::pr_status::{CiStatusIcon, PrStatusIcon};
use super::shared::{ContextMenu, ContextMenuItem, list_text_max_width, truncate_line};
use crate::color;
use crate::model::{CiStatus, PrStatus, PullRequest};
use gpui::{
    App, ClickEvent, ElementId, InteractiveElement, IntoElement, MouseDownEvent, ParentElement,
    RenderOnce, SharedString, Styled, Window, div, prelude::*, px,
};

type ToggleMenuHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;
type DismissMenuHandler = Box<dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static>;
type ClosePrHandler = Box<dyn Fn(&mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct PrItem {
    id: ElementId,
    open_id: SharedString,
    menu_id: SharedString,
    group: SharedString,
    title: SharedString,
    repo: SharedString,
    number: SharedString,
    url: SharedString,
    status: PrStatus,
    ci: CiStatus,
    approvals: SharedString,
    required_approvals: SharedString,
    menu_open: bool,
    closing: bool,
    on_toggle_menu: Option<ToggleMenuHandler>,
    on_dismiss_menu: Option<DismissMenuHandler>,
    on_close_pr: Option<ClosePrHandler>,
}

fn truncated_title(title: SharedString, window: &mut Window) -> SharedString {
    let extra_reserved = px(8.0) // gap before context menu
        + px(28.0) // context menu button
        + px(20.0) // status icon
        + px(8.0); // gap after icon
    let max_width = list_text_max_width(window, extra_reserved);
    truncate_line(title, max_width, window)
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
            open_id: SharedString::from(format!("pr-open-{ix}")),
            menu_id: SharedString::from(format!("pr-menu-{ix}")),
            group: SharedString::from(format!("pr-row-{ix}")),
            title: pr.title.trim().to_string().into(),
            repo: pr.repo.clone().into(),
            number: pr_number_label(pr).into(),
            url: pr.url.clone().into(),
            status: pr.status().clone(),
            ci: pr.ci,
            approvals: pr.approvals.to_string().into(),
            required_approvals: pr.required_approvals.to_string().into(),
            menu_open: false,
            closing: false,
            on_toggle_menu: None,
            on_dismiss_menu: None,
            on_close_pr: None,
        }
    }

    pub fn menu_open(mut self, open: bool) -> Self {
        self.menu_open = open;
        self
    }

    pub fn closing(mut self, closing: bool) -> Self {
        self.closing = closing;
        self
    }

    pub fn on_toggle_menu(
        mut self,
        listener: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle_menu = Some(Box::new(listener));
        self
    }

    pub fn on_dismiss_menu(
        mut self,
        listener: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_dismiss_menu = Some(Box::new(listener));
        self
    }

    pub fn on_close_pr(mut self, listener: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close_pr = Some(Box::new(listener));
        self
    }
}

impl RenderOnce for PrItem {
    fn render(self, window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let url = self.url.clone();
        let menu_id = self.menu_id;
        let title = truncated_title(self.title, window);

        let mut menu = ContextMenu::new(menu_id)
            .open(self.menu_open)
            .hover_group(self.group.clone())
            .items(vec![
                ContextMenuItem::new("close", "Close pull request").loading(self.closing),
            ]);
        if let Some(on_toggle_menu) = self.on_toggle_menu {
            menu = menu.on_toggle_open(on_toggle_menu);
        }
        if let Some(on_dismiss_menu) = self.on_dismiss_menu {
            menu = menu.on_dismiss(on_dismiss_menu);
        }
        if let Some(on_close_pr) = self.on_close_pr {
            menu = menu.on_select(move |item_id, window, cx| {
                if item_id == "close" {
                    on_close_pr(window, cx);
                }
            });
        }

        div()
            .id(self.id)
            .group(self.group)
            .w_full()
            .min_w_0()
            .flex()
            .items_center()
            .gap_2()
            .py_2()
            .px_3()
            .rounded_md()
            .hover(|this| this.bg(color::gray::s3()))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .min_w_0()
                    .id(self.open_id)
                    .cursor_pointer()
                    .on_click(move |_, _window, cx| cx.open_url(&url))
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .items_center()
                            .min_w_0()
                            .child(PrStatusIcon::new(self.status.clone()))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .child(title),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .items_center()
                            .ml(px(28.0))
                            .text_size(px(12.0))
                            .text_color(color::gray::s10())
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(4.0))
                                    .child(self.repo)
                                    .child(self.number)
                                    .child("·")
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(4.0))
                                            .child("Approvals")
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap(px(2.0))
                                                    .child(self.approvals)
                                                    .child("/")
                                                    .child(self.required_approvals),
                                            ),
                                    ),
                            )
                            .child(CiStatusIcon::new(self.ci)),
                    ),
            )
            .child(menu)
    }
}
