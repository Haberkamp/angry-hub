use std::sync::Arc;

/// In-window destination. Nested variants are pages under a shared layout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Route {
    Login,
    PullRequests(PullRequestsPage),
    Activity,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PullRequestsPage {
    All,
    Repo(Arc<str>),
}

/// Named condition plus fallback. The app decides what each name means.
#[derive(Clone, Debug)]
pub struct Guard<R> {
    pub name: &'static str,
    pub redirect: R,
}

pub trait Guarded: Clone {
    fn guard(&self) -> Option<Guard<Self>>;

    fn apply_guards(&self, allow: impl Fn(&str) -> bool) -> Self {
        let mut route = self.clone();
        for _ in 0..16 {
            match route.guard() {
                Some(Guard { name, redirect }) if !allow(name) => route = redirect,
                _ => break,
            }
        }
        route
    }
}

impl Route {
    pub fn pull_requests() -> Self {
        Self::PullRequests(PullRequestsPage::All)
    }

    pub fn is_pull_requests(&self) -> bool {
        matches!(self, Self::PullRequests(_))
    }

    /// Segmented-control id for the main layout section.
    pub fn section_id(&self) -> &'static str {
        match self {
            Self::PullRequests(_) | Self::Login => "prs",
            Self::Activity => "activity",
        }
    }

    pub fn selected_repo(&self) -> Option<&str> {
        match self {
            Self::PullRequests(PullRequestsPage::All) => Some("all"),
            Self::PullRequests(PullRequestsPage::Repo(repo)) => Some(repo.as_ref()),
            Self::Login | Self::Activity => None,
        }
    }
}

impl Guarded for Route {
    fn guard(&self) -> Option<Guard<Self>> {
        match self {
            Self::Login => Some(Guard {
                name: "guest",
                redirect: Self::pull_requests(),
            }),
            Self::PullRequests(_) | Self::Activity => Some(Guard {
                name: "session",
                redirect: Self::Login,
            }),
        }
    }
}
