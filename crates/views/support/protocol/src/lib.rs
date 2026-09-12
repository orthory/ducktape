//! The single v1 contract between a Rust WASM view and its native GPUI host.
//! Guests own application decisions. The host owns layout, devices and the
//! authority to execute requests. Neither side serializes toolkit objects.

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;

pub const MAX_FRAME_BYTES: usize = 8 << 20;
pub const MAX_PAYLOAD_BYTES: usize = 1 << 20;
pub const MAX_REQUESTS: usize = 256;
pub const MAX_NODES: usize = 16_384;
pub const MAX_DEPTH: usize = 64;
pub const MAX_TEXT_BYTES: usize = 2 << 20;
pub const MANIFEST_SECTION: &str = "ducktape.view";

/// Decode exactly one bounded JSON value. Unknown structural fields are
/// rejected by the contract types; capability-specific data is validated
/// again by its registered handler.
pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, String> {
    if bytes.len() > MAX_FRAME_BYTES {
        return Err("view message exceeds byte budget".into());
    }
    serde_json::from_slice(bytes).map_err(|error| error.to_string())
}

pub fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
    let bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    if bytes.len() > MAX_FRAME_BYTES {
        return Err("view message exceeds byte budget".into());
    }
    Ok(bytes)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub module: String,
    pub name: String,
    pub description: String,
    /// Exact capability names. A declaration requests authority; the host's
    /// admission policy still determines which declarations it grants.
    pub capabilities: Vec<String>,
    pub surfaces: Vec<String>,
}

impl Manifest {
    pub fn validate(&self) -> Result<(), String> {
        let bounded = name(&self.module)
            && !self.name.is_empty()
            && self.name.len() <= 64
            && self.description.len() <= 256
            && self.capabilities.len() <= 64
            && self.surfaces.len() <= 64;
        if !bounded {
            return Err("invalid view manifest".into());
        }
        for names in [&self.capabilities, &self.surfaces] {
            let mut seen = std::collections::BTreeSet::new();
            for item in names {
                let valid = name(item) && seen.insert(item);
                if !valid {
                    return Err("invalid or duplicate manifest declaration".into());
                }
            }
        }
        Ok(())
    }
}

pub fn name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"._-/".contains(&byte))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub id: u64,
    pub kind: String,
    pub payload: Vec<u8>,
}

impl Request {
    pub fn validate(&self) -> Result<(), String> {
        let valid = name(&self.kind) && self.payload.len() <= MAX_PAYLOAD_BYTES;
        if !valid {
            return Err("invalid request kind or payload budget".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case", deny_unknown_fields)]
pub enum Event {
    /// A semantic action attached to a stable widget identity. Revision
    /// prevents an input queued against an old tree from hitting a new one.
    Action { revision: u64, key: String, action: String, value: Value },
    Response { id: u64, result: Result<Vec<u8>, String>, done: bool },
    Editor { transaction: EditorTransaction },
    Size { key: String, width: f32, height: f32 },
    Focus { key: String, focused: bool },
    Resync,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    pub revision: u64,
    /// None retains the last tree. The first frame must carry a root.
    pub root: Option<Node>,
    pub requests: Vec<Request>,
    pub cancels: Vec<u64>,
    pub commands: Vec<Command>,
    pub busy: bool,
}

impl Frame {
    pub fn validate(&self) -> Result<(), String> {
        let bounded = self.requests.len() <= MAX_REQUESTS
            && self.cancels.len() <= MAX_REQUESTS
            && self.commands.len() <= MAX_REQUESTS;
        if !bounded {
            return Err("view frame exceeds operation budget".into());
        }
        let mut requests = std::collections::BTreeSet::new();
        for request in &self.requests {
            request.validate()?;
            if !requests.insert(request.id) {
                return Err("duplicate request id in frame".into());
            }
        }
        if let Some(root) = &self.root {
            let mut pending = vec![(root, 0)];
            let mut keys = std::collections::BTreeSet::new();
            let mut text_bytes = 0usize;
            while let Some((node, depth)) = pending.pop() {
                let valid = depth < MAX_DEPTH
                    && keys.len() < MAX_NODES
                    && name(&node.key)
                    && keys.insert(node.key.as_str());
                if !valid {
                    return Err("invalid view tree depth, count or identity".into());
                }
                node.style.validate()?;
                text_bytes = text_bytes.saturating_add(node.text_bytes());
                if text_bytes > MAX_TEXT_BYTES {
                    return Err("view tree exceeds text budget".into());
                }
                pending.extend(node.children.iter().map(|child| (child, depth + 1)));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Node {
    pub key: String,
    pub kind: Kind,
    #[serde(default)]
    pub style: Style,
    #[serde(default)]
    pub children: Vec<Node>,
}

impl Node {
    pub fn new(key: impl Into<String>, kind: Kind) -> Self {
        Self { key: key.into(), kind, style: Style::default(), children: Vec::new() }
    }

    pub fn child(mut self, child: Node) -> Self {
        self.children.push(child);
        self
    }

    fn text_bytes(&self) -> usize {
        match &self.kind {
            Kind::Text { text, .. } | Kind::Markdown { source: text }
            | Kind::Code { source: text, .. } => text.len(),
            Kind::Button { label, action, .. } => label.len() + action.len(),
            Kind::Input { value, placeholder, .. } => value.len() + placeholder.len(),
            Kind::Toggle { label, .. } => label.len(),
            Kind::Select { options, .. } => options.iter().map(String::len).sum(),
            Kind::Tooltip { text } => text.len(),
            Kind::Surface { props, .. } => props.to_string().len(),
            _ => 0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Kind {
    Column,
    Row,
    Stack,
    Grid { columns: u16 },
    Scroll { axis: Axis, anchor: Anchor },
    Text { text: String, selectable: bool },
    Markdown { source: String },
    Code { source: String, language: String },
    Button { label: String, action: String, disabled: bool },
    Input { value: String, placeholder: String, action: String, submit: Option<String>, secret: bool, multiline: bool, disabled: bool },
    Toggle { label: String, value: bool, action: String, disabled: bool },
    Select { options: Vec<String>, selected: Option<u32>, action: String, disabled: bool },
    Image { asset: String, fit: Fit, alt: String },
    Icon { name: String, label: String },
    Rule,
    Space,
    Tooltip { text: String },
    Menu { open: bool },
    Overlay { dismiss: Option<String> },
    Sensor { action: String },
    ResizeHandle { action: String, axis: Axis },
    /// Native editing mechanics over a guest-owned document. The contents
    /// transfer separately; frames reference a revision rather than copying
    /// every document into every redraw.
    Editor { document: DocumentRef, readonly: bool, placeholder: String },
    /// Explicit extension point: the host looks up a registered provider and
    /// validates its props. Unknown surfaces fail visibly, never disappear.
    Surface { name: String, props: Value, action: Option<String> },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Axis { Horizontal, Vertical, Both }
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Anchor { Start, End, Keep }
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fit { Contain, Cover, Fill }

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(tag = "unit", content = "value", rename_all = "snake_case")]
pub enum Length { #[default] Auto, Fill, Pixels(f32), Portion(u16) }

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Style {
    pub width: Length,
    pub height: Length,
    pub min_width: Option<f32>,
    pub max_width: Option<f32>,
    pub min_height: Option<f32>,
    pub max_height: Option<f32>,
    pub gap: f32,
    pub padding: [f32; 4],
    pub radius: f32,
    pub border_width: f32,
    /// Packed RGBA, independent of native toolkit color representations.
    pub foreground: Option<u32>,
    pub background: Option<u32>,
    pub border: Option<u32>,
    pub hover_background: Option<u32>,
    pub font_size: Option<f32>,
    pub font_family: Option<String>,
    pub bold: bool,
    pub center: bool,
    pub clip: bool,
}

impl Style {
    fn validate(&self) -> Result<(), String> {
        let dimensions = [self.min_width, self.max_width, self.min_height,
            self.max_height, self.font_size, Some(self.gap), Some(self.radius),
            Some(self.border_width)];
        let lengths = [self.width, self.height].into_iter().filter_map(|length| match length {
            Length::Pixels(value) => Some(value),
            _ => None,
        });
        let valid = dimensions.into_iter().flatten().chain(self.padding).chain(lengths)
            .all(|value| value.is_finite() && (0.0..=16_384.0).contains(&value));
        if !valid {
            return Err("invalid view geometry".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentRef {
    pub id: String,
    pub reset: u64,
    pub revision: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    pub anchor: u32,
    pub head: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Edit {
    pub start: u32,
    pub end: u32,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditorTransaction {
    pub id: u64,
    pub document: DocumentRef,
    pub selection: Selection,
    pub action: String,
    pub edits: Vec<Edit>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    Focus { key: String },
    Scroll { key: String, target: String },
    Clipboard { text: String },
    OpenLink { url: String },
    Document { document: DocumentRef, text: String, selection: Selection },
    Edit { transaction: EditorTransaction },
    EditorAccepted { id: u64, document: DocumentRef },
    EditorRejected { id: u64, reason: String },
    Presentation { document: DocumentRef, value: Value },
    Intent { kind: String, value: Value },
}

/// Validate every edit against the same original UTF-8 buffer before writing
/// any bytes. Invalid or overlapping edits never partially change a document.
pub fn apply_edits(text: &str, edits: &[Edit]) -> Result<String, String> {
    if edits.len() > MAX_REQUESTS {
        return Err("too many editor patches".into());
    }
    let mut end = 0;
    let mut size = text.len();
    for edit in edits {
        let start = edit.start as usize;
        let next = edit.end as usize;
        let valid = start >= end && start <= next && next <= text.len()
            && text.is_char_boundary(start) && text.is_char_boundary(next);
        if !valid {
            return Err("editor patches overlap or split UTF-8".into());
        }
        size = size.saturating_sub(next - start).saturating_add(edit.text.len());
        if size > MAX_TEXT_BYTES {
            return Err("edited document exceeds text budget".into());
        }
        end = next;
    }
    let mut result = String::with_capacity(size);
    let mut end = 0;
    for edit in edits {
        result.push_str(&text[end..edit.start as usize]);
        result.push_str(&edit.text);
        end = edit.end as usize;
    }
    result.push_str(&text[end..]);
    Ok(result)
}
