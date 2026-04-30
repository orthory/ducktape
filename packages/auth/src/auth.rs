use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum AuthProvider {
    Github,
    Slack,
    JohnDoe(String),
}
