//! Display projections only. Paths, IDs and editor seeds are never shortened.
//! The conservative charge includes repeated labels, rich spans and per-row
//! furniture; the actual guest-frame tests are the no-sanitizer-loss oracle.
use serde_json::Value;

const DISPLAY_BUDGET: usize = 48 << 10;
const TEXT_PREFIX: usize = 2 << 10;

fn charge(value: &Value) -> usize {
    let children = match value {
        Value::String(text) => text.len().saturating_mul(4),
        Value::Array(values) => values
            .iter()
            .fold(0usize, |sum, v| sum.saturating_add(charge(v))),
        Value::Object(fields) => fields
            .values()
            .fold(0usize, |sum, v| sum.saturating_add(charge(v))),
        _ => 0,
    };
    children.saturating_add(64)
}

fn prefix(value: &mut Value, limit: usize) -> bool {
    let Some(text) = value.as_str() else {
        return false;
    };
    let (head, clipped) = super::head_within(text, limit);
    if clipped {
        *value = head.into();
    }
    clipped
}

// Only display text is eligible. In particular `path`, `id`, `anchor`, `link`,
// and scope strings survive byte-for-byte, even when that means omitting a row.
fn shorten_record(value: &mut Value) -> bool {
    match value {
        Value::Array(rows) => rows
            .iter_mut()
            .fold(false, |cut, row| shorten_record(row) | cut),
        Value::Object(fields) => {
            let mut clipped = false;
            for (key, value) in fields {
                if matches!(
                    key.as_str(),
                    "body"
                        | "text"
                        | "mention"
                        | "link_text"
                        | "bold_italic"
                        | "bold"
                        | "italic"
                        | "plain"
                ) {
                    clipped |= prefix(value, TEXT_PREFIX);
                } else {
                    clipped |= shorten_record(value);
                }
            }
            clipped
        }
        _ => false,
    }
}

/// The production Forge encoder. The landing note and newest discussion get
/// first claim around the item body, then the other lists. Omitted rows are counted.
pub(super) fn forge(mut props: Value) -> Vec<u8> {
    let arrays = [
        "linked_note",
        "forge_item_blocks",
        "discussion",
        "diff_rows",
        "staged_comments",
        "forge_item_reviews",
        "repos",
        "items",
        "branches",
        "merge_conflicts",
        "tree_entries",
    ];
    project(&mut props, &arrays, &["forge_item_body"], "file_text");
    serde_json::to_vec(&props).expect("forge props encode")
}

/// The read remains the edit seed. Only its separate preview crosses as a
/// rendered Surface argument, and display clipping never changes read status.
pub(super) fn files(mut props: Value) -> Vec<u8> {
    props["preview_display_text"] = props["preview_text"].clone();
    let arrays = ["entries", "directories", "history", "diff"];
    project(
        &mut props,
        &arrays,
        &["preview_text"],
        "preview_display_text",
    );
    props["preview_display_clipped"] =
        (props["preview_display_text"] != props["preview_text"]).into();
    serde_json::to_vec(&props).expect("files props encode")
}

fn project(props: &mut Value, arrays: &[&str], source_fields: &[&str], preview: &str) {
    let mut pending = Vec::with_capacity(arrays.len());
    for key in arrays {
        let rows = props[*key].take().as_array().cloned().unwrap_or_default();
        props[*key] = Value::Array(Vec::new());
        pending.push((*key, rows));
    }
    let preview_text = props[preview].take();
    props[preview] = "".into();
    // Sources stay in props/state, but do not render. Do not charge a complete
    // read against its display projection, or turn it into a shortened editor.
    let source: Vec<_> = source_fields
        .iter()
        .map(|key| (*key, props[*key].take()))
        .collect();
    let baseline = charge(props);
    for (key, value) in source {
        props[key] = value;
    }
    let unavailable = baseline > DISPLAY_BUDGET;
    let mut left = DISPLAY_BUDGET.saturating_sub(baseline);
    let mut omitted = 0usize;
    // Give an open preview a useful prefix before long listings spend the rest.
    let mut text = preview_text;
    let mut shortened = prefix(&mut text, left.saturating_sub(64) / 8);
    left = left.saturating_sub(charge(&text));
    props[preview] = text;
    for (key, mut rows) in pending {
        let newest = key == "discussion";
        if newest {
            rows.reverse();
        }
        let mut kept = Vec::new();
        let mut exhausted = false;
        for mut row in rows {
            let cut = shorten_record(&mut row);
            let cost = charge(&row);
            if exhausted || cost > left {
                omitted = omitted.saturating_add(1);
                exhausted = true;
                if newest {
                    props["discussion_clipped"] = true.into();
                }
            } else {
                left -= cost;
                shortened |= cut;
                kept.push(row);
            }
        }
        if newest {
            kept.reverse();
        }
        props[key] = kept.into();
    }
    props["display_omitted"] = (omitted.min(i64::MAX as usize) as i64).into();
    props["display_shortened"] = shortened.into();
    props["display_unavailable"] = unavailable.into();
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn source_and_route_identity_survive_display_clipping() {
        let source = "한글".repeat(12_000);
        let input = json!({"preview_text": source, "preview_truncated": false,
            "preview_path": "/shared/source", "entries": [{"key": 17, "path": "/shared/entry", "name": "entry"}],
            "directories": [], "history": [], "diff": []});
        let result: Value = serde_json::from_slice(&files(input)).unwrap();
        assert_eq!(result["preview_text"], source);
        assert_eq!(result["preview_truncated"], false);
        assert_eq!(result["preview_path"], "/shared/source");
        assert_eq!(result["entries"][0]["key"], 17);
        assert_eq!(result["entries"][0]["path"], "/shared/entry");
        assert_eq!(result["preview_display_clipped"], true);
        assert!(result["preview_display_text"].as_str().unwrap().len() < source.len());
    }

    #[test]
    fn counted_omissions_keep_the_newest_contiguous_tail() {
        let rows: Vec<_> = (0..100)
            .map(|n| json!({"id": n, "body": "note".repeat(700)}))
            .collect();
        let result: Value =
            serde_json::from_slice(&forge(json!({"discussion": rows, "file_text": ""}))).unwrap();
        let kept = result["discussion"].as_array().unwrap();
        assert_eq!(kept.last().unwrap()["id"], 99);
        let first = kept.first().unwrap()["id"].as_i64().unwrap();
        assert_eq!(result["display_omitted"], first);
        assert!(first > 0);
        assert_eq!(result["discussion_clipped"], true);
        for (offset, row) in kept.iter().enumerate() {
            assert_eq!(row["id"], first + offset as i64);
        }
    }

    #[test]
    fn oversized_identity_gets_an_explicit_unavailable_view_not_a_changed_path() {
        let path = "/".repeat(20_000);
        let result: Value =
            serde_json::from_slice(&files(json!({"path": path, "preview_text": "original"})))
                .unwrap();
        assert_eq!(result["path"], path);
        assert_eq!(result["preview_text"], "original");
        assert_eq!(result["display_unavailable"], true);
    }
}
