//! Capability-free contract shared by terminal presentation and lifecycle code.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionMode {
    Single,
    Shared,
}

impl SessionMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::Shared => "shared",
        }
    }
}
