use std::ops::Range;

/// A renderer-parity inline mark, plus the marker glyphs themselves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Inline {
    Marker,
    Bold,
    Italic,
    Link,
}

/// Byte-ranged mirror of `chat::client::inline_spans`, minus mentions: bare
/// `http(s)://` runs, then `**`/`__` bold, then `*`/`_` italic; unmatched or
/// empty fences stay plain. Ranges land on char boundaries by construction —
/// the scanner only advances through `char_indices`.
pub fn inline_marks(line: &str) -> Vec<(Range<usize>, Inline)> {
    let mut marks = Vec::new();
    let mut at = 0;
    while at < line.len() {
        let rest = &line[at..];
        // Only these bytes can open the existing grammar. Skip ordinary prose
        // in one scan instead of retrying every delimiter at every character.
        if !matches!(rest.as_bytes()[0], b'*' | b'_' | b'h') {
            let Some(next) = rest.find(['*', '_', 'h']) else {
                break;
            };
            at += next;
            continue;
        }
        if let Some(len) = url_len(rest) {
            marks.push((at..at + len, Inline::Link));
            at += len;
            continue;
        }
        let fence = ["**", "__", "*", "_"]
            .iter()
            .find_map(|marker| fenced(rest, marker));
        let Some((marker_len, inner_len)) = fence else {
            at += rest.chars().next().map_or(1, char::len_utf8);
            continue;
        };
        let bold = marker_len == 2;
        let kind = if bold { Inline::Bold } else { Inline::Italic };
        let body = at + marker_len..at + marker_len + inner_len;
        marks.push((at..body.start, Inline::Marker));
        marks.push((body.clone(), kind));
        marks.push((body.end..body.end + marker_len, Inline::Marker));
        at = body.end + marker_len;
    }
    marks
}

/// If `rest` opens with `marker` and a later closing `marker` encloses a
/// non-empty body, `(marker byte length, body byte length)`.
fn fenced(rest: &str, marker: &str) -> Option<(usize, usize)> {
    let body = rest.strip_prefix(marker)?;
    let close = body.find(marker)?;
    if close == 0 {
        return None;
    }
    Some((marker.len(), close))
}

/// If `rest` opens a bare link, its byte length: the renderer's rule — an
/// `http(s)://` prefix, then everything up to whitespace.
fn url_len(rest: &str) -> Option<usize> {
    let starts_link = rest.starts_with("http://") || rest.starts_with("https://");
    if !starts_link {
        return None;
    }
    let len = rest.find(char::is_whitespace).unwrap_or(rest.len());
    Some(len)
}
