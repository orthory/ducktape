//! Shared argument parsing for synchronous CLI command families.

/// Parse `--name value` pairs plus positionals. `-n` is the one short alias
/// for `--network`, the workspace selector accepted by every command.
pub(super) fn parse_flags(
    args: &[String],
) -> Result<(Vec<String>, std::collections::BTreeMap<String, String>), String> {
    let mut positional = Vec::new();
    let mut flags = std::collections::BTreeMap::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let name = match a.as_str() {
            "-n" => Some("network"),
            other => other.strip_prefix("--"),
        };
        if let Some(name) = name {
            let value = it.next().ok_or_else(|| format!("--{name} needs a value"))?;
            flags.insert(name.to_string(), value.clone());
        } else {
            positional.push(a.clone());
        }
    }
    Ok((positional, flags))
}
