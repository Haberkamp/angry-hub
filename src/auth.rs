#![allow(dead_code)]

use crate::github::Github;
use crate::keychain::{self, SecretStore};
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

    pub fn load(store: &impl SecretStore) -> Self {
        let mut auth = Self::new();
        if let Some(secret) = store.load()
            && let Some(tokens) = keychain::decode(&secret)
        {
            auth.session.store(tokens);
        }
        auth
    }

    pub fn login(&mut self, tokens: Tokens, store: &impl SecretStore) -> Result<(), String> {
        store.save(&keychain::encode(&tokens)?)?;
        self.session.store(tokens);
        Ok(())
    }

    pub fn logout(&mut self, store: &impl SecretStore) {
        store.delete();
        self.session.clear();
    }

    pub fn check(&self) -> bool {
        self.session.access_token().is_some()
    }

    pub fn refresh(
        &mut self,
        github: &impl Github,
        store: &impl SecretStore,
    ) -> Result<(), crate::github::GithubError> {
        let Some(refresh_token) = self.session.refresh_token().map(str::to_string) else {
            return Err(crate::github::GithubError::new("no refresh token"));
        };
        let tokens = github.refresh(&refresh_token)?;
        self.login(tokens, store)
            .map_err(crate::github::GithubError::new)?;
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
    use crate::keychain::MemoryStore;
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
        auth.login(tokens(), &MemoryStore::new()).unwrap();
        assert!(auth.check());
        assert_eq!(auth.session().access_token(), Some("access"));
        assert_eq!(auth.session().refresh_token(), Some("refresh"));
    }

    #[test]
    fn logout_marks_the_user_as_logged_out() {
        let store = MemoryStore::new();
        let mut auth = Auth::new();
        auth.login(tokens(), &store).unwrap();
        auth.logout(&store);
        assert!(!auth.check());
        assert_eq!(auth.session().access_token(), None);
        assert_eq!(auth.session().refresh_token(), None);
    }

    #[test]
    fn refresh_replaces_the_stored_tokens() {
        let store = MemoryStore::new();
        let mut auth = Auth::new();
        auth.login(tokens(), &store).unwrap();
        let github = FakeGithub::with_refresh(Tokens {
            access_token: "access-2".into(),
            refresh_token: Some("refresh-2".into()),
        });

        auth.refresh(&github, &store).unwrap();

        assert!(auth.check());
        assert_eq!(auth.session().access_token(), Some("access-2"));
        assert_eq!(auth.session().refresh_token(), Some("refresh-2"));
        let reloaded = Auth::load(&store);
        assert_eq!(reloaded.session().access_token(), Some("access-2"));
        assert_eq!(reloaded.session().refresh_token(), Some("refresh-2"));
    }

    #[test]
    fn load_restores_tokens_saved_at_login() {
        let store = MemoryStore::new();
        let mut auth = Auth::new();
        auth.login(tokens(), &store).unwrap();

        let loaded = Auth::load(&store);
        assert!(loaded.check());
        assert_eq!(loaded.session().refresh_token(), Some("refresh"));
    }
}
