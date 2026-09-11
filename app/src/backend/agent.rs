use super::*;
use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, renderer};
use iced::{Element, Length, Rectangle, Size, Theme, mouse};
use std::hash::{Hash, Hasher};
use std::rc::Rc;

const LINK_TOKEN_BYTES: u64 = 4 * 1024;

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct AgentChatEvent {
    pub id: i64,
    pub kind: String,
    pub title: String,
    pub detail: String,
    pub status: String,
    pub answer: String,
    pub saga_id: String,
}

/// Parsed Markdown whose items live with the widget instead of borrowing an
/// Ice state field. The app is multi-window, so the native Ice markdown node
/// cannot ask for a theme without a window id; this adapter owns the parse and
/// uses the app palette's stable text roles directly.
pub fn agent_markdown(source: String, dark: bool) -> Element<'static, String> {
    Element::new(AgentMarkdown::new(&source, dark))
}

/// The forge reader's Markdown: the same surface as [`agent_markdown`], plus
/// the document's in-repo pictures (parked by `forge_blob` under `doc`)
/// drawn in place of their alt text.
pub fn forge_markdown(source: String, doc: String, dark: bool) -> Element<'static, String> {
    Element::new(AgentMarkdown::new(&source, dark).with_doc(doc))
}

pub(crate) fn provider_output_event(provider: &str, line: &str, id: i64) -> Option<AgentChatEvent> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    if provider == "claude" && value["type"].as_str() == Some("result") {
        let answer = value["result"].as_str()?.to_string();
        return Some(chat_preview(id, answer));
    }
    let claude_assistant = provider == "claude" && value["type"] == "assistant";
    if claude_assistant {
        // Tool names describe observed activity without copying arguments,
        // tool output, or thinking blocks into the chat status.
        let blocks = value["message"]["content"].as_array()?;
        let tool = blocks
            .iter()
            .rev()
            .find(|block| block["type"] == "tool_use")?;
        let name = clip_text(tool["name"].as_str()?, 80);
        return Some(AgentChatEvent {
            id,
            kind: "status".into(),
            title: format!("Using {name}"),
            detail: String::new(),
            status: String::new(),
            answer: String::new(),
            saga_id: String::new(),
        });
    }
    let claude_tool_reply = provider == "claude" && value["type"] == "user";
    if claude_tool_reply {
        let blocks = value["message"]["content"].as_array()?;
        let result = blocks
            .iter()
            .rev()
            .find(|block| block["type"] == "tool_result")?;
        let failed = result["is_error"] == true;
        let title = if failed {
            "Tool failed · waiting for agent"
        } else {
            "Tool finished · waiting for agent"
        };
        return Some(AgentChatEvent {
            id,
            kind: "status".into(),
            title: title.into(),
            detail: String::new(),
            status: String::new(),
            answer: String::new(),
            saga_id: String::new(),
        });
    }
    let event_type = value["type"].as_str().unwrap_or_default();
    let item = &value["item"];
    let item_type = item["type"]
        .as_str()
        .or_else(|| item["item_type"].as_str())
        .unwrap_or_default();
    if item_type == "agent_message" {
        let answer = item["text"]
            .as_str()
            .or_else(|| item["message"].as_str())?
            .to_string();
        return Some(chat_preview(id, answer));
    }
    let completed = event_type.ends_with("completed");
    let status = if completed { "done" } else { "running" };
    let (title, detail) = match item_type {
        "reasoning" => (
            "Reasoning".to_string(),
            json_text(item.get("text").or_else(|| item.get("summary"))),
        ),
        "command_execution" => (
            "Command".to_string(),
            json_text(
                item.get("command")
                    .or_else(|| item.get("aggregated_output")),
            ),
        ),
        "mcp_tool_call" => {
            let server = item["server"].as_str().unwrap_or("tool");
            let tool = item["tool"].as_str().unwrap_or("call");
            (
                format!("{server} · {tool}"),
                json_text(item.get("arguments")),
            )
        }
        "web_search" => ("Web search".to_string(), json_text(item.get("query"))),
        _ => return None,
    };
    Some(AgentChatEvent {
        id,
        kind: "activity".into(),
        title,
        detail: clip_text(&detail, 1_200),
        status: status.into(),
        answer: String::new(),
        saga_id: String::new(),
    })
}

fn chat_preview(id: i64, answer: String) -> AgentChatEvent {
    AgentChatEvent {
        id,
        kind: "preview".into(),
        title: "Writing the answer".into(),
        detail: String::new(),
        status: String::new(),
        answer,
        saga_id: String::new(),
    }
}

pub(crate) fn agent_ws_url(rpc: &str) -> String {
    let base = if let Some(rest) = rpc.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = rpc.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        rpc.to_string()
    };
    format!("{}/v1/ws", base.trim_end_matches('/'))
}

pub(crate) fn read_link_token(workspace: &Path) -> Result<String, String> {
    let path = workspace.join("service-link.token");
    let metadata = std::fs::metadata(&path).map_err(|_| {
        "The node's agent event token is not available in its workspace.".to_string()
    })?;
    if metadata.len() > LINK_TOKEN_BYTES {
        return Err("The node's agent event token is unexpectedly large.".into());
    }
    let token = std::fs::read_to_string(path)
        .map_err(|_| "The node's agent event token could not be read.".to_string())?;
    let token = token.trim().to_string();
    if token.is_empty() {
        return Err("The node's agent event token is empty.".into());
    }
    Ok(token)
}

pub(crate) fn subscription_refusal(value: &serde_json::Value) -> Option<String> {
    let is_refusal =
        value["type"].as_str() == Some("refused") || value["type"].as_str() == Some("error");
    if !is_refusal {
        return None;
    }
    Some(
        value["detail"]
            .as_str()
            .or_else(|| value["error"].as_str())
            .unwrap_or("The node refused the agent event stream.")
            .to_string(),
    )
}

fn json_text(value: Option<&serde_json::Value>) -> String {
    let Some(value) = value else {
        return String::new();
    };
    match value {
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Array(items) => items
            .iter()
            .filter_map(serde_json::Value::as_str)
            .collect::<Vec<_>>()
            .join(" "),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}

pub(crate) fn clip_text(text: &str, limit: usize) -> String {
    if text.len() <= limit {
        return text.to_string();
    }
    let end = text
        .char_indices()
        .map(|(index, _)| index)
        .take_while(|index| *index <= limit)
        .last()
        .unwrap_or(0);
    format!("{}…", &text[..end])
}

struct AgentMarkdown {
    items: Rc<[iced::widget::markdown::Item]>,
    settings: iced::widget::markdown::Settings,
    viewer: SelectViewer,
}

impl AgentMarkdown {
    fn new(source: &str, dark: bool) -> Self {
        use iced::widget::markdown;
        let link = if dark {
            iced::Color::from_rgb8(0xc9, 0x8a, 0x63)
        } else {
            iced::Color::from_rgb8(0xa0, 0x5a, 0x3c)
        };
        let code_background = if dark {
            iced::Color::from_rgb8(0x26, 0x25, 0x23)
        } else {
            iced::Color::from_rgb8(0xf3, 0xf2, 0xef)
        };
        let code_foreground = if dark {
            iced::Color::from_rgb8(0xe8, 0xe6, 0xdf)
        } else {
            iced::Color::from_rgb8(0x3f, 0x3e, 0x39)
        };
        let mono = iced::Font::with_name("Geist Mono");
        let style = markdown::Style {
            font: iced::Font::with_name("Geist"),
            inline_code_highlight: markdown::Highlight {
                background: code_background.into(),
                border: iced::border::rounded(4),
            },
            inline_code_padding: iced::Padding::from([1.0, 2.0]),
            inline_code_color: code_foreground,
            inline_code_font: mono,
            code_block_font: mono,
            link_color: link,
        };
        let mut settings = markdown::Settings::with_text_size(13.5, style);
        settings.h1_size = 18.0.into();
        settings.h2_size = 16.5.into();
        settings.h3_size = 15.0.into();
        settings.h4_size = 14.25.into();
        settings.h5_size = 13.5.into();
        settings.h6_size = 12.75.into();
        settings.code_size = 12.0.into();
        settings.spacing = 9.0.into();
        Self {
            items: markdown::parse(source).collect::<Vec<_>>().into(),
            settings,
            viewer: SelectViewer {
                doc: None,
                key: document_key(source),
                blocks: std::cell::Cell::new(0),
            },
        }
    }

    /// Draw the in-repo pictures parked under `doc` (see `picture.rs`).
    fn with_doc(mut self, doc: String) -> Self {
        self.viewer.doc = Some(doc);
        self
    }

    fn view(&self) -> Element<'_, String> {
        // The blocks are numbered by the order the viewer is asked for them,
        // which is the document's reading order — nested lists and quotes
        // route their items back through the same viewer. `view` is built
        // many times a frame (tag, layout, update, draw), so the count starts
        // over here: the same document always numbers its blocks the same.
        self.viewer.blocks.set(0);
        iced::widget::markdown::view_with(self.items.iter(), self.settings, &self.viewer)
    }
}

/// The Markdown surface with draggable text: every paragraph and heading
/// behind a [`SelectRich`], every code block one [`CodeSelect`], and each of
/// them numbered into ONE document ([`SelectPlace`]) — so a drag that starts
/// in a heading and ends in a code block selects everything between them, and
/// Ctrl+C copies the run whole. Lists, quotes and tables keep iced's default
/// look and route their text back through here, which numbers their items in
/// with the rest. An image draws the picture `forge_blob` parked under `doc`
/// for its URL as written (relative path, `duck://files`,
/// `duck://forge/.../blob/...`), and keeps iced's default (the alt text in a
/// plate) for everything else — including every image when there is no
/// document, as in the agent's answers.
struct SelectViewer {
    doc: Option<String>,
    /// The document every block of this surface shares a selection in, keyed
    /// by its own text, and the running count that gives each block its place
    /// in reading order.
    key: u64,
    blocks: std::cell::Cell<usize>,
}

impl SelectViewer {
    /// The next block's place, in reading order.
    fn next(&self) -> SelectPlace {
        let ordinal = self.blocks.get();
        self.blocks.set(ordinal + 1);
        SelectPlace::block(self.key, ordinal)
    }
}

/// A Markdown document's selection key: its own text. Two documents spelling
/// the same bytes on screen at once show one selection twice — the whole cost
/// of not threading an identity down through iced's viewer.
fn document_key(source: &str) -> u64 {
    let mut hasher = std::hash::DefaultHasher::new();
    source.hash(&mut hasher);
    hasher.finish()
}

impl<'a> iced::widget::markdown::Viewer<'a, String> for SelectViewer {
    fn on_link_click(url: iced::widget::markdown::Uri) -> String {
        url
    }

    fn image(
        &self,
        settings: iced::widget::markdown::Settings,
        url: &'a iced::widget::markdown::Uri,
        _title: &'a str,
        alt: &iced::widget::markdown::Text,
    ) -> Element<'a, String> {
        use iced::widget::{container, rich_text};
        let parked = self
            .doc
            .as_deref()
            .and_then(|doc| super::picture::inline_picture(doc, url));
        let Some(picture) = parked else {
            return container(
                rich_text(alt.spans(settings.style)).on_link_click(Self::on_link_click),
            )
            .padding(settings.spacing.0)
            .class(<iced::Theme as iced::widget::markdown::Catalog>::code_block())
            .into();
        };
        container(picture.element()).width(Length::Fill).into()
    }

    fn heading(
        &self,
        settings: iced::widget::markdown::Settings,
        level: &'a iced::widget::markdown::HeadingLevel,
        text: &'a iced::widget::markdown::Text,
        index: usize,
    ) -> Element<'a, String> {
        use iced::widget::markdown::HeadingLevel;
        use iced::widget::{container, rich_text};
        let size = match level {
            HeadingLevel::H1 => settings.h1_size,
            HeadingLevel::H2 => settings.h2_size,
            HeadingLevel::H3 => settings.h3_size,
            HeadingLevel::H4 => settings.h4_size,
            HeadingLevel::H5 => settings.h5_size,
            HeadingLevel::H6 => settings.h6_size,
        };
        let spans = text.spans(settings.style);
        let rich = rich_text(spans.clone())
            .on_link_click(Self::on_link_click)
            .size(size);
        let top = match index > 0 {
            true => settings.text_size / 2.0,
            false => iced::Pixels::ZERO,
        };
        container(SelectRich::new(rich, spans, size).at(self.next()))
            .padding(iced::padding::top(top))
            .into()
    }

    fn paragraph(
        &self,
        settings: iced::widget::markdown::Settings,
        text: &iced::widget::markdown::Text,
    ) -> Element<'a, String> {
        let spans = text.spans(settings.style);
        let rich = iced::widget::rich_text(spans.clone())
            .on_link_click(Self::on_link_click)
            .size(settings.text_size);
        SelectRich::new(rich, spans, settings.text_size)
            .at(self.next())
            .into()
    }

    fn code_block(
        &self,
        settings: iced::widget::markdown::Settings,
        _language: Option<&'a str>,
        _code: &'a str,
        lines: &'a [iced::widget::markdown::Text],
    ) -> Element<'a, String> {
        use iced::widget::markdown::Catalog as _;
        use iced::widget::{container, scrollable};
        let metrics = CodeMetrics {
            size: settings.code_size,
            line_height: iced::advanced::text::LineHeight::default(),
            font: settings.style.code_block_font,
        };
        let plate = CodeSelect::new(
            lines.iter().map(|line| line.spans(settings.style)),
            metrics,
            None,
            Length::Shrink,
        )
        .at(self.next());
        container(
            scrollable(container(plate).padding(settings.code_size)).direction(
                scrollable::Direction::Horizontal(
                    scrollable::Scrollbar::default()
                        .width(settings.code_size / 2)
                        .scroller_width(settings.code_size / 2),
                ),
            ),
        )
        .width(Length::Fill)
        .padding(settings.code_size / 4)
        .class(Theme::code_block())
        .into()
    }
}

impl Widget<String, Theme, iced::Renderer> for AgentMarkdown {
    fn tag(&self) -> tree::Tag {
        self.view().as_widget().tag()
    }

    fn state(&self) -> tree::State {
        self.view().as_widget().state()
    }

    fn children(&self) -> Vec<Tree> {
        self.view().as_widget().children()
    }

    fn diff(&self, tree: &mut Tree) {
        self.view().as_widget().diff(tree);
    }

    fn size(&self) -> Size<Length> {
        self.view().as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.view().as_widget().size_hint()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.view().as_widget_mut().layout(tree, renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        self.view()
            .as_widget_mut()
            .operate(tree, layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &iced::Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, String>,
        viewport: &Rectangle,
    ) {
        self.view().as_widget_mut().update(
            tree, event, layout, cursor, renderer, clipboard, shell, viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.view()
            .as_widget()
            .mouse_interaction(tree, layout, cursor, viewport, renderer)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.view()
            .as_widget()
            .draw(tree, renderer, theme, style, layout, cursor, viewport);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_tool_progress_names_the_observed_tool_without_private_content() {
        let line = serde_json::json!({
            "type": "assistant",
            "message": { "content": [
                { "type": "thinking", "thinking": "SECRET reasoning" },
                { "type": "tool_use", "name": "Read", "input": { "file_path": "SECRET path" } }
            ] }
        })
        .to_string();
        let event = provider_output_event("claude", &line, 8).unwrap();
        assert_eq!(event.kind, "status");
        assert_eq!(event.title, "Using Read");
        assert!(event.detail.is_empty());
        assert!(event.answer.is_empty());
        let result = r#"{"type":"user","message":{"content":[{"type":"tool_result","content":"SECRET output"}]}}"#;
        let event = provider_output_event("claude", result, 9).unwrap();
        assert_eq!(event.title, "Tool finished · waiting for agent");
        assert!(event.detail.is_empty());
        assert!(event.answer.is_empty());
        let thought = r#"{"type":"assistant","message":{"content":[{"type":"thinking","thinking":"SECRET"}]}}"#;
        assert!(provider_output_event("claude", thought, 9).is_none());
    }

    #[test]
    fn provider_output_projects_known_events_not_raw_json() {
        let line = serde_json::json!({
            "type": "item.completed",
            "item": { "type": "agent_message", "text": "answer" }
        })
        .to_string();
        let event = provider_output_event("codex", &line, 7).unwrap();
        assert_eq!(event.kind, "preview");
        assert_eq!(event.answer, "answer");
        assert!(provider_output_event("codex", r#"{"type":"unknown","secret":"no"}"#, 8).is_none());
    }
}
