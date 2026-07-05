//! the capability spec: a TOML file describing one executor capability.
//!
//! everything capability-host used to hardcode in Rust — which binary to
//! probe, the argv to invoke it, how to parse its output, which model refs
//! route to it — is data in a spec file. the two built-in executors (codex,
//! claude) are embedded specs parsed by this exact module, so an operator
//! adding a THIRD executor writes a TOML file, not Rust:
//!
//! ```toml
//! spec = 1
//! [capability]
//! tag = "ollama"
//! description = "local ollama daemon via its cli"
//! [detect]
//! bin = "ollama"
//! [invoke]
//! args = ["run", "{model}"]
//! prompt = "stdin"
//! [output]
//! format = "text"
//! [models]
//! patterns = ["llama*", "qwen*"]
//! default = "llama4"
//! ```
//!
//! ## trust model — read this before adding spec sources
//!
//! a spec names an arbitrary local binary and the argv to run it with: loading
//! a spec is EXECUTING CODE by proxy. specs are operator-trusted configuration
//! — the same trust class as a shell profile or systemd unit. they load from
//! exactly two places, both local and operator-controlled: the specs embedded
//! in this crate at compile time, and `$DUCKTAPE_CAPABILITY_DIR` (default
//! `~/.ducktape/capabilities`). specs are NEVER fetched from the network, and
//! nothing consensus-side may ever read one (host-local files are
//! non-deterministic input; the consensus capability module sees only the
//! announced TAGS, never the specs behind them).
//!
//! ## routing
//!
//! `[models].patterns` are `*`-glob patterns over model refs. one model ref is
//! routed to ONE spec, deterministically:
//!
//! 1. every loaded spec's patterns are tried; the spec with the matching
//!    pattern carrying the MOST literal (non-`*`) characters wins — a more
//!    specific pattern beats a more generic one, so `claude*` (6 literals)
//!    beats the `*` catch-all (0) for `claude-sonnet-5`;
//! 2. ties break to the lexicographically smaller tag, so routing never
//!    depends on file order or discovery order.
//!
//! an UNPINNED request (empty model ref) routes like the empty string, which
//! only a `*` catch-all matches — the catch-all spec's `[models].default`
//! then supplies the model. routing is over ALL loaded specs, including ones
//! whose binary was not found on this host: `claude-x` on a codex-only node
//! errors "capability 'claude' is not provided", not "unknown model".
//!
//! ## override precedence
//!
//! embedded specs load first; operator specs load second and REPLACE an
//! embedded spec with the same tag wholesale (no field merging — a spec is
//! the unit of override). duplicate tags WITHIN the operator dir are a hard
//! error: two files claiming one tag is operator confusion, not a precedence
//! question.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

/// the one spec format version this build understands. parsing rejects any
/// other value loudly — an operator on a newer format gets "unsupported spec
/// version", never silently misread fields.
const SPEC_VERSION: u64 = 1;

/// consensus-mirrored tag shape: the capability module rejects tags longer
/// than this, so validating here means an announce built from a loaded spec
/// can never bounce off consensus.
const MAX_TAG_LEN: usize = 64;

/// bounds for `[invoke].timeout_secs` — a zero timeout would kill every job
/// at spawn; anything over an hour is a hang, not a job.
const TIMEOUT_RANGE: std::ops::RangeInclusive<u64> = 1..=3600;

/// a validated capability spec — the parsed, checked form of one TOML file.
/// construction goes through [`CapabilitySpec::parse`] only, so holding one
/// is proof the invariants documented on each field hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilitySpec {
    /// the registry tag announced network-wide. `[a-z0-9._-]`, non-empty,
    /// ≤ 64 bytes — exactly the consensus capability module's rules.
    pub tag: String,
    /// human-facing one-liner for docs and status surfaces.
    pub description: String,
    /// the binary name probed on `PATH`.
    pub bin: String,
    /// optional env var naming an explicit binary path. an override that
    /// points at nothing is a loud warning + absent capability, never a
    /// silent fallback to the `PATH` probe.
    pub env: Option<String>,
    /// argv template (after the binary). every element is passed verbatim
    /// except the `{model}` placeholder, which is replaced with the job's
    /// resolved model ref. args are NEVER shell-interpreted — no quoting, no
    /// expansion, no injection surface.
    pub args: Vec<String>,
    /// how the prompt reaches the child. v1 supports stdin only: the prompt
    /// is fed concurrently with output collection, then EOF.
    pub prompt: PromptMode,
    /// per-job wall-clock budget; the child is killed at the deadline.
    /// `DUCKTAPE_PROVIDER_TIMEOUT_SECS` overrides ALL specs at once.
    pub timeout_secs: u64,
    /// which named stdout parser extracts the assistant's final text.
    pub output: OutputFormat,
    /// `*`-glob patterns over model refs this capability serves. at least one.
    pub patterns: Vec<String>,
    /// the model used when an unpinned request routes here. optional: a spec
    /// without one refuses unpinned requests with a clear error.
    pub default_model: Option<String>,
}

/// how the prompt is delivered to the child process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptMode {
    /// written to the child's stdin, then EOF. the only v1 mode: an argv
    /// placeholder would leak prompts into `ps` output and hit ARG_MAX.
    Stdin,
}

/// the named stdout parsers. a CLOSED set on purpose: each name is a tested
/// parser for a real CLI's output contract, not a config-described guess.
/// adding a name is a code change with tests — that is the point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// `codex exec --json` JSONL event stream; the LAST agent message wins.
    CodexJsonl,
    /// `claude -p --output-format json`: one `{"type":"result",...}` object.
    ClaudeJson,
    /// raw stdout, trimmed. the generic escape hatch: any CLI that prints
    /// the answer plainly is wireable with this and zero code.
    Text,
}

// ---- raw (serde) shapes ------------------------------------------------------
// the on-disk TOML shape, deliberately separate from CapabilitySpec: serde
// gets a dumb mirror of the file, validation turns it into the checked type.

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSpec {
    spec: u64,
    capability: RawCapability,
    detect: RawDetect,
    invoke: RawInvoke,
    output: RawOutput,
    models: RawModels,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCapability {
    tag: String,
    #[serde(default)]
    description: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDetect {
    bin: String,
    #[serde(default)]
    env: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawInvoke {
    #[serde(default)]
    args: Vec<String>,
    prompt: String,
    #[serde(default = "default_timeout")]
    timeout_secs: u64,
}

fn default_timeout() -> u64 {
    300
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawOutput {
    format: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawModels {
    patterns: Vec<String>,
    #[serde(default)]
    default: Option<String>,
}

impl CapabilitySpec {
    /// parse and validate one spec's TOML. `origin` names the source (a file
    /// path or "embedded:<tag>") so every error says WHICH spec is broken.
    /// unknown fields are rejected (`deny_unknown_fields`): a typo like
    /// `patern` fails loud instead of silently changing routing.
    pub fn parse(toml_text: &str, origin: &str) -> Result<Self, String> {
        let raw: RawSpec =
            toml::from_str(toml_text).map_err(|e| format!("{origin}: not a valid spec: {e}"))?;
        if raw.spec != SPEC_VERSION {
            return Err(format!(
                "{origin}: unsupported spec version {} (this build understands {SPEC_VERSION})",
                raw.spec
            ));
        }
        let tag = raw.capability.tag;
        if tag.is_empty() || tag.len() > MAX_TAG_LEN {
            return Err(format!(
                "{origin}: tag must be 1..={MAX_TAG_LEN} bytes, got {}",
                tag.len()
            ));
        }
        // mirror the consensus module exactly: a tag that validates here must
        // never bounce off the capability registry's Announce validation.
        if !tag
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b))
        {
            return Err(format!(
                "{origin}: tag has invalid characters (want [a-z0-9._-]): {tag:?}"
            ));
        }
        if raw.detect.bin.is_empty() {
            return Err(format!("{origin}: detect.bin must be non-empty"));
        }
        let prompt = match raw.invoke.prompt.as_str() {
            "stdin" => PromptMode::Stdin,
            other => {
                return Err(format!(
                    "{origin}: invoke.prompt {other:?} is not supported (v1 supports \"stdin\")"
                ));
            }
        };
        if !TIMEOUT_RANGE.contains(&raw.invoke.timeout_secs) {
            return Err(format!(
                "{origin}: invoke.timeout_secs must be within {TIMEOUT_RANGE:?}, got {}",
                raw.invoke.timeout_secs
            ));
        }
        let output = match raw.output.format.as_str() {
            "codex-jsonl" => OutputFormat::CodexJsonl,
            "claude-json" => OutputFormat::ClaudeJson,
            "text" => OutputFormat::Text,
            other => {
                return Err(format!(
                    "{origin}: output.format {other:?} is not a known parser \
                     (want codex-jsonl | claude-json | text)"
                ));
            }
        };
        if raw.models.patterns.is_empty() {
            return Err(format!(
                "{origin}: models.patterns must name at least one pattern"
            ));
        }
        if let Some(p) = raw.models.patterns.iter().find(|p| p.is_empty()) {
            let _ = p;
            return Err(format!("{origin}: models.patterns entries must be non-empty"));
        }
        if raw.models.default.as_deref() == Some("") {
            return Err(format!("{origin}: models.default must be non-empty when set"));
        }
        Ok(Self {
            tag,
            description: raw.capability.description,
            bin: raw.detect.bin,
            env: raw.detect.env,
            args: raw.invoke.args,
            prompt,
            timeout_secs: raw.invoke.timeout_secs,
            output,
            patterns: raw.models.patterns,
            default_model: raw.models.default,
        })
    }
}

// ---- the loaded spec set -----------------------------------------------------

/// every spec this host loaded — embedded built-ins plus the operator dir —
/// keyed by tag, override already applied. routing happens HERE (over all
/// specs), provider lookup happens in the ProviderSet (over the discovered
/// subset): a model ref for an installed-nowhere capability still routes, so
/// the error can name the missing capability instead of shrugging.
#[derive(Debug, Clone)]
pub struct SpecSet {
    specs: BTreeMap<String, CapabilitySpec>,
}

/// the built-in specs compiled into this binary. parsed at runtime through
/// the same [`CapabilitySpec::parse`] path as operator files; a unit test
/// asserts validity so a broken embedded spec fails CI, not a node boot —
/// the `expect` here is unreachable past that test.
pub fn builtin_specs() -> Vec<CapabilitySpec> {
    [
        ("embedded:codex", include_str!("../specs/codex.toml")),
        ("embedded:claude", include_str!("../specs/claude.toml")),
    ]
    .into_iter()
    .map(|(origin, text)| {
        CapabilitySpec::parse(text, origin).expect("embedded specs are CI-validated")
    })
    .collect()
}

impl SpecSet {
    /// compose the host's spec set: built-ins, then the operator dir (if it
    /// exists), same-tag operator specs replacing built-ins wholesale.
    pub fn load(operator_dir: Option<&Path>) -> Result<Self, String> {
        let mut specs: BTreeMap<String, CapabilitySpec> = builtin_specs()
            .into_iter()
            .map(|s| (s.tag.clone(), s))
            .collect();
        if let Some(dir) = operator_dir {
            for spec in load_dir(dir)? {
                // operator overrides embedded silently by design — replacing a
                // built-in is the documented way to retune codex/claude flags.
                specs.insert(spec.tag.clone(), spec);
            }
        }
        Ok(Self { specs })
    }

    /// a set from explicit specs — the test seam, no filesystem involved.
    pub fn from_specs(list: Vec<CapabilitySpec>) -> Self {
        Self {
            specs: list.into_iter().map(|s| (s.tag.clone(), s)).collect(),
        }
    }

    pub fn get(&self, tag: &str) -> Option<&CapabilitySpec> {
        self.specs.get(tag)
    }

    pub fn iter(&self) -> impl Iterator<Item = &CapabilitySpec> {
        self.specs.values()
    }

    /// route a model ref to a spec — the ONE place model naming meets
    /// capability selection (see the module docs for the precedence rule).
    /// an unpinned request routes as the empty string, matched only by a
    /// `*` catch-all. `None` means no loaded spec claims this model ref.
    pub fn route(&self, model_ref: &str) -> Option<&CapabilitySpec> {
        let model = model_ref.trim();
        let mut best: Option<(usize, &CapabilitySpec)> = None;
        // BTreeMap iteration is tag-ascending, so a STRICTLY-greater update
        // keeps the lexicographically smallest tag among equal scores.
        for spec in self.specs.values() {
            let score = spec
                .patterns
                .iter()
                .filter(|p| glob_match(p, model))
                .map(|p| literal_len(p))
                .max();
            if let Some(score) = score
                && best.is_none_or(|(b, _)| score > b)
            {
                best = Some((score, spec));
            }
        }
        best.map(|(_, spec)| spec)
    }
}

/// load and validate every `*.toml` in `dir`, sorted by file name for stable
/// error ordering. duplicate tags in one dir are a hard error.
fn load_dir(dir: &Path) -> Result<Vec<CapabilitySpec>, String> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| format!("capability spec dir {}: {e}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "toml"))
        .collect();
    entries.sort();
    let mut seen: BTreeMap<String, std::path::PathBuf> = BTreeMap::new();
    let mut specs = Vec::new();
    for path in entries {
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("capability spec {}: {e}", path.display()))?;
        let spec = CapabilitySpec::parse(&text, &path.display().to_string())?;
        if let Some(prev) = seen.insert(spec.tag.clone(), path.clone()) {
            return Err(format!(
                "duplicate capability tag {:?}: {} and {}",
                spec.tag,
                prev.display(),
                path.display()
            ));
        }
        specs.push(spec);
    }
    Ok(specs)
}

/// `*`-only glob: `*` matches any run of characters (including empty), all
/// other characters match themselves. no `?`, no classes — model refs are
/// simple names and the restraint keeps precedence (literal count) obvious.
pub(crate) fn glob_match(pattern: &str, text: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == text;
    }
    let mut rest = text;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if i == 0 {
            // anchored head: the pattern does not start with '*'.
            let Some(after) = rest.strip_prefix(part) else {
                return false;
            };
            rest = after;
        } else if i == parts.len() - 1 {
            // anchored tail: the pattern does not end with '*'.
            let Some(before) = rest.strip_suffix(part) else {
                return false;
            };
            rest = before;
        } else {
            // greedy-enough middle: take the first occurrence left to right.
            let Some(at) = rest.find(part) else {
                return false;
            };
            rest = &rest[at + part.len()..];
        }
    }
    true
}

/// the routing specificity score: literal (non-`*`) characters in a pattern.
fn literal_len(pattern: &str) -> usize {
    pattern.chars().filter(|c| *c != '*').count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec_toml(tag: &str, patterns: &str) -> String {
        format!(
            r#"
spec = 1
[capability]
tag = "{tag}"
[detect]
bin = "{tag}-cli"
[invoke]
args = ["run", "{{model}}"]
prompt = "stdin"
[output]
format = "text"
[models]
patterns = {patterns}
"#
        )
    }

    #[test]
    fn embedded_specs_are_valid_and_cover_codex_and_claude() {
        // the CI gate for the expect() in builtin_specs(): a broken embedded
        // spec fails HERE, never at node boot.
        let specs = builtin_specs();
        let tags: Vec<&str> = specs.iter().map(|s| s.tag.as_str()).collect();
        assert_eq!(tags, vec!["codex", "claude"]);
        assert_eq!(specs[0].output, OutputFormat::CodexJsonl);
        assert_eq!(specs[1].output, OutputFormat::ClaudeJson);
        assert!(specs[0].default_model.is_some(), "codex carries the unpinned default");
    }

    #[test]
    fn routing_prefers_literal_specificity_over_catchall() {
        let set = SpecSet::load(None).unwrap();
        assert_eq!(set.route("claude-sonnet-5").unwrap().tag, "claude");
        assert_eq!(set.route("gpt-5.3-codex-spark").unwrap().tag, "codex");
        assert_eq!(set.route("some-unknown-model").unwrap().tag, "codex", "catch-all");
        assert_eq!(set.route("").unwrap().tag, "codex", "unpinned routes to catch-all");
        assert_eq!(set.route("  ").unwrap().tag, "codex", "whitespace = unpinned");
    }

    #[test]
    fn routing_ties_break_to_the_smaller_tag() {
        let a = CapabilitySpec::parse(&spec_toml("bbb", r#"["m-*"]"#), "t").unwrap();
        let b = CapabilitySpec::parse(&spec_toml("aaa", r#"["m-*"]"#), "t").unwrap();
        let set = SpecSet::from_specs(vec![a, b]);
        assert_eq!(set.route("m-1").unwrap().tag, "aaa", "deterministic tie-break");
    }

    #[test]
    fn routing_none_when_no_pattern_matches() {
        let only = CapabilitySpec::parse(&spec_toml("x", r#"["x-*"]"#), "t").unwrap();
        let set = SpecSet::from_specs(vec![only]);
        assert!(set.route("y-model").is_none());
        assert!(set.route("").is_none(), "no catch-all, no unpinned route");
    }

    #[test]
    fn glob_semantics() {
        assert!(glob_match("*", ""));
        assert!(glob_match("*", "anything"));
        assert!(glob_match("claude*", "claude-sonnet-5"));
        assert!(!glob_match("claude*", "xclaude"));
        assert!(glob_match("*codex*", "gpt-5.3-codex-spark"));
        assert!(glob_match("gpt-*", "gpt-5.5"));
        assert!(!glob_match("gpt-*x", "gpt-5.5"));
        assert!(glob_match("a*b*c", "aXXbYYc"));
        assert!(!glob_match("exact", "exactx"));
        assert!(glob_match("exact", "exact"));
    }

    #[test]
    fn version_tag_prompt_format_timeout_and_patterns_validate() {
        let base = spec_toml("ok", r#"["*"]"#);
        assert!(CapabilitySpec::parse(&base, "t").is_ok());

        for (needle, replacement, expect) in [
            ("spec = 1", "spec = 2", "unsupported spec version"),
            (r#"tag = "ok""#, r#"tag = "OK""#, "invalid characters"),
            (r#"tag = "ok""#, r#"tag = """#, "1..=64 bytes"),
            (r#"prompt = "stdin""#, r#"prompt = "argv""#, "not supported"),
            (r#"format = "text""#, r#"format = "yaml""#, "not a known parser"),
            (r#"patterns = ["*"]"#, r#"patterns = []"#, "at least one pattern"),
            (r#"patterns = ["*"]"#, r#"patterns = [""]"#, "non-empty"),
        ] {
            let broken = base.replace(needle, replacement);
            let err = CapabilitySpec::parse(&broken, "t").unwrap_err();
            assert!(err.contains(expect), "wanted {expect:?} in {err:?}");
        }

        let long_tag = spec_toml(&"x".repeat(65), r#"["*"]"#);
        assert!(CapabilitySpec::parse(&long_tag, "t").unwrap_err().contains("1..=64"));

        let bad_timeout = base.replace(r#"prompt = "stdin""#, "prompt = \"stdin\"\ntimeout_secs = 0");
        assert!(CapabilitySpec::parse(&bad_timeout, "t").unwrap_err().contains("timeout_secs"));
    }

    #[test]
    fn unknown_fields_fail_loud() {
        // a typo must never silently change routing.
        let typo = spec_toml("ok", r#"["*"]"#).replace("[models]", "[models]\npatern = 1");
        // (inserted BEFORE patterns so it lands in the models table)
        let err = CapabilitySpec::parse(&typo, "t").unwrap_err();
        assert!(err.contains("not a valid spec"), "got {err:?}");
    }

    #[test]
    fn operator_dir_overrides_embedded_and_rejects_duplicates() {
        let dir = std::env::temp_dir().join(format!("capspec-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // override the embedded codex spec wholesale.
        std::fs::write(dir.join("my-codex.toml"), spec_toml("codex", r#"["*"]"#)).unwrap();
        let set = SpecSet::load(Some(&dir)).unwrap();
        assert_eq!(set.get("codex").unwrap().bin, "codex-cli", "operator spec won");
        assert!(set.get("claude").is_some(), "untouched built-in remains");

        // two operator files claiming one tag: hard error naming both files.
        std::fs::write(dir.join("zz-codex.toml"), spec_toml("codex", r#"["*"]"#)).unwrap();
        let err = SpecSet::load(Some(&dir)).unwrap_err();
        assert!(err.contains("duplicate capability tag"), "got {err:?}");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
