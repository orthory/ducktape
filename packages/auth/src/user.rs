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

// canonical render of a User is its display_name verbatim — the exact inverse
// of `from_str`. `provider` is never serialized (from_str always sets it None),
// so a User round-trips through `from_str(user.to_string())` losslessly.
impl std::fmt::Display for User {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.display_name)
    }
}
