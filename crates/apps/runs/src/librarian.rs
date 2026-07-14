//! Strict, side-effect-free validation for the dormant Librarian wire.
//!
//! These parsers reject an untrusted value as a whole. In particular, they do
//! not trim or truncate model output into something later code might trust.

use crate::{
    ANSWER_MAX_BYTES, CALL_ID_MAX_BYTES, LibrarianAnswerPayload, LibrarianCallRequest,
    LibrarianCallResult, MAX_ENCODED_ANSWER_BYTES, MAX_ENTRY_BYTES, MAX_EVIDENCE_REFS,
    MAX_UNCERTAINTIES, QUESTION_MAX_BYTES,
};

fn required_bounded(name: &str, value: &str, max: usize) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("{name} must not be empty"));
    }
    if value.len() > max {
        return Err(format!(
            "{name} is {} UTF-8 bytes; the cap is {max}",
            value.len()
        ));
    }
    Ok(())
}

/// Validate the only two public request arguments.
pub fn validate_librarian_call_request(request: &LibrarianCallRequest) -> Result<(), String> {
    required_bounded("call_id", &request.call_id, CALL_ID_MAX_BYTES)?;
    required_bounded("question", &request.question, QUESTION_MAX_BYTES)
}

/// Strictly decode and validate an untrusted request object.
pub fn decode_librarian_call_request(bytes: &[u8]) -> Result<LibrarianCallRequest, String> {
    let request: LibrarianCallRequest =
        serde_json::from_slice(bytes).map_err(|e| format!("librarian request decode: {e}"))?;
    validate_librarian_call_request(&request)?;
    Ok(request)
}

/// Validate a decoded answer payload without changing it.
pub fn validate_librarian_answer_payload(payload: &LibrarianAnswerPayload) -> Result<(), String> {
    required_bounded("answer", &payload.answer, ANSWER_MAX_BYTES)?;
    if payload.evidence_refs.len() > MAX_EVIDENCE_REFS {
        return Err(format!(
            "evidence_refs carries {} entries; the cap is {MAX_EVIDENCE_REFS}",
            payload.evidence_refs.len()
        ));
    }
    if payload.uncertainties.len() > MAX_UNCERTAINTIES {
        return Err(format!(
            "uncertainties carries {} entries; the cap is {MAX_UNCERTAINTIES}",
            payload.uncertainties.len()
        ));
    }
    for (index, value) in payload.evidence_refs.iter().enumerate() {
        required_bounded(&format!("evidence_refs[{index}]"), value, MAX_ENTRY_BYTES)?;
    }
    for (index, value) in payload.uncertainties.iter().enumerate() {
        required_bounded(&format!("uncertainties[{index}]"), value, MAX_ENTRY_BYTES)?;
    }
    let encoded = serde_json::to_vec(payload).expect("LibrarianAnswerPayload is serializable");
    if encoded.len() > MAX_ENCODED_ANSWER_BYTES {
        return Err(format!(
            "encoded librarian answer is {} bytes; the cap is {MAX_ENCODED_ANSWER_BYTES}",
            encoded.len()
        ));
    }
    Ok(())
}

fn validate_node_id(node: &str) -> Result<(), String> {
    if node.len() != 64
        || !node
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("provenance must carry a 64-character lowercase hex node id".into());
    }
    Ok(())
}

/// Render a validated consensus node key as public Librarian provenance.
pub fn librarian_provenance(node_id: &str) -> Result<String, String> {
    validate_node_id(node_id)?;
    Ok(format!("{node_id}@nodes.duck"))
}

fn validate_provenance(value: &str) -> Result<(), String> {
    const SUFFIX: &str = "@nodes.duck";
    let Some(node) = value.strip_suffix(SUFFIX) else {
        return Err("provenance must be <lowercase-node-id>@nodes.duck".into());
    };
    validate_node_id(node)
}

/// Validate a decoded result, including child-run and node provenance data.
pub fn validate_librarian_call_result(result: &LibrarianCallResult) -> Result<(), String> {
    validate_librarian_answer_payload(&result.answer)?;
    if result.child_run_id.is_empty() {
        return Err("child_run_id must not be empty".into());
    }
    validate_provenance(&result.provenance)
}

/// Strictly decode an answer payload under the aggregate encoded-size fence.
pub fn decode_librarian_answer(bytes: &[u8]) -> Result<LibrarianAnswerPayload, String> {
    if bytes.len() > MAX_ENCODED_ANSWER_BYTES {
        return Err(format!(
            "encoded librarian answer is {} bytes; the cap is {MAX_ENCODED_ANSWER_BYTES}",
            bytes.len()
        ));
    }
    let payload: LibrarianAnswerPayload =
        serde_json::from_slice(bytes).map_err(|e| format!("librarian answer decode: {e}"))?;
    validate_librarian_answer_payload(&payload)?;
    Ok(payload)
}

/// Strictly decode a complete future call result under the same encoded fence.
pub fn decode_librarian_call_result(bytes: &[u8]) -> Result<LibrarianCallResult, String> {
    if bytes.len() > MAX_ENCODED_ANSWER_BYTES {
        return Err(format!(
            "encoded librarian answer is {} bytes; the cap is {MAX_ENCODED_ANSWER_BYTES}",
            bytes.len()
        ));
    }
    let result: LibrarianCallResult =
        serde_json::from_slice(bytes).map_err(|e| format!("librarian result decode: {e}"))?;
    validate_librarian_call_result(&result)?;
    Ok(result)
}
