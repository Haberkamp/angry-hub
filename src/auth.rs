#![allow(dead_code)]

use crate::github::Github;
use crate::session::{Session, Tokens};

pub struct Auth {
    session: Session,
}

impl Auth {
    pub fn new() -> Self {
        Self {
            session: Session::new(),
        }
    }

    pub fn login(&mut self, tokens: Tokens) {
        self.session.store(tokens);
    }

    pub fn logout(&mut self) {
        self.session.clear();
    }

    pub fn check(&self) -> bool {
        self.session.access_token().is_some()
    }

    pub fn refresh(&mut self, github: &impl Github) -> Result<(), crate::github::GithubError> {
        let Some(refresh_token) = self.session.refresh_token().map(str::to_string) else {
            return Err(crate::github::GithubError::new("no refresh token"));
        };
        let tokens = github.refresh(&refresh_token)?;
        self.login(tokens);
        Ok(())
    }

    pub fn session(&self) -> &Session {
        &self.session
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::FakeGithub;
    use crate::session::Tokens;

    fn tokens() -> Tokens {
        Tokens {
            access_token: "access".into(),
            refresh_token: Some("refresh".into()),
        }
    }

    #[test]
    fn starts_logged_out() {
        let auth = Auth::new();
        assert!(!auth.check());
    }

    #[test]
    fn login_marks_the_user_as_logged_in() {
        let mut auth = Auth::new();
        auth.login(tokens());
        assert!(auth.check());
        assert_eq!(auth.session().access_token(), Some("access"));
        assert_eq!(auth.session().refresh_token(), Some("refresh"));
    }

    #[test]
    fn logout_marks_the_user_as_logged_out() {
        let mut auth = Auth::new();
        auth.login(tokens());
        auth.logout();
        assert!(!auth.check());
        assert_eq!(auth.session().access_token(), None);
        assert_eq!(auth.session().refresh_token(), None);
    }

    #[test]
    fn refresh_replaces_the_stored_tokens() {
        let mut auth = Auth::new();
        auth.login(tokens());
        let github = FakeGithub::with_refresh(Tokens {
            access_token: "access-2".into(),
            refresh_token: Some("refresh-2".into()),
        });

        auth.refresh(&github).unwrap();

        assert!(auth.check());
        assert_eq!(auth.session().access_token(), Some("access-2"));
        assert_eq!(auth.session().refresh_token(), Some("refresh-2"));
    }
}
