use crate::context_menu::{ContextMenu, ContextMenuItem};
use crate::model::{CiStatus, PrStatus, PullRequest};
use crate::pr_status::{CiStatusIcon, PrStatusIcon};
use gpui::{
    App, ClickEvent, ElementId, InteractiveElement, IntoElement, MouseDownEvent, ParentElement,
    Pixels, RenderOnce, SharedString, Styled, Window, div, prelude::*, px, rgb,
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
    menu_open: bool,
    closing: bool,
    on_toggle_menu: Option<ToggleMenuHandler>,
    on_dismiss_menu: Option<DismissMenuHandler>,
    on_close_pr: Option<ClosePrHandler>,
}

fn truncated_title(title: SharedString, window: &mut Window) -> SharedString {
    let list_width = window.viewport_size().width.min(px(560.0));
    let reserved = px(16.0) // pr-list px_4
        + px(16.0)
        + px(12.0) // pr item px_3
        + px(12.0)
        + px(8.0) // gap before context menu
        + px(28.0) // context menu button
        + px(20.0) // status icon
        + px(8.0); // gap after icon
    let max_width: Pixels = (list_width - reserved).max(px(48.0));

    let text_style = window.text_style();
    let font_size = text_style.font_size.to_pixels(window.rem_size());
    let mut runs = vec![text_style.to_run(title.len())];
    window
        .text_system()
        .line_wrapper(text_style.font(), font_size)
        .truncate_line(title, max_width, "...", &mut runs)
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
            title: pr.title.clone().into(),
            repo: pr.repo.clone().into(),
            number: pr_number_label(pr).into(),
            url: pr.url.clone().into(),
            status: pr.status().clone(),
            ci: pr.ci,
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
            .hover(|this| this.bg(rgb(0x2a2a2a)))
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
                    ),
            )
            .child(menu)
    }
}
