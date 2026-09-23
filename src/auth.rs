#![allow(dead_code)]

pub struct Auth {
    logged_in: bool,
}

impl Auth {
    pub fn new() -> Self {
        Self { logged_in: false }
    }

    pub fn login(&mut self) {
        self.logged_in = true;
    }

    pub fn logout(&mut self) {
        self.logged_in = false;
    }

    pub fn check(&self) -> bool {
        self.logged_in
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_logged_out() {
        let auth = Auth::new();
        assert!(!auth.check());
    }

    #[test]
    fn login_marks_the_user_as_logged_in() {
        let mut auth = Auth::new();
        auth.login();
        assert!(auth.check());
    }

    #[test]
    fn logout_marks_the_user_as_logged_out() {
        let mut auth = Auth::new();
        auth.login();
        auth.logout();
        assert!(!auth.check());
    }

    #[test]
    fn login_twice_stays_logged_in() {
        let mut auth = Auth::new();
        auth.login();
        auth.login();
        assert!(auth.check());
    }

    #[test]
    fn logout_when_already_logged_out_stays_logged_out() {
        let mut auth = Auth::new();
        auth.logout();
        assert!(!auth.check());
    }
}
