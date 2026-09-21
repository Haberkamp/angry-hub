use super::pr_status::{CiStatusIcon, PrStatusIcon};
use super::shared::{
    ContextMenu, ContextMenuItem, Icon, IconName, Tooltip, list_text_max_width, truncate_line,
};
use crate::models::{CiStatus, PrStatus, PullRequest};
use crate::ui::color;
use gpui::{
    App, ClickEvent, ElementId, InteractiveElement, IntoElement, MouseDownEvent, ParentElement,
    RenderOnce, SharedString, Styled, Window, div, prelude::*, px,
};

type ToggleMenuHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;
type DismissMenuHandler = Box<dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static>;
type ClosePrHandler = Box<dyn Fn(&mut Window, &mut App) + 'static>;
type CopyBranchHandler = Box<dyn Fn(&mut Window, &mut App) + 'static>;

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
    show_approvals: bool,
    has_conflicts: bool,
    conflict_tooltip_id: SharedString,
    ci_tooltip_id: SharedString,
    menu_open: bool,
    closing: bool,
    on_toggle_menu: Option<ToggleMenuHandler>,
    on_dismiss_menu: Option<DismissMenuHandler>,
    on_close_pr: Option<ClosePrHandler>,
    on_copy_branch: Option<CopyBranchHandler>,
}

fn truncated_title(title: SharedString, has_conflicts: bool, window: &mut Window) -> SharedString {
    let extra_reserved = px(8.0) // gap before context menu
        + px(28.0) // context menu button
        + px(20.0) // status icon
        + px(8.0) // gap after icon
        + if has_conflicts {
            px(8.0) + px(16.0) // gap + conflict icon
        } else {
            px(0.0)
        };
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
            show_approvals: pr.required_approvals > 0,
            has_conflicts: pr.has_conflicts,
            conflict_tooltip_id: SharedString::from(format!("pr-conflicts-{ix}")),
            ci_tooltip_id: SharedString::from(format!("pr-ci-{ix}")),
            menu_open: false,
            closing: false,
            on_toggle_menu: None,
            on_dismiss_menu: None,
            on_close_pr: None,
            on_copy_branch: None,
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

    pub fn on_copy_branch(mut self, listener: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_copy_branch = Some(Box::new(listener));
        self
    }
}

impl RenderOnce for PrItem {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let url = self.url.clone();
        let menu_id = self.menu_id;
        let title = truncated_title(self.title, self.has_conflicts, window);

        let conflict_icon = if self.has_conflicts {
            Some(
                Tooltip::new(self.conflict_tooltip_id, "Merge conflicts").child(
                    Icon::new(IconName::MergeConflicts)
                        .size(px(16.0))
                        .color(color::status::failure(cx)),
                ),
            )
        } else {
            None
        };

        let mut menu = ContextMenu::new(menu_id)
            .open(self.menu_open)
            .hover_group(self.group.clone())
            .items(vec![
                ContextMenuItem::new("copy-branch", "Copy branch name"),
                ContextMenuItem::new("close", "Close pull request").loading(self.closing),
            ]);
        if let Some(on_toggle_menu) = self.on_toggle_menu {
            menu = menu.on_toggle_open(on_toggle_menu);
        }
        if let Some(on_dismiss_menu) = self.on_dismiss_menu {
            menu = menu.on_dismiss(on_dismiss_menu);
        }
        if self.on_close_pr.is_some() || self.on_copy_branch.is_some() {
            let on_close_pr = self.on_close_pr;
            let on_copy_branch = self.on_copy_branch;
            menu = menu.on_select(move |item_id, window, cx| match item_id {
                "copy-branch" => {
                    if let Some(on_copy_branch) = &on_copy_branch {
                        on_copy_branch(window, cx);
                    }
                }
                "close" => {
                    if let Some(on_close_pr) = &on_close_pr {
                        on_close_pr(window, cx);
                    }
                }
                _ => {}
            });
        }

        let hover = color::interaction::hovered(cx);
        let meta = color::text::secondary(cx);

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
            .hover(|this| this.bg(hover))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .min_w_0()
                    .id(self.open_id)
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
                                    .flex()
                                    .items_center()
                                    .gap(px(8.0))
                                    .min_w_0()
                                    .child(
                                        div()
                                            .min_w_0()
                                            .overflow_hidden()
                                            .whitespace_nowrap()
                                            .child(title),
                                    )
                                    .children(conflict_icon),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .items_center()
                            .ml(px(28.0))
                            .text_size(px(12.0))
                            .text_color(meta)
                            .child({
                                let mut meta = div()
                                    .flex()
                                    .items_center()
                                    .gap(px(4.0))
                                    .child(self.repo)
                                    .child(self.number);
                                if self.show_approvals {
                                    meta = meta.child("·").child(
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
                                    );
                                }
                                meta
                            })
                            .child(
                                Tooltip::new(
                                    self.ci_tooltip_id,
                                    match self.ci {
                                        CiStatus::Success => "CI passed",
                                        CiStatus::Failure => "CI failed",
                                        CiStatus::Pending => "CI pending",
                                        CiStatus::None => "No CI",
                                    },
                                )
                                .child(CiStatusIcon::new(self.ci)),
                            ),
                    ),
            )
            .child(menu)
    }
}
