//! Wire protocol for the loopback agent bridge: one JSON object per line,
//! request in / response out, plus the semantic-node model the `tree`/`find`
//! tools serve and the curated intent/condition vocabularies.
//!
//! This is a trust boundary. Only curated intents and curated state
//! projections cross it; there is no `eval` analog by design, and nothing here
//! ever carries key material or capability URLs.

use serde::{Deserialize, Serialize};

/// A single request line: an id the caller correlates responses by, and the
/// command to run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub id: u64,
    pub cmd: Cmd,
}

/// A single response line. `ok` mirrors whether `error` is `None`; `result`
/// carries the command's payload (or `null` on error).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub id: u64,
    pub ok: bool,
    pub result: serde_json::Value,
    pub error: Option<String>,
}

impl Response {
    pub fn ok(id: u64, result: serde_json::Value) -> Self {
        Self {
            id,
            ok: true,
            result,
            error: None,
        }
    }

    pub fn err(id: u64, error: impl Into<String>) -> Self {
        Self {
            id,
            ok: false,
            result: serde_json::Value::Null,
            error: Some(error.into()),
        }
    }
}

/// The command set. Mirrors the tauri-agent surface one-to-one; `window`
/// defaults to `"main"` when omitted (see [`window_or_main`]).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Cmd {
    /// Full semantic tree for a window.
    Tree { window: Option<String> },
    /// Filter the flat node list by role / name-substring / text-substring.
    Find {
        window: Option<String>,
        role: Option<Role>,
        name: Option<String>,
        text: Option<String>,
    },
    /// Move-press-release at a target's bounds center (or raw x/y).
    Click { target: Target },
    /// Move the cursor onto a target without pressing.
    Hover { target: Target },
    /// Move onto a target, then wheel-scroll by (dx, dy) pixels.
    Scroll { target: Target, dx: f32, dy: f32 },
    /// Press at `from`, interpolate to `to`, release.
    Drag { from: Target, to: Target },
    /// Type a string, one synthetic key press/release per character.
    Type { text: String },
    /// Press a named key (enter/tab/escape/…) with optional modifiers.
    Press {
        key: String,
        modifiers: Option<Vec<String>>,
    },
    /// Dot-path query into the curated state projection (whole tree if `None`).
    State { path: Option<String> },
    /// Inject a curated semantic intent into the app's update loop.
    Intent { intent: Intent },
    /// Base64 PNG screenshot of a window.
    Shot { window: Option<String> },
    /// Read (and optionally clear) the log ring.
    Logs { clear: bool },
    /// Poll until a condition holds or the timeout (default 5000ms) elapses.
    Wait { cond: Cond, timeout_ms: Option<u64> },
    /// One-shot condition check; never blocks.
    Expect { cond: Cond },
    /// List known windows and their root bounds.
    Windows,
    /// Dump the AccessKit tree actually pushed to the OS adapter.
    A11y { window: Option<String> },
}

/// Resolves an optional window name to the default `"main"`.
pub fn window_or_main(window: &Option<String>) -> &str {
    window.as_deref().unwrap_or("main")
}

/// A click/hover/drag target: either an `@ref` from the last snapshot, or a
/// raw window-space coordinate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Target {
    #[serde(rename = "ref")]
    pub r#ref: Option<String>,
    pub x: Option<f32>,
    pub y: Option<f32>,
}

/// A semantic node in the served tree. `@ref` handles are valid only until the
/// next `tree`/`find` snapshot (tauri-agent convention).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemNode {
    #[serde(rename = "ref")]
    pub r#ref: String,
    pub role: Role,
    pub name: String,
    pub value: Option<String>,
    pub bounds: Rect,
    pub disabled: bool,
    pub focused: bool,
    pub children: Vec<SemNode>,
}

/// The semantic roles view code tags widgets with. Maps onto AccessKit roles
/// via [`Role::to_accesskit`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Window,
    Button,
    Link,
    TextInput,
    Checkbox,
    Tab,
    List,
    ListItem,
    Heading,
    Label,
    Image,
    Group,
    Region,
}

impl Role {
    /// The AccessKit role this maps to (via the fork's re-exported accesskit).
    pub fn to_accesskit(self) -> iced_winit::accesskit::Role {
        use iced_winit::accesskit::Role as A;
        match self {
            Role::Window => A::Window,
            Role::Button => A::Button,
            Role::Link => A::Link,
            Role::TextInput => A::TextInput,
            Role::Checkbox => A::CheckBox,
            Role::Tab => A::Tab,
            Role::List => A::List,
            Role::ListItem => A::ListItem,
            Role::Heading => A::Heading,
            Role::Label => A::Label,
            Role::Image => A::Image,
            Role::Group => A::Group,
            Role::Region => A::Region,
        }
    }
}

/// A window-space rectangle (top-left origin, y-down), the wire form of
/// `iced::Rectangle`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    /// Center point, where synthetic clicks land.
    pub fn center(&self) -> (f32, f32) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }
}

impl From<iced::Rectangle> for Rect {
    fn from(r: iced::Rectangle) -> Self {
        Self {
            x: r.x,
            y: r.y,
            width: r.width,
            height: r.height,
        }
    }
}

/// Curated, side-effect-understood intents — deliberately not the full app
/// `Message` enum. The app maps each to a real message in its update loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Intent {
    Section { name: String },
    Navigate { url: String },
    ToggleTheme,
    Search { query: String },
}

/// A condition the `wait`/`expect` tools evaluate against the current snapshot
/// or state projection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cond {
    /// A node of the given role/name (either optional) exists — or must not.
    Node {
        role: Option<Role>,
        name: Option<String>,
        exists: bool,
    },
    /// A dot-path in the state projection equals the given JSON value.
    StatePath {
        path: String,
        equals: serde_json::Value,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_round_trips() {
        let line = r#"{"id":1,"cmd":{"cmd":"find","window":"main","role":"button","name":"Forge","text":null}}"#;
        let req: Request = serde_json::from_str(line).unwrap();
        assert!(matches!(req.cmd, Cmd::Find { .. }));
        let back = serde_json::to_string(&req).unwrap();
        let again: Request = serde_json::from_str(&back).unwrap();
        assert_eq!(req.id, again.id);
    }

    #[test]
    fn role_maps_to_accesskit() {
        use iced_winit::accesskit::Role as A;
        assert_eq!(Role::Button.to_accesskit(), A::Button);
        assert_eq!(Role::TextInput.to_accesskit(), A::TextInput);
        assert_eq!(Role::Checkbox.to_accesskit(), A::CheckBox);
        assert_eq!(Role::ListItem.to_accesskit(), A::ListItem);
    }

    #[test]
    fn role_snake_case_wire_names() {
        assert_eq!(serde_json::to_string(&Role::TextInput).unwrap(), "\"text_input\"");
        assert_eq!(serde_json::to_string(&Role::ListItem).unwrap(), "\"list_item\"");
    }

    #[test]
    fn unit_and_target_commands_round_trip() {
        // Unit variant (internally tagged) carries only the discriminator.
        let windows = serde_json::to_string(&Request {
            id: 7,
            cmd: Cmd::Windows,
        })
        .unwrap();
        assert_eq!(windows, r#"{"id":7,"cmd":{"cmd":"windows"}}"#);

        // A ref-targeted click parses and preserves its ref.
        let line = r#"{"id":2,"cmd":{"cmd":"click","target":{"ref":"@5","x":null,"y":null}}}"#;
        let req: Request = serde_json::from_str(line).unwrap();
        match req.cmd {
            Cmd::Click { target } => assert_eq!(target.r#ref.as_deref(), Some("@5")),
            other => panic!("expected click, got {other:?}"),
        }
    }

    #[test]
    fn intent_and_cond_round_trip() {
        let intent = Cmd::Intent {
            intent: Intent::Section {
                name: "home".into(),
            },
        };
        let s = serde_json::to_string(&intent).unwrap();
        assert!(s.contains(r#""cmd":"intent""#));
        assert!(s.contains(r#""section":{"name":"home"}"#));

        let cond = Cond::Node {
            role: Some(Role::Button),
            name: Some("Settings".into()),
            exists: true,
        };
        let back: Cond = serde_json::from_str(&serde_json::to_string(&cond).unwrap()).unwrap();
        assert!(matches!(back, Cond::Node { exists: true, .. }));
    }

    #[test]
    fn window_defaults_to_main() {
        assert_eq!(window_or_main(&None), "main");
        assert_eq!(window_or_main(&Some("huddle".into())), "huddle");
    }
}
