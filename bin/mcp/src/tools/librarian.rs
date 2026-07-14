//! Test-only candidate declaration for the post-re-genesis Librarian tool.
//!
//! This module is deliberately absent from `all`, `find`, and every live
//! routing/signing path. Keeping the schema beside the registry lets tests pin
//! the future public surface without making it discoverable or invocable.

use serde_json::{Value, json};

pub(super) fn schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "call_id": {"type": "string"},
            "question": {"type": "string"}
        },
        "required": ["call_id", "question"],
        "additionalProperties": false
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_schema_is_exact_and_not_registered() {
        assert_eq!(
            schema(),
            json!({
                "type": "object",
                "properties": {
                    "call_id": {"type": "string"},
                    "question": {"type": "string"}
                },
                "required": ["call_id", "question"],
                "additionalProperties": false
            })
        );
        assert!(crate::tools::find("ducktape_ask_librarian").is_none());
        assert!(
            !crate::tools::list()
                .to_string()
                .contains("ducktape_ask_librarian")
        );
    }
}
