#![allow(dead_code)]

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Session {
    access_token: Option<String>,
    refresh_token: Option<String>,
}

impl Session {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn store(&mut self, tokens: Tokens) {
        self.access_token = Some(tokens.access_token);
        if let Some(refresh_token) = tokens.refresh_token.filter(|token| !token.is_empty()) {
            self.refresh_token = Some(refresh_token);
        }
    }

    pub fn clear(&mut self) {
        self.access_token = None;
        self.refresh_token = None;
    }

    pub fn access_token(&self) -> Option<&str> {
        self.access_token.as_deref()
    }

    pub fn refresh_token(&self) -> Option<&str> {
        self.refresh_token.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens() -> Tokens {
        Tokens {
            access_token: "access".into(),
            refresh_token: Some("refresh".into()),
        }
    }

    #[test]
    fn stores_both_tokens() {
        let mut session = Session::new();
        session.store(tokens());

        assert_eq!(session.access_token(), Some("access"));
        assert_eq!(session.refresh_token(), Some("refresh"));
    }

    #[test]
    fn refresh_replaces_both_tokens() {
        let mut session = Session::new();
        session.store(tokens());
        session.store(Tokens {
            access_token: "access-2".into(),
            refresh_token: Some("refresh-2".into()),
        });

        assert_eq!(session.access_token(), Some("access-2"));
        assert_eq!(session.refresh_token(), Some("refresh-2"));
    }

    #[test]
    fn clear_removes_both_tokens() {
        let mut session = Session::new();
        session.store(tokens());
        session.clear();

        assert_eq!(session.access_token(), None);
        assert_eq!(session.refresh_token(), None);
    }
}
