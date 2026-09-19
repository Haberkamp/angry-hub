use gpui::Context;

use crate::route::Guarded;

/// Browser-style navigation stack for a single window.
///
/// `R` is the app's route type. This type does not know what pages exist
/// or what guard names mean. Callers pass `allow(name)` on each change.
pub struct Router<R> {
    history: Vec<R>,
    index: usize,
    previous: Option<R>,
}

impl<R: Clone + PartialEq + Guarded + 'static> Router<R> {
    pub fn new(initial: R, allow: impl Fn(&str) -> bool) -> Self {
        Self {
            history: vec![initial.apply_guards(allow)],
            index: 0,
            previous: None,
        }
    }

    pub fn current(&self) -> &R {
        &self.history[self.index]
    }

    /// Route shown before the last successful navigation, if any.
    pub fn previous(&self) -> Option<&R> {
        self.previous.as_ref()
    }

    /// Most recent matching route in the stack (including current).
    pub fn find_last(&self, pred: impl Fn(&R) -> bool) -> Option<R> {
        self.history.iter().rev().find(|route| pred(route)).cloned()
    }

    pub fn can_go_back(&self) -> bool {
        self.index > 0
    }

    pub fn can_go_forward(&self) -> bool {
        self.index + 1 < self.history.len()
    }

    /// Push a new route and drop any forward entries.
    pub fn navigate(&mut self, route: R, allow: impl Fn(&str) -> bool, cx: &mut Context<Self>) {
        let route = route.apply_guards(allow);
        if self.current() == &route {
            return;
        }
        self.previous = Some(self.current().clone());
        self.history.truncate(self.index + 1);
        self.history.push(route);
        self.index += 1;
        cx.notify();
    }

    /// Swap the current entry without growing history.
    pub fn replace(&mut self, route: R, allow: impl Fn(&str) -> bool, cx: &mut Context<Self>) {
        let route = route.apply_guards(allow);
        if self.current() == &route {
            return;
        }
        self.previous = Some(self.current().clone());
        self.history[self.index] = route;
        cx.notify();
    }

    pub fn back(&mut self, allow: impl Fn(&str) -> bool, cx: &mut Context<Self>) {
        if !self.can_go_back() {
            return;
        }
        self.previous = Some(self.current().clone());
        self.index -= 1;
        self.apply_current(allow, cx);
    }

    pub fn forward(&mut self, allow: impl Fn(&str) -> bool, cx: &mut Context<Self>) {
        if !self.can_go_forward() {
            return;
        }
        self.previous = Some(self.current().clone());
        self.index += 1;
        self.apply_current(allow, cx);
    }

    pub fn reset(&mut self, route: R, allow: impl Fn(&str) -> bool, cx: &mut Context<Self>) {
        let route = route.apply_guards(allow);
        self.previous = None;
        self.history.clear();
        self.history.push(route);
        self.index = 0;
        cx.notify();
    }

    /// Re-run guards on the current entry (e.g. after session changes).
    /// Replaces the stack when the resolved route differs so Back cannot
    /// return to a now-illegal page.
    pub fn enforce(&mut self, allow: impl Fn(&str) -> bool, cx: &mut Context<Self>) {
        let dest = self.current().apply_guards(&allow);
        if dest == *self.current() {
            return;
        }
        self.reset(dest, allow, cx);
    }

    fn apply_current(&mut self, allow: impl Fn(&str) -> bool, cx: &mut Context<Self>) {
        let dest = self.current().apply_guards(allow);
        if dest != *self.current() {
            self.history[self.index] = dest;
        }
        cx.notify();
    }
}
