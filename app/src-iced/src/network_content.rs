//! Trusted loader for the inert, network-owned `net.duck` snapshot.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use ammonia::{Builder, UrlRelative};
use html5ever::serialize::{SerializeOpts, TraversalScope};
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::{Handle, NodeData, RcDom, SerializableHandle};
use reqwest::Url;

use crate::transport::{FileEntry, NodeClient};

pub(crate) const NETWORK_CONTENT_ROOT: &str = "/shared/.duck/net";
pub(crate) const NETWORK_CSP: &str = "sandbox; default-src 'none'; script-src 'none'; connect-src 'none'; img-src data:; style-src 'unsafe-inline'; font-src 'none'; media-src 'none'; frame-src 'none'; child-src 'none'; worker-src 'none'; object-src 'none'; form-action 'none'; base-uri 'none'";

const ALLOWED_TAGS: &[&str] = &[
    "main",
    "section",
    "article",
    "header",
    "footer",
    "nav",
    "aside",
    "div",
    "span",
    "p",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "ul",
    "ol",
    "li",
    "dl",
    "dt",
    "dd",
    "blockquote",
    "pre",
    "code",
    "strong",
    "em",
    "b",
    "i",
    "u",
    "s",
    "small",
    "mark",
    "sub",
    "sup",
    "br",
    "hr",
    "a",
    "img",
    "figure",
    "figcaption",
    "table",
    "thead",
    "tbody",
    "tfoot",
    "tr",
    "th",
    "td",
    "caption",
];

const CLEAN_CONTENT_TAGS: &[&str] = &[
    "title", "script", "iframe", "object", "embed", "template", "svg", "math", "form", "input",
    "button", "textarea", "select", "option", "link", "meta", "base", "audio", "video", "source",
    "track",
];

/// One sanitized document tied to the exact DuckFS snapshot that supplied it.
#[derive(Debug, Clone)]
pub(crate) struct LocalDocument {
    pub(crate) url: String,
    pub(crate) bytes: Arc<[u8]>,
    // Retained as provenance for the pending local-document handoff.
    #[allow(dead_code)]
    pub(crate) snapshot: String,
    #[allow(dead_code)]
    pub(crate) title: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Request {
    url: String,
    relative: String,
}

/// Load exactly one HTML document from the current network-owned snapshot.
pub(crate) async fn load(client: NodeClient, url: String) -> Result<LocalDocument, String> {
    let request = parse_request(&url)?;
    let snapshot = client
        .files_refs()
        .await
        .map_err(|error| error.to_string())?
        .head
        .ok_or_else(|| "net.duck has no DuckFS snapshot.".to_string())?;
    let absolute = format!("{NETWORK_CONTENT_ROOT}/{}", request.relative);
    let entry = client
        .files_stat(&absolute, Some(&snapshot))
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("net.duck page not found: {}", request.relative))?;
    validate_entry(&entry, &absolute, &request.relative)?;
    let bytes = client
        .files_read_exact(&absolute, &snapshot, entry.size)
        .await
        .map_err(|error| error.to_string())?;
    build_document(request, snapshot, &entry, bytes)
}

fn parse_request(raw: &str) -> Result<Request, String> {
    if raw.is_empty()
        || raw.len() > 2 * 1024
        || raw
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
    {
        return Err("net.duck address is empty, oversized, or malformed.".into());
    }
    let parsed = Url::parse(raw).map_err(|_| "net.duck address is malformed.".to_string())?;
    if parsed.scheme() != "duck"
        || parsed.host_str() != Some("net.duck")
        || parsed.port().is_some()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
    {
        return Err("Only direct net.duck HTML paths without a query are local.".into());
    }

    // Validate the raw origin form before URL normalization can erase `.` or
    // `..` segments. The URL parser above remains the authority check.
    let (_, rest) = raw
        .split_once("://")
        .ok_or_else(|| "net.duck address is malformed.".to_string())?;
    let origin_form = rest.split_once('#').map_or(rest, |(path, _)| path);
    let path = origin_form
        .find('/')
        .map_or("/", |index| &origin_form[index..]);
    if path.starts_with("//") || path.contains(['\\', '?', '#', '%']) {
        return Err("net.duck path is not canonical.".into());
    }
    let relative = if path == "/" {
        "index.html"
    } else {
        path.strip_prefix('/')
            .ok_or_else(|| "net.duck path is not origin-form.".to_string())?
    };
    gateway::validate_content_path(relative)
        .map_err(|_| "net.duck path is not canonical.".to_string())?;
    if !relative.to_ascii_lowercase().ends_with(".html") {
        return Err("net.duck opens HTML documents only.".into());
    }
    let mut url = format!("duck://net.duck{path}");
    if let Some(fragment) = parsed.fragment() {
        url.push('#');
        url.push_str(fragment);
    }
    Ok(Request {
        url,
        relative: relative.to_string(),
    })
}

fn validate_entry(entry: &FileEntry, absolute: &str, relative: &str) -> Result<(), String> {
    if entry.path != absolute {
        return Err("net.duck stat returned a different path.".into());
    }
    if entry.kind != "file" {
        return Err(format!("net.duck page is not a file: {relative}"));
    }
    if entry.size > gateway::MAX_FILE_BYTES {
        return Err(format!("net.duck page exceeds the file cap: {relative}"));
    }
    let mime = entry
        .meta
        .get("mime")
        .map(String::as_str)
        .unwrap_or_else(|| {
            if relative.to_ascii_lowercase().ends_with(".html") {
                "text/html"
            } else {
                "application/octet-stream"
            }
        });
    if mime != "text/html" {
        return Err("net.duck page is not HTML.".into());
    }
    Ok(())
}

fn build_document(
    request: Request,
    snapshot: String,
    entry: &FileEntry,
    bytes: Vec<u8>,
) -> Result<LocalDocument, String> {
    if u64::try_from(bytes.len()).ok() != Some(entry.size) {
        return Err("net.duck page changed while reading.".into());
    }
    let html = String::from_utf8(bytes).map_err(|_| "net.duck page is not valid UTF-8.")?;
    let parsed = parse_document(&html)?;
    let title = parsed.title;
    let document = format!(
        "<!doctype html><html{}><head><meta charset=\"utf-8\"><meta http-equiv=\"Content-Security-Policy\" content=\"{}\"><meta name=\"referrer\" content=\"no-referrer\"><title>{}</title>{}</head><body{}>{}</body></html>",
        parsed.html_attributes,
        escape_attribute(NETWORK_CSP),
        escape_text(&title),
        sanitize_fragment(&parsed.head),
        parsed.body_attributes,
        sanitize_fragment(&parsed.body),
    );
    if document.len() as u64 > gateway::MAX_FILE_BYTES {
        return Err("Sanitized net.duck page exceeds the render cap.".into());
    }
    Ok(LocalDocument {
        url: request.url,
        bytes: Arc::from(document.into_bytes()),
        snapshot,
        title,
    })
}

struct ParsedDocument {
    html_attributes: String,
    body_attributes: String,
    title: String,
    head: String,
    body: String,
}

fn parse_document(source: &str) -> Result<ParsedDocument, String> {
    let dom = html5ever::parse_document(RcDom::default(), Default::default()).one(source);
    let html = find_element(&dom.document, "html")
        .ok_or_else(|| "net.duck page has no HTML document root.".to_string())?;
    let head = find_element(&html, "head")
        .ok_or_else(|| "net.duck page has no document head.".to_string())?;
    let body = find_element(&html, "body")
        .ok_or_else(|| "net.duck page has no document body.".to_string())?;
    let title = find_element(&head, "title")
        .map(|node| text_content(&node))
        .unwrap_or_default();
    let title = title.trim().chars().take(128).collect::<String>();

    Ok(ParsedDocument {
        html_attributes: safe_attributes(&html),
        body_attributes: safe_attributes(&body),
        title: if title.is_empty() {
            "net.duck".into()
        } else {
            title
        },
        head: serialize_children(&head)?,
        body: serialize_children(&body)?,
    })
}

fn find_element(node: &Handle, tag: &str) -> Option<Handle> {
    if matches!(&node.data, NodeData::Element { name, .. } if name.local.as_ref() == tag) {
        return Some(node.clone());
    }
    node.children
        .borrow()
        .iter()
        .find_map(|child| find_element(child, tag))
}

fn text_content(node: &Handle) -> String {
    match &node.data {
        NodeData::Text { contents } => contents.borrow().to_string(),
        _ => node.children.borrow().iter().map(text_content).collect(),
    }
}

fn safe_attributes(node: &Handle) -> String {
    let NodeData::Element { attrs, .. } = &node.data else {
        return String::new();
    };
    attrs
        .borrow()
        .iter()
        .filter(|attribute| {
            attribute.name.prefix.is_none()
                && attribute.name.ns.as_ref().is_empty()
                && matches!(
                    attribute.name.local.as_ref(),
                    "id" | "class" | "title" | "lang" | "dir" | "role"
                )
        })
        .map(|attribute| {
            format!(
                " {}=\"{}\"",
                attribute.name.local,
                escape_attribute(&attribute.value)
            )
        })
        .collect()
}

fn serialize_children(node: &Handle) -> Result<String, String> {
    let mut bytes = Vec::new();
    let serializable: SerializableHandle = node.clone().into();
    html5ever::serialize(
        &mut bytes,
        &serializable,
        SerializeOpts {
            traversal_scope: TraversalScope::ChildrenOnly(None),
            ..Default::default()
        },
    )
    .map_err(|error| format!("Failed to normalize net.duck HTML: {error}"))?;
    String::from_utf8(bytes).map_err(|_| "Normalized net.duck HTML is not UTF-8.".into())
}

fn sanitize_fragment(html: &str) -> String {
    let mut tags = ALLOWED_TAGS.iter().copied().collect::<HashSet<_>>();
    tags.insert("style");
    let attributes = HashMap::from([
        ("a", HashSet::from(["href"])),
        ("img", HashSet::from(["src", "alt"])),
    ]);
    let clean = Builder::empty()
        .tags(tags)
        .clean_content_tags(CLEAN_CONTENT_TAGS.iter().copied().collect())
        .generic_attributes(HashSet::from([
            "id", "class", "title", "lang", "dir", "role",
        ]))
        .tag_attributes(attributes)
        .url_schemes(HashSet::from(["data"]))
        .url_relative(UrlRelative::PassThrough)
        .link_rel(None)
        .attribute_filter(|tag, attribute, value| match (tag, attribute) {
            ("a", "href") if value.starts_with('#') => Some(Cow::Borrowed(value)),
            ("img", "src") if is_safe_data_image(value) => Some(Cow::Borrowed(value)),
            ("img", "alt") => Some(Cow::Borrowed(value)),
            (_, "id" | "class" | "title" | "lang" | "dir" | "role") => Some(Cow::Borrowed(value)),
            _ => None,
        })
        .clean(html)
        .to_string();
    strip_blocked_styles(&clean)
}

fn is_safe_data_image(value: &str) -> bool {
    let prefix = value
        .get(..value.len().min(40))
        .unwrap_or(value)
        .to_ascii_lowercase();
    [
        "data:image/gif;base64,",
        "data:image/jpeg;base64,",
        "data:image/png;base64,",
        "data:image/webp;base64,",
    ]
    .iter()
    .any(|allowed| prefix.starts_with(allowed))
}

fn strip_blocked_styles(html: &str) -> String {
    // Ammonia has already parsed and serialized this fragment, so style tags
    // have canonical boundaries. Inspect only their raw-text contents: image
    // data elsewhere in the document must not disable otherwise-safe CSS.
    let lowercase = html.to_ascii_lowercase();
    let mut output = String::with_capacity(html.len());
    let mut copied = 0;
    let mut cursor = 0;
    while let Some(offset) = lowercase[cursor..].find("<style") {
        let start = cursor + offset;
        let after_name = start + "<style".len();
        if !lowercase
            .as_bytes()
            .get(after_name)
            .is_some_and(|byte| *byte == b'>' || byte.is_ascii_whitespace())
        {
            cursor = after_name;
            continue;
        }
        let Some(open_offset) = lowercase[after_name..].find('>') else {
            break;
        };
        let content_start = after_name + open_offset + 1;
        let close_start = lowercase[content_start..]
            .find("</style>")
            .map_or(html.len(), |offset| content_start + offset);
        let close_end = close_start.saturating_add("</style>".len()).min(html.len());
        if contains_blocked_css(&html[content_start..close_start]) {
            output.push_str(&html[copied..start]);
            copied = close_end;
        }
        cursor = close_end.max(content_start);
    }
    output.push_str(&html[copied..]);
    output
}

fn contains_blocked_css(css: &str) -> bool {
    let compact = normalize_css_for_scan(css);
    [
        "url(",
        "@import",
        "@namespace",
        "expression(",
        "behavior:",
        "-moz-binding",
        "javascript:",
        "data:",
    ]
    .iter()
    .any(|blocked| compact.contains(blocked))
}

fn normalize_css_for_scan(css: &str) -> String {
    let bytes = css.as_bytes();
    let mut normalized = String::with_capacity(css.len());
    let mut cursor = 0;
    while cursor < bytes.len() {
        if bytes[cursor..].starts_with(b"/*") {
            cursor += 2;
            while cursor < bytes.len() && !bytes[cursor..].starts_with(b"*/") {
                cursor += 1;
            }
            cursor = (cursor + 2).min(bytes.len());
            continue;
        }
        if matches!(bytes[cursor], b'\'' | b'"') {
            let quote = bytes[cursor];
            cursor += 1;
            while cursor < bytes.len() && bytes[cursor] != quote {
                if bytes[cursor] == b'\\' {
                    cursor += 1;
                    if cursor < bytes.len() && bytes[cursor] == b'\r' {
                        cursor += 1;
                        if cursor < bytes.len() && bytes[cursor] == b'\n' {
                            cursor += 1;
                        }
                        continue;
                    }
                }
                cursor += 1;
            }
            cursor = (cursor + 1).min(bytes.len());
            normalized.push('|');
            continue;
        }
        if bytes[cursor] == b'\\' {
            cursor += 1;
            if cursor >= bytes.len() {
                break;
            }
            if matches!(bytes[cursor], b'\n' | b'\r' | 0x0c) {
                if bytes[cursor] == b'\r'
                    && bytes.get(cursor + 1).is_some_and(|byte| *byte == b'\n')
                {
                    cursor += 1;
                }
                cursor += 1;
                continue;
            }
            let digits_start = cursor;
            let mut value = 0_u32;
            while cursor < bytes.len()
                && cursor - digits_start < 6
                && bytes[cursor].is_ascii_hexdigit()
            {
                value = value * 16 + u32::from(hex_value(bytes[cursor]));
                cursor += 1;
            }
            if cursor != digits_start {
                if cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
                    if bytes[cursor] == b'\r'
                        && bytes.get(cursor + 1).is_some_and(|byte| *byte == b'\n')
                    {
                        cursor += 1;
                    }
                    cursor += 1;
                }
                if let Some(decoded) =
                    char::from_u32(value).filter(|character| !character.is_ascii_whitespace())
                {
                    for character in decoded.to_lowercase() {
                        normalized.push(character);
                    }
                    // Fail closed for the common `\70p` spelling of an escaped
                    // `p`: consuming the repeated literal catches it without
                    // weakening how ordinary CSS is preserved.
                    if decoded.is_ascii()
                        && bytes
                            .get(cursor)
                            .is_some_and(|byte| byte.eq_ignore_ascii_case(&(decoded as u8)))
                    {
                        cursor += 1;
                    }
                } else {
                    normalized.push('|');
                }
                continue;
            }
            let escaped = bytes[cursor];
            normalized.push(if escaped.is_ascii() {
                escaped.to_ascii_lowercase() as char
            } else {
                '|'
            });
            cursor += 1;
            continue;
        }
        let byte = bytes[cursor];
        if !byte.is_ascii_whitespace() {
            normalized.push(if byte.is_ascii() {
                byte.to_ascii_lowercase() as char
            } else {
                '|'
            });
        }
        cursor += 1;
    }
    normalized
}

const fn hex_value(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        b'A'..=b'F' => byte - b'A' + 10,
        _ => 0,
    }
}

fn escape_attribute(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_text(value: &str) -> String {
    value.replace('&', "&amp;").replace('<', "&lt;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn entry(size: u64, kind: &str, mime: Option<&str>) -> FileEntry {
        FileEntry {
            path: format!("{NETWORK_CONTENT_ROOT}/index.html"),
            kind: kind.into(),
            size,
            exec: false,
            object: "11".repeat(32),
            meta: mime
                .map(|mime| BTreeMap::from([("mime".into(), mime.into())]))
                .unwrap_or_default(),
        }
    }

    #[test]
    fn parses_only_exact_canonical_network_html_paths() {
        let root = parse_request("duck://net.duck").unwrap();
        assert_eq!(root.relative, "index.html");
        assert_eq!(root.url, "duck://net.duck/");
        let anchored = parse_request("duck://net.duck/docs/start.html#part").unwrap();
        assert_eq!(anchored.relative, "docs/start.html");
        assert_eq!(anchored.url, "duck://net.duck/docs/start.html#part");
        assert_eq!(
            parse_request("duck://net.duck/docs/start.HTML")
                .unwrap()
                .relative,
            "docs/start.HTML"
        );
        for rejected in [
            "https://net.duck/index.html",
            "duck://api.net.duck/index.html",
            "duck://net.duck:80/index.html",
            "duck://user@net.duck/index.html",
            "duck://net.duck/docs/../index.html",
            "duck://net.duck/%2e%2e/secret.html",
            "duck://net.duck/docs\\index.html",
            "duck://net.duck/index.html?x=1",
            "duck://net.duck/app.js",
        ] {
            assert!(parse_request(rejected).is_err(), "accepted {rejected}");
        }
    }

    #[test]
    fn sanitizer_keeps_only_inert_markup_links_images_and_css() {
        let safe = sanitize_fragment(
            r##"<style>body { color: #222 }</style><main id="home" onclick="steal()">
            <a href="#part">inside</a><a href="https://evil.test">outside</a>
            <img alt="ok" src="data:image/png;base64,AAAA"><img src="data:image/svg+xml;base64,AAAA">
            <script>fetch('https://evil.test')</script><iframe src="https://evil.test">bad</iframe>
            </main>"##,
        );
        assert!(safe.contains("<style>body { color: #222 }</style>"));
        assert!(safe.contains("href=\"#part\""));
        assert!(safe.contains("data:image/png;base64,AAAA"));
        for forbidden in [
            "onclick",
            "https://evil.test",
            "data:image/svg",
            "<script",
            "<iframe",
            "fetch(",
        ] {
            assert!(!safe.contains(forbidden), "kept {forbidden}: {safe}");
        }

        let unsafe_css = sanitize_fragment(
            "<style>@import 'https://evil.test/x.css'; body { color:red }</style><p>ok</p>",
        );
        assert!(!unsafe_css.contains("<style"));
        assert_eq!(unsafe_css, "<p>ok</p>");

        let prefixed_end_tag = sanitize_fragment(
            "<style>p{color:red}</stylesheet>@import 'https://evil.test/x.css'</style><p>ok</p>",
        );
        assert!(!prefixed_end_tag.contains("<style"));
        assert_eq!(prefixed_end_tag, "<p>ok</p>");

        for escaped_or_split in [
            r#"<style>main{background:u\72l(https://evil.test/x)}</style><p>ok</p>"#,
            r#"<style>@im\70port "https://evil.test/x.css";</style><p>ok</p>"#,
            "<style>main{background:u/**/rl(https://evil.test/x)}</style><p>ok</p>",
            "<style>@im/**/port 'https://evil.test/x.css';</style><p>ok</p>",
        ] {
            let sanitized = sanitize_fragment(escaped_or_split);
            assert_eq!(sanitized, "<p>ok</p>", "kept escaped CSS: {sanitized}");
        }

        let safe_css_text = sanitize_fragment(
            r#"<style>/* harmless */ p::before { content: "url( and /* stay text"; color: teal }</style><p>ok</p>"#,
        );
        assert!(safe_css_text.contains("<style>"));
        assert!(safe_css_text.contains("color: teal"));

        let full_document = sanitize_fragment(
            "<!doctype html><html><head><title>must not leak</title><style>body { color: teal }</style></head><body><p>body stays</p></body></html>",
        );
        assert!(!full_document.contains("must not leak"));
        assert!(full_document.contains("<style>body { color: teal }</style>"));
        assert!(full_document.contains("<p>body stays</p>"));
    }

    #[test]
    fn local_document_preserves_safe_document_shape_and_is_sandboxed() {
        let long_title = "N".repeat(140);
        let html = format!(
            "<!doctype html><html lang=\"en\" data-bad=\"drop\"><head><title>  {long_title}  </title><style>body {{ color: teal }}</style></head><body id=\"root\" role=\"main\" onclick=\"steal()\"><main title=\"safe\">hello &amp; net</main></body></html>"
        )
        .into_bytes();
        let document = build_document(
            parse_request("duck://net.duck").unwrap(),
            "22".repeat(32),
            &entry(html.len() as u64, "file", Some("text/html")),
            html,
        )
        .unwrap();
        let rendered = String::from_utf8(document.bytes.to_vec()).unwrap();
        assert_eq!(document.snapshot, "22".repeat(32));
        assert_eq!(document.url, "duck://net.duck/");
        assert_eq!(document.title, "N".repeat(128));
        assert!(rendered.starts_with("<!doctype html><html lang=\"en\"><head>"));
        assert!(rendered.contains("<title>"));
        assert!(rendered.contains("<style>body { color: teal }</style></head>"));
        assert!(rendered.contains("<body id=\"root\" role=\"main\"><main title=\"safe\">"));
        assert!(!rendered.contains("<iframe"));
        assert!(!rendered.contains("srcdoc"));
        assert!(!rendered.contains("data-bad"));
        assert!(!rendered.contains("onclick"));
        assert!(rendered.contains("sandbox; default-src 'none'"));
        assert!(rendered.contains("name=\"referrer\" content=\"no-referrer\""));
        assert!(rendered.contains("script-src 'none'"));
        assert!(rendered.contains("hello &amp; net"));

        let absolute = format!("{NETWORK_CONTENT_ROOT}/index.html");
        assert!(
            validate_entry(&entry(1, "dir", Some("text/html")), &absolute, "index.html").is_err()
        );
        assert!(
            validate_entry(
                &entry(1, "file", Some("text/plain")),
                &absolute,
                "index.html"
            )
            .is_err()
        );
        assert!(
            validate_entry(
                &entry(gateway::MAX_FILE_BYTES + 1, "file", Some("text/html")),
                &absolute,
                "index.html",
            )
            .is_err()
        );
        let mut wrong_path = entry(1, "file", Some("text/html"));
        wrong_path.path = format!("{NETWORK_CONTENT_ROOT}/other.html");
        assert!(validate_entry(&wrong_path, &absolute, "index.html").is_err());
    }

    #[test]
    fn strict_utf8_and_exact_read_size_are_required() {
        let request = parse_request("duck://net.duck").unwrap();
        assert!(
            build_document(
                request.clone(),
                "33".repeat(32),
                &entry(2, "file", None),
                vec![b'a'],
            )
            .is_err()
        );
        assert!(
            build_document(
                request,
                "33".repeat(32),
                &entry(1, "file", None),
                vec![0xff],
            )
            .is_err()
        );
    }
}
