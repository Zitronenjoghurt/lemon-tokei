use std::collections::HashSet;

#[derive(Clone)]
pub struct Config {
    pub port: u16,
    pub allowed_users: HashSet<String>,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            port: std::env::var("TOKEI_PORT")
                .unwrap_or("8000".to_string())
                .parse::<u16>()
                .expect("Invalid port number"),
            allowed_users: std::env::var("ALLOWED_USERS")
                .unwrap_or_default()
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
        }
    }

    pub fn is_user_allowed(&self, user: &str) -> bool {
        self.allowed_users.is_empty() || self.allowed_users.contains(user)
    }
}
