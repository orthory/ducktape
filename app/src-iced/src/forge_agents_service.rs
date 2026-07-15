//! Native service edge for the transport-free Forge and Agents screens.
//!
//! The public facade stays stable while the two concrete domains keep their
//! repository and consensus concerns isolated. Shared code is limited to the
//! signed-frame and bounded wire primitives used by both domains.

mod agents;
mod forge;

pub use agents::{execute_agents, run_output_subscription};
pub use forge::execute_forge;

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use crate::backend::{Backend, ContentTarget};
use crate::transport::NodeClient;

const MAX_SIGNED_PAYLOAD_BYTES: usize = 23 * 1024;

async fn query(node: Option<&NodeClient>, target: &str, value: Value) -> Result<Value, String> {
    node.ok_or_else(|| format!("enter a network to use {target}"))?
        .query(target, value)
        .await
        .map_err(|error| error.to_string())
}

async fn submit_signed(
    backend: Option<&Backend>,
    node: Option<&NodeClient>,
    target: ContentTarget,
    payload: Value,
) -> Result<(), String> {
    let backend = backend.ok_or_else(|| "desktop identity backend is unavailable".to_string())?;
    let client = node.ok_or_else(|| format!("enter a network to use {}", target.as_str()))?;
    let bytes = serde_json::to_vec(&payload)
        .map_err(|error| format!("could not encode {} write: {error}", target.as_str()))?;
    if bytes.is_empty() || bytes.len() > MAX_SIGNED_PAYLOAD_BYTES {
        return Err(format!(
            "{} write exceeds the signed-frame payload limit",
            target.as_str()
        ));
    }
    let frame = backend
        .sign_content_frame(target, hex_encode(&bytes))
        .await?;
    client
        .submit_frame(hex_decode(&frame)?)
        .await
        .map(|_| ())
        .map_err(|error| error.to_string())
}

async fn current_identity_key(backend: Option<&Backend>) -> Result<Option<String>, String> {
    match backend {
        Some(backend) => backend.identity_state().await.map(|state| state.pubkey),
        None => Ok(None),
    }
}

fn variant_array<'a>(value: &'a Value, key: &str, max: usize) -> Result<&'a Vec<Value>, String> {
    let rows = value
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("node returned an invalid {key} reply"))?;
    if rows.len() > max {
        return Err(format!(
            "node returned too many {key} rows ({} > {max})",
            rows.len()
        ));
    }
    Ok(rows)
}

fn bounded_string(value: &Value, field: &str, max: usize) -> Result<String, String> {
    let text = value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("record is missing {field}"))?;
    validate_bounded_text(text, field, max, true)?;
    Ok(text.to_owned())
}

fn optional_string(value: &Value, field: &str, max: usize) -> Result<Option<String>, String> {
    match value.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => {
            validate_bounded_text(text, field, max, true)?;
            Ok(Some(text.clone()))
        }
        _ => Err(format!("record contains an invalid {field}")),
    }
}

fn required_u64(value: &Value, field: &str) -> Result<u64, String> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("record is missing {field}"))
}

fn validate_bounded_text(
    text: &str,
    field: &str,
    max: usize,
    allow_empty: bool,
) -> Result<(), String> {
    if (!allow_empty && text.is_empty()) || text.len() > max || text.contains('\0') {
        return Err(format!("{field} is missing, too long, or contains NUL"));
    }
    Ok(())
}

fn bytes_vec(value: &Value) -> Result<Vec<u8>, String> {
    let bytes = value
        .as_array()
        .ok_or_else(|| "wire key is not a byte array".to_string())?;
    if bytes.is_empty() || bytes.len() > 64 {
        return Err("wire key is outside the 1..=64 byte bound".into());
    }
    bytes
        .iter()
        .map(|byte| {
            byte.as_u64()
                .and_then(|byte| u8::try_from(byte).ok())
                .ok_or_else(|| "wire key contains an invalid byte".to_string())
        })
        .collect()
}

fn bytes_hex(value: &Value) -> Result<String, String> {
    bytes_vec(value).map(|bytes| hex_encode(&bytes))
}

fn format_stamp(stamp: u64) -> String {
    let seconds = if stamp >= 1_000_000_000_000 {
        stamp / 1_000
    } else if stamp >= 1_000_000_000 {
        stamp
    } else {
        return String::new();
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(seconds);
    let delta = now.saturating_sub(seconds);
    match delta {
        0..=59 => "now".into(),
        60..=3_599 => format!("{}m ago", delta / 60),
        3_600..=86_399 => format!("{}h ago", delta / 3_600),
        _ => format!("{}d ago", delta / 86_400),
    }
}

fn fresh_id(prefix: &str) -> String {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}-{millis:x}-{:x}-{sequence:x}", std::process::id())
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[usize::from(byte >> 4)] as char);
        output.push(HEX[usize::from(byte & 0x0f)] as char);
    }
    output
}

fn hex_decode(value: &str) -> Result<Vec<u8>, String> {
    if value.is_empty()
        || !value.len().is_multiple_of(2)
        || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("signed frame is not even-length hexadecimal".into());
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let text = std::str::from_utf8(pair).map_err(|error| error.to_string())?;
            u8::from_str_radix(text, 16).map_err(|error| error.to_string())
        })
        .collect()
}

fn validate_digest(value: &str) -> Result<(), String> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("blob digest must be 64-character hexadecimal".into());
    }
    Ok(())
}
