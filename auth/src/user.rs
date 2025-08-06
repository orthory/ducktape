use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct User {
    display_name: String,
    provider: Option<crate::auth::AuthProvider>,
}

impl User {
    pub fn from_str(input: &str) -> Self {
        Self {
            display_name: input.to_string(),
            provider: None,
        }
    }
}
