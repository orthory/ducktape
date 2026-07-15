//! Shared `.duck` Browser authority validation.

/// Validate the shared Browser authority contract. Reserved DuckDNS roots have
/// no account gateway; only the exact network-owned `net.duck` origin exists.
pub(crate) fn validate_duck_host(host: &str) -> Result<(), String> {
    if host.contains(':') || !host.is_ascii() {
        return Err("Enter <account>.duck or <label>.<account>.duck.".into());
    }
    let canonical = host.to_ascii_lowercase();
    let labels = canonical.split('.').collect::<Vec<_>>();
    if labels.last().is_none_or(|label| *label != "duck")
        || !(labels.len() == 2 || labels.len() == 3)
        || labels.iter().any(|label| {
            label.is_empty()
                || label.len() > 63
                || label.starts_with('-')
                || label.ends_with('-')
                || !label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
    {
        return Err("Enter <account>.duck or <label>.<account>.duck.".into());
    }
    if canonical == "net.duck" {
        return Ok(());
    }
    let handle = labels[labels.len() - 2];
    duckdns::validate_handle(handle)
        .map_err(|_| format!("{handle}.duck is reserved or is not a valid account."))
}
