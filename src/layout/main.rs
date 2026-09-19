use gpui::{
    Context, Entity, IntoElement, ParentElement, PromptLevel, Styled, Window, div, prelude::*, px,
};

use crate::code_host;
use crate::route::Route;
use crate::router::Router;
use crate::session::Session;
use crate::ui::{Button, Icon, IconName, Segment, SegmentedControl, Spinner};

/// Logged-in chrome around a page (section switcher, logout, scroll column).
pub fn main_layout<V: 'static>(
    router: Entity<Router<Route>>,
    session: Entity<Session>,
    refreshing: bool,
    outlet: impl IntoElement,
    cx: &mut Context<V>,
) -> impl IntoElement {
    let route = router.read(cx).current().clone();
    let section_id = route.section_id();
    let previous_section_id = router
        .read(cx)
        .previous()
        .map(Route::section_id)
        .unwrap_or(section_id);

    div()
        .id("main-layout")
        .size_full()
        .flex()
        .flex_col()
        .relative()
        .child(
            div()
                .id("pr-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .items_center()
                .child(
                    div()
                        .id("pr-list")
                        .w_full()
                        .max_w(px(560.0))
                        .flex()
                        .flex_col()
                        .flex_1()
                        .px_4()
                        .pt_16()
                        .pb_16()
                        .child(
                            div()
                                .id("view-switcher")
                                .flex()
                                .flex_none()
                                .pl_3()
                                .pb(px(32.0))
                                .map(|mut this| {
                                    this.style().align_self = Some(gpui::AlignItems::FlexStart);
                                    this
                                })
                                .child({
                                    let router = router.clone();
                                    let session = session.clone();
                                    SegmentedControl::new("main-view")
                                        .segments([
                                            Segment::new("prs", "Pull Requests"),
                                            Segment::new("activity", "Activity"),
                                        ])
                                        .selected(section_id)
                                        .previous_selected(previous_section_id)
                                        .on_change(cx.listener(move |_, id, _, cx| {
                                            let logged_in = session.read(cx).logged_in;
                                            router.update(cx, |router, cx| {
                                                let route = if id == "activity" {
                                                    Route::Activity
                                                } else {
                                                    router
                                                        .find_last(Route::is_pull_requests)
                                                        .unwrap_or_else(Route::pull_requests)
                                                };
                                                router.navigate(
                                                    route,
                                                    Session::allow(logged_in),
                                                    cx,
                                                );
                                            });
                                        }))
                                }),
                        )
                        .child(outlet),
                ),
        )
        .child(
            div()
                .id("top-right")
                .absolute()
                .top_4()
                .right_4()
                .flex()
                .items_center()
                .gap_2()
                .when(refreshing, |this| {
                    this.child(div().id("refreshing").child(Spinner::new("refreshing")))
                })
                .child({
                    let session = session.clone();
                    let router = router.clone();
                    Button::new("logout", "")
                        .icon(Icon::new(IconName::Logout).size(px(20.0)))
                        .tertiary()
                        .on_click(cx.listener(move |_, _, window, cx| {
                            logout(session.clone(), router.clone(), window, cx);
                        }))
                }),
        )
}

fn logout<V: 'static>(
    session: Entity<Session>,
    router: Entity<Router<Route>>,
    window: &mut Window,
    cx: &mut Context<V>,
) {
    let answer = window.prompt(
        PromptLevel::Warning,
        "Are you sure you want to log out?",
        None,
        &["Logout", "Cancel"],
        cx,
    );
    let host = code_host();
    cx.spawn(async move |this, cx| {
        if answer.await == Ok(0) {
            this.update(cx, |_, cx| {
                host.logout();
                session.update(cx, |session, cx| {
                    session.logged_in = false;
                    cx.notify();
                });
                router.update(cx, |router, cx| {
                    router.enforce(Session::allow(false), cx);
                });
            })
            .ok();
        }
    })
    .detach();
}
