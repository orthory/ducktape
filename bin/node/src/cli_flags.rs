//! Shared argument parsing for synchronous CLI command families.

/// flags that take NO value — their bare presence is the whole signal. every
/// other `--name` consumes the next token as its value. `--json` is the opt-in
/// machine-readable output selector on the read verbs; keeping it here (rather
/// than a per-verb list) means the one parser recognizes it uniformly.
const BOOL_FLAGS: &[&str] = &["json"];

/// Parse `--name value` pairs plus positionals. `-n` is the one short alias
/// for `--network`, the workspace selector accepted by every command. A flag
/// named in [`BOOL_FLAGS`] is valueless (recorded as an empty string) and never
/// consumes the following token, so `--json <path>` leaves `<path>` positional.
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
        let Some(name) = name else {
            positional.push(a.clone());
            continue;
        };
        let is_bool_flag = BOOL_FLAGS.contains(&name);
        if is_bool_flag {
            flags.insert(name.to_string(), String::new());
            continue;
        }
        let value = it.next().ok_or_else(|| format!("--{name} needs a value"))?;
        flags.insert(name.to_string(), value.clone());
    }
    Ok((positional, flags))
}

#[cfg(test)]
mod tests {
    use super::parse_flags;

    fn args(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn value_flag_still_consumes_next_token() {
        let (pos, flags) = parse_flags(&args(&["--config", "node.toml"])).unwrap();
        assert!(pos.is_empty());
        assert_eq!(flags.get("config").map(String::as_str), Some("node.toml"));
    }

    #[test]
    fn short_network_alias_maps_to_network() {
        let (_, flags) = parse_flags(&args(&["-n", "ducktape#abcd1234"])).unwrap();
        assert_eq!(
            flags.get("network").map(String::as_str),
            Some("ducktape#abcd1234")
        );
    }

    #[test]
    fn bool_flag_as_last_arg_needs_no_value() {
        let (pos, flags) = parse_flags(&args(&["--config", "n.toml", "--json"])).unwrap();
        assert!(pos.is_empty());
        assert!(flags.contains_key("json"));
        assert_eq!(flags.get("config").map(String::as_str), Some("n.toml"));
    }

    #[test]
    fn bool_flag_before_positional_does_not_eat_it() {
        // `--json <positional>` must leave the positional alone.
        let (pos, flags) = parse_flags(&args(&["--json", "deadbeef"])).unwrap();
        assert!(flags.contains_key("json"));
        assert_eq!(pos, vec!["deadbeef".to_string()]);
    }

    #[test]
    fn bool_flag_before_value_flag() {
        let (pos, flags) = parse_flags(&args(&["--json", "--config", "n.toml"])).unwrap();
        assert!(pos.is_empty());
        assert!(flags.contains_key("json"));
        assert_eq!(flags.get("config").map(String::as_str), Some("n.toml"));
    }

    #[test]
    fn missing_value_still_errors_for_value_flags() {
        assert!(parse_flags(&args(&["--config"])).is_err());
    }
}
