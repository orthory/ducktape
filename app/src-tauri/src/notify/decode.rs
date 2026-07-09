use std::fmt::Write as _;

use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub enum OriginKind {
    External,
    Module,
    System,
}

#[derive(Debug, Clone)]
pub struct Origin {
    pub kind: OriginKind,
    pub id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OpRow {
    // These block coordinates are retained for future toast timestamps and debugging.
    #[allow(dead_code)]
    pub height: u64,
    #[allow(dead_code)]
    pub seq: u32,
    #[allow(dead_code)]
    pub time: u64,
    pub origin: Origin,
    /// the embedded op payload; None when the row carried `payloadHex` or nothing.
    pub payload: Option<Value>,
}

/// Decode one OpRow envelope. Returns None on any malformed/missing field
/// (the notifier skips what it cannot read -- never panics on wire data).
pub fn decode_op_row(v: &Value) -> Option<OpRow> {
    let obj = v.as_object()?;
    let height = obj.get("height")?.as_u64()?;
    let seq = u32::try_from(obj.get("seq")?.as_u64()?).ok()?;
    let time = obj.get("time")?.as_u64()?;
    let origin = decode_origin(obj.get("origin")?)?;
    let payload = obj.get("payload").cloned();

    Some(OpRow {
        height,
        seq,
        time,
        origin,
        payload,
    })
}

fn decode_origin(v: &Value) -> Option<Origin> {
    let obj = v.as_object()?;
    let kind = match obj.get("kind")?.as_str()? {
        "external" => OriginKind::External,
        "module" => OriginKind::Module,
        "system" => OriginKind::System,
        _ => return None,
    };
    let id = match obj.get("id") {
        Some(id) => Some(id.as_str()?.to_string()),
        None => None,
    };

    Some(Origin { kind, id })
}

/// `payload.get(variant)` for a snake_case-tagged enum op -- Some(fields) when
/// this op is that variant.
pub fn variant<'a>(payload: &'a Value, name: &str) -> Option<&'a Value> {
    payload.get(name)
}

/// A JSON number-array of bytes -> lowercase hex ("" for empty array,
/// None when not an array of 0..=255 numbers).
pub fn bytes_hex(v: &Value) -> Option<String> {
    let bytes = v.as_array()?;
    let mut hex = String::with_capacity(bytes.len() * 2);

    for byte in bytes {
        let byte = u8::try_from(byte.as_u64()?).ok()?;
        write!(&mut hex, "{byte:02x}").ok()?;
    }

    Some(hex)
}

/// Walk post_message `blocks` (paragraph/quote spans -> marks) and collect the
/// lowercase-hex user keys of every `{"mention":{"user":[..]}}` mark.
pub fn mention_user_hexes(blocks: &Value) -> Vec<String> {
    let Some(blocks) = blocks.as_array() else {
        return Vec::new();
    };

    let mut users = Vec::new();
    for block in blocks {
        for spans in [variant(block, "paragraph"), variant(block, "quote")]
            .into_iter()
            .flatten()
        {
            collect_span_mentions(spans, &mut users);
        }
    }
    users
}

fn collect_span_mentions(spans: &Value, users: &mut Vec<String>) {
    let Some(spans) = spans.as_array() else {
        return;
    };

    for span in spans {
        let Some(marks) = span.get("marks").and_then(Value::as_array) else {
            continue;
        };
        for mark in marks {
            if let Some(user) = mark
                .get("mention")
                .and_then(|mention| mention.get("user"))
                .and_then(bytes_hex)
            {
                users.push(user);
            }
        }
    }
}

/// Flatten blocks to a short plain-text preview (paragraph/quote span text
/// joined, code -> its text, divider skipped), truncated to `max` chars on a
/// char boundary.
pub fn blocks_preview(blocks: &Value, max: usize) -> String {
    let mut preview = String::new();
    let Some(blocks) = blocks.as_array() else {
        return preview;
    };

    for block in blocks {
        let mut block_text = String::new();
        append_text_block(&mut block_text, variant(block, "paragraph"));
        append_text_block(&mut block_text, variant(block, "quote"));
        append_code_block(&mut block_text, variant(block, "code"));

        let block_text = block_text.trim();
        if block_text.is_empty() {
            continue;
        }

        if !preview.is_empty() {
            preview.push(' ');
        }
        preview.push_str(block_text);

        if preview.chars().count() >= max {
            break;
        }
    }

    truncate_chars(&preview, max)
}

fn append_text_block(preview: &mut String, block: Option<&Value>) {
    let Some(spans) = block.and_then(Value::as_array) else {
        return;
    };

    for span in spans {
        if let Some(text) = span.get("text").and_then(Value::as_str) {
            preview.push_str(text);
        }
    }
}

fn append_code_block(preview: &mut String, block: Option<&Value>) {
    let Some(block) = block else {
        return;
    };

    if let Some(text) = block.get("text").and_then(Value::as_str) {
        preview.push_str(text);
    }
}

fn truncate_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn decode_op_row_reads_full_envelope_and_payload_variant() {
        let row = decode_op_row(&json!({
            "height": 42,
            "seq": 1,
            "time": 1720000000,
            "origin": {
                "kind": "external",
                "id": "aabb"
            },
            "payload": {
                "post_message": {
                    "channel_id": "general",
                    "message_id": "m1",
                    "blocks": [],
                    "thread": null,
                    "as_agent": null
                }
            }
        }))
        .unwrap();

        assert_eq!(row.height, 42);
        assert_eq!(row.seq, 1);
        assert_eq!(row.time, 1720000000);
        assert_eq!(row.origin.kind, OriginKind::External);
        assert_eq!(row.origin.id.as_deref(), Some("aabb"));

        let payload = row.payload.as_ref().unwrap();
        assert!(variant(payload, "post_message").is_some());
    }

    #[test]
    fn decode_op_row_reads_payload_hex_row_without_payload() {
        let row = decode_op_row(&json!({
            "height": 43,
            "seq": 2,
            "time": 1720000001,
            "origin": {
                "kind": "system"
            },
            "payloadHex": "deadbeef"
        }))
        .unwrap();

        assert_eq!(row.height, 43);
        assert_eq!(row.seq, 2);
        assert_eq!(row.time, 1720000001);
        assert_eq!(row.origin.kind, OriginKind::System);
        assert_eq!(row.origin.id, None);
        assert_eq!(row.payload, None);
    }

    #[test]
    fn decode_op_row_rejects_malformed_rows_without_panicking() {
        assert!(decode_op_row(&json!({
            "seq": 1,
            "time": 1720000000,
            "origin": {
                "kind": "external"
            }
        }))
        .is_none());

        assert!(decode_op_row(&json!({
            "height": 42,
            "seq": 1,
            "time": 1720000000,
            "origin": "external"
        }))
        .is_none());

        assert!(decode_op_row(&json!({
            "height": 42,
            "seq": "1",
            "time": 1720000000,
            "origin": {
                "kind": "external"
            }
        }))
        .is_none());
    }

    #[test]
    fn bytes_hex_decodes_byte_arrays() {
        assert_eq!(bytes_hex(&json!([])), Some(String::new()));
        assert_eq!(bytes_hex(&json!([18, 52])), Some("1234".to_string()));
        assert_eq!(bytes_hex(&json!([256])), None);
        assert_eq!(bytes_hex(&json!("x")), None);
    }

    #[test]
    fn mention_user_hexes_collects_only_user_mentions() {
        let mentions = mention_user_hexes(&json!([
            {
                "paragraph": [
                    {
                        "text": "hi ",
                        "marks": []
                    },
                    {
                        "text": "@jess",
                        "marks": [
                            {
                                "mention": {
                                    "user": [18, 52]
                                }
                            }
                        ]
                    }
                ]
            },
            {
                "quote": [
                    {
                        "text": "q",
                        "marks": [
                            {
                                "mention": {
                                    "agent": {
                                        "module": "runs",
                                        "agent_id": "helper"
                                    }
                                }
                            }
                        ]
                    }
                ]
            },
            "divider"
        ]));

        assert_eq!(mentions, vec!["1234"]);
    }

    #[test]
    fn blocks_preview_joins_text_and_truncates_on_char_boundary() {
        let blocks = json!([
            {
                "paragraph": [
                    {
                        "text": "hi ",
                        "marks": []
                    },
                    {
                        "text": "🙂 there",
                        "marks": []
                    }
                ]
            },
            {
                "quote": [
                    {
                        "text": "quoted",
                        "marks": []
                    }
                ]
            },
            {
                "paragraph": [
                    {
                        "text": "",
                        "marks": []
                    }
                ]
            },
            "divider",
            {
                "code": {
                    "text": "code"
                }
            },
            "divider"
        ]);

        assert_eq!(blocks_preview(&blocks, 4), "hi 🙂");
        assert_eq!(blocks_preview(&blocks, 14), "hi 🙂 there quo");
        assert_eq!(blocks_preview(&blocks, 64), "hi 🙂 there quoted code");
    }
}
