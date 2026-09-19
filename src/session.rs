/// App-level flags used by route guards. The router only sees a `Fn(&str) -> bool`.
pub struct Session {
    pub logged_in: bool,
}

impl Session {
    pub fn new(logged_in: bool) -> Self {
        Self { logged_in }
    }

    pub fn named(logged_in: bool, name: &str) -> bool {
        match name {
            "session" => logged_in,
            "guest" => !logged_in,
            _ => true,
        }
    }

    pub fn allow(logged_in: bool) -> impl Fn(&str) -> bool {
        move |name| Self::named(logged_in, name)
    }
}
