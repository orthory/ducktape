//! the capability spec: a TOML file describing one executor capability.
//!
//! everything capability-host used to hardcode in Rust — which binary to
//! probe, the argv to invoke it, how to parse its output — is data in a spec
//! file. the built-in executors are embedded spec FILES (globbed by build.rs
//! — no Rust source names an executor) parsed by this exact module, so an
//! operator adding another executor writes a TOML file, not Rust:
//!
//! ```toml
//! spec = 1
//! [capability]
//! tag = "ollama"
//! description = "local ollama daemon via its cli"
//! [detect]
//! bin = "ollama"
//! [invoke]
//! args = ["run", "llama4"]
//! prompt = "stdin"
//! [output]
//! format = "text"
//! ```
//!
//! dispatch is by EXPLICIT capability tag — nothing is ever inferred from a
//! model name. a job names the tag it needs ("ollama"); the spec's argv says,
//! literally and completely, what running that tag means on this host (which
//! binary, which flags, which model — all operator policy). a finer-grained
//! need ("this executor, but with these exact flags") is a finer tag with its own
//! spec, not a routing rule — `[[variants]]` (see [`crate::variants`]) is
//! load-time sugar for writing a family of such finer tags in one file, each
//! still one tag with one fixed argv. no executor name appears anywhere in
//! this crate's code or tests — executors exist only as spec data.
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
//! ## override precedence
//!
//! embedded specs load first; operator specs load second and REPLACE an
//! embedded spec with the same tag wholesale (no field merging — a spec is
//! the unit of override). duplicate tags WITHIN the operator dir are a hard
//! error: two files claiming one tag is operator confusion, not a precedence
//! question.

use std::collections::BTreeMap;
use std::path::Path;

use capability::validate_tag;
use serde::Deserialize;

use crate::session::{self, SessionSpec};
use crate::variants::{self, RawVariant};
use crate::workspace::{self, WorkspaceMode};

/// the one spec format version this build understands. parsing rejects any
/// other value loudly — an operator on a newer format gets "unsupported spec
/// version", never silently misread fields.
const SPEC_VERSION: u64 = 1;

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
    /// argv (after the binary), passed verbatim to exec — fully literal, no
    /// placeholders, NEVER shell-interpreted: no quoting, no expansion, no
    /// injection surface. which model (if any) the executor runs is encoded
    /// here as ordinary flags — operator policy, invisible to consensus.
    pub args: Vec<String>,
    /// how the prompt reaches the child. v1 supports stdin only: the prompt
    /// is fed concurrently with output collection, then EOF.
    pub prompt: PromptMode,
    /// per-job wall-clock budget; the child is killed at the deadline.
    /// `DUCKTAPE_PROVIDER_TIMEOUT_SECS` overrides ALL specs at once.
    pub timeout_secs: u64,
    /// which named stdout parser extracts the assistant's final text.
    pub output: OutputFormat,
    /// the child's working-directory policy — scratch unless the spec's
    /// `[workspace]` opts into per-agent persistence (see [`crate::workspace`]).
    pub workspace: WorkspaceMode,
    /// optional `[session]` thread-continuity plumbing — host-local capture
    /// and resume of the executor's own session id (see [`crate::session`]).
    pub session: Option<SessionSpec>,
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
    /// a JSONL event stream; the LAST `agent_message` item wins.
    JsonlEvents,
    /// a single `{"type":"result",...}` object (the contract of
    /// `--output-format json`-style print modes).
    JsonResult,
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
    /// optional `[workspace]` — validated in [`crate::workspace`].
    #[serde(default)]
    workspace: Option<workspace::RawWorkspace>,
    /// optional `[session]` — validated in [`crate::session`].
    #[serde(default)]
    session: Option<session::RawSession>,
    /// optional `[[variants]]` — finer tags expanded at load time, validated
    /// in [`crate::variants`].
    #[serde(default)]
    variants: Vec<RawVariant>,
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

impl CapabilitySpec {
    /// parse and validate one SINGLE-tag spec's TOML — the convenience for
    /// callers holding a spec that declares no `[[variants]]` (a file that
    /// does is a hard error here, never a silent drop of its variants; load
    /// whole files through [`CapabilitySpec::parse_all`]).
    pub fn parse(toml_text: &str, origin: &str) -> Result<Self, String> {
        let (base, variants) = Self::parse_raw(toml_text, origin)?;
        if !variants.is_empty() {
            return Err(format!(
                "{origin}: spec declares [[variants]] and expands to multiple \
                 tags; load it via parse_all"
            ));
        }
        Ok(base)
    }

    /// parse and validate one spec FILE's TOML into every spec it defines:
    /// the base spec first, then its `[[variants]]` expansions in declaration
    /// order. this is the loaders' entry point — one file, 1+ tags.
    pub fn parse_all(toml_text: &str, origin: &str) -> Result<Vec<Self>, String> {
        let (base, raw_variants) = Self::parse_raw(toml_text, origin)?;
        let mut specs = variants::expand(&base, &raw_variants, origin)?;
        specs.insert(0, base);
        Ok(specs)
    }

    /// the shared parse core: the validated base spec plus its still-raw
    /// variant entries. `origin` names the source (a file path or
    /// "embedded:<file>") so every error says WHICH spec is broken. unknown
    /// fields are rejected (`deny_unknown_fields`): a typo like `patern`
    /// fails loud instead of silently changing routing.
    fn parse_raw(toml_text: &str, origin: &str) -> Result<(Self, Vec<RawVariant>), String> {
        let raw: RawSpec =
            toml::from_str(toml_text).map_err(|e| format!("{origin}: not a valid spec: {e}"))?;
        if raw.spec != SPEC_VERSION {
            return Err(format!(
                "{origin}: unsupported spec version {} (this build understands {SPEC_VERSION})",
                raw.spec
            ));
        }
        let tag = raw.capability.tag;
        // THE consensus tag rule (shared, not mirrored): a tag that validates
        // here can never bounce off the capability registry's Announce.
        validate_tag(&tag).map_err(|e| format!("{origin}: {e}"))?;
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
            "jsonl-events" => OutputFormat::JsonlEvents,
            "json-result" => OutputFormat::JsonResult,
            "text" => OutputFormat::Text,
            other => {
                return Err(format!(
                    "{origin}: output.format {other:?} is not a known parser \
                     (want jsonl-events | json-result | text)"
                ));
            }
        };
        // scratch is expressed by OMITTING [workspace] — no redundant
        // "scratch" spelling exists to drift from the default.
        let workspace = raw
            .workspace
            .map(|w| workspace::parse_workspace(&w, origin))
            .transpose()?
            .unwrap_or_default();
        let session = raw
            .session
            .map(|s| session::parse_session(&s, origin))
            .transpose()?;
        Ok((
            Self {
                tag,
                description: raw.capability.description,
                bin: raw.detect.bin,
                env: raw.detect.env,
                args: raw.invoke.args,
                prompt,
                timeout_secs: raw.invoke.timeout_secs,
                output,
                workspace,
                session,
            },
            raw.variants,
        ))
    }
}

// ---- the loaded spec set -----------------------------------------------------

/// every spec this host loaded — embedded built-ins plus the operator dir —
/// keyed by tag, override already applied. the LOADED set is wider than the
/// DISCOVERED one (a spec whose binary is absent still loads), so a request
/// for an uninstalled capability errors by name instead of shrugging.
#[derive(Debug, Clone)]
pub struct SpecSet {
    specs: BTreeMap<String, CapabilitySpec>,
}

/// the built-in specs compiled into this binary — every `specs/*.toml`,
/// globbed and embedded by build.rs, so no Rust source names an executor.
/// parsed at runtime through the same [`CapabilitySpec::parse_all`] path as
/// operator files (one file may expand into a base tag plus `[[variants]]`);
/// a unit test asserts validity so a broken embedded spec fails CI, not a
/// node boot — the `expect` here is unreachable past that test.
pub fn builtin_specs() -> Vec<CapabilitySpec> {
    include!(concat!(env!("OUT_DIR"), "/builtin_specs.rs"))
        .into_iter()
        .flat_map(|(origin, text): (&str, &str)| {
            CapabilitySpec::parse_all(text, origin).expect("embedded specs are CI-validated")
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
                // built-in is the documented way to retune its flags.
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
}

/// load and validate every `*.toml` in `dir`, sorted by file name for stable
/// error ordering. duplicate tags in one dir are a hard error — a file's
/// `[[variants]]` expansions count like any other tag, so a variant of one
/// file colliding with another file's tag is caught the same way.
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
        for spec in CapabilitySpec::parse_all(&text, &path.display().to_string())? {
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
    }
    Ok(specs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec_toml(tag: &str) -> String {
        format!(
            r#"
spec = 1
[capability]
tag = "{tag}"
[detect]
bin = "{tag}-cli"
[invoke]
args = ["run"]
prompt = "stdin"
[output]
format = "text"
"#
        )
    }

    #[test]
    fn embedded_specs_parse_with_valid_unique_tags() {
        // the CI gate for the expect() in builtin_specs(): a broken embedded
        // spec fails HERE, never at node boot. deliberately tag-agnostic —
        // WHICH executors ship as built-ins is data, and no test hardcodes
        // executor names.
        let specs = builtin_specs();
        assert!(!specs.is_empty(), "the crate ships built-in specs");
        let tags: std::collections::BTreeSet<&str> =
            specs.iter().map(|s| s.tag.as_str()).collect();
        assert_eq!(tags.len(), specs.len(), "embedded tags are unique");
    }

    #[test]
    fn version_tag_prompt_format_and_timeout_validate() {
        let base = spec_toml("ok");
        assert!(CapabilitySpec::parse(&base, "t").is_ok());

        for (needle, replacement, expect) in [
            ("spec = 1", "spec = 2", "unsupported spec version"),
            (r#"tag = "ok""#, r#"tag = "OK""#, "invalid characters"),
            (r#"tag = "ok""#, r#"tag = """#, "non-empty"),
            (r#"prompt = "stdin""#, r#"prompt = "argv""#, "not supported"),
            (r#"format = "text""#, r#"format = "yaml""#, "not a known parser"),
        ] {
            let broken = base.replace(needle, replacement);
            let err = CapabilitySpec::parse(&broken, "t").unwrap_err();
            assert!(err.contains(expect), "wanted {expect:?} in {err:?}");
        }

        let long_tag = spec_toml(&"x".repeat(65));
        assert!(CapabilitySpec::parse(&long_tag, "t").unwrap_err().contains("64 bytes"));

        let bad_timeout = base.replace(r#"prompt = "stdin""#, "prompt = \"stdin\"\ntimeout_secs = 0");
        assert!(CapabilitySpec::parse(&bad_timeout, "t").unwrap_err().contains("timeout_secs"));
    }

    #[test]
    fn unknown_fields_fail_loud() {
        // a typo — or a field from the retired [models] routing era — must
        // never be silently ignored: the operator wrote config that does
        // nothing, and the boot error is what tells them.
        let typo = spec_toml("ok").replace("[invoke]", "[invoke]\ntimeout_sec = 1");
        let err = CapabilitySpec::parse(&typo, "t").unwrap_err();
        assert!(err.contains("not a valid spec"), "got {err:?}");

        let stale = format!("{}\n[models]\npatterns = [\"*\"]\n", spec_toml("ok"));
        let err = CapabilitySpec::parse(&stale, "t").unwrap_err();
        assert!(err.contains("not a valid spec"), "got {err:?}");
    }

    #[test]
    fn workspace_and_session_sections_parse_and_default_off() {
        // absent sections keep the v1 posture: scratch dir, no sessions.
        let plain = CapabilitySpec::parse(&spec_toml("ok"), "t").unwrap();
        assert_eq!(plain.workspace, crate::WorkspaceMode::Scratch);
        assert_eq!(plain.session, None);

        let full = format!(
            r#"{}
[workspace]
mode = "persistent"
[session]
capture = "json-result-field:session_id"
resume_args_append = ["--resume", "{{session_id}}"]
"#,
            spec_toml("ok")
        );
        let spec = CapabilitySpec::parse(&full, "t").unwrap();
        assert_eq!(spec.workspace, crate::WorkspaceMode::Persistent);
        let session = spec.session.expect("session parsed");
        assert_eq!(
            session.capture,
            crate::SessionCapture::JsonResultField("session_id".into())
        );
        assert_eq!(
            session.resume,
            crate::ResumeArgv::Append(vec!["--resume".into(), "{session_id}".into()])
        );
    }

    #[test]
    fn workspace_and_session_sections_fail_loud_on_unknown_or_bad_fields() {
        let base = spec_toml("ok");
        for (extra, expect) in [
            // an unknown workspace mode and a typo'd field are both loud.
            ("[workspace]\nmode = \"shared\"\n", "not supported"),
            ("[workspace]\nmodes = \"persistent\"\n", "not a valid spec"),
            (
                "[workspace]\nmode = \"persistent\"\nroot = \"/x\"\n",
                "not a valid spec",
            ),
            // session: unknown capture, unknown field, no/both resume styles,
            // and a slot-less resume argv.
            (
                "[session]\ncapture = \"csv\"\nresume_args = [\"{session_id}\"]\n",
                "not a known mode",
            ),
            (
                "[session]\ncapture = \"jsonl-events\"\nresume = [\"x\"]\n",
                "not a valid spec",
            ),
            ("[session]\ncapture = \"jsonl-events\"\n", "exactly one"),
            (
                "[session]\ncapture = \"jsonl-events\"\nresume_args = [\"{session_id}\"]\nresume_args_append = [\"x\"]\n",
                "exactly one",
            ),
            (
                "[session]\ncapture = \"jsonl-events\"\nresume_args = [\"resume\"]\n",
                "{session_id}",
            ),
        ] {
            let toml = format!("{base}\n{extra}");
            let err = CapabilitySpec::parse(&toml, "t").unwrap_err();
            assert!(err.contains(expect), "wanted {expect:?} in {err:?}");
        }
    }

    #[test]
    fn single_spec_parse_refuses_a_variants_file() {
        // parse() is the single-tag convenience; silently dropping a file's
        // variants would be exactly the quiet misread this loader forbids.
        let toml = format!(
            "{}\n[[variants]]\nsuffix = \"m_low\"\nargs = [\"a\"]\n",
            spec_toml("ok")
        );
        let err = CapabilitySpec::parse(&toml, "t").unwrap_err();
        assert!(err.contains("parse_all"), "got {err:?}");
    }

    #[test]
    fn operator_files_expand_variants_and_override_stays_wholesale_by_tag() {
        let dir =
            std::env::temp_dir().join(format!("capspec-variants-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // an operator family file expands exactly like an embedded one.
        let family = format!(
            "{}\n[[variants]]\nsuffix = \"m1_low\"\nargs = [\"run\", \"--fast\"]\n",
            spec_toml("fam")
        );
        std::fs::write(dir.join("family.toml"), family).unwrap();

        // overriding a base tag replaces THAT tag only: a variant tag is its
        // own tag, so the embedded siblings stay. parent derived from the
        // data, never hardcoded.
        let builtins = builtin_specs();
        let variant = builtins
            .iter()
            .find(|s| s.tag.contains('_'))
            .expect("built-ins ship variant tags");
        let parent = variant.tag.split('_').next().unwrap().to_string();
        std::fs::write(dir.join("override.toml"), spec_toml(&parent)).unwrap();

        let set = SpecSet::load(Some(&dir)).unwrap();
        assert_eq!(
            set.get("fam_m1_low").unwrap().args,
            vec!["run", "--fast"],
            "operator variants expand into their own tags"
        );
        assert_eq!(
            set.get("fam").unwrap().bin,
            "fam-cli",
            "operator base loads too"
        );
        assert_eq!(
            set.get(&parent).unwrap().bin,
            format!("{parent}-cli"),
            "operator spec replaced the base tag wholesale"
        );
        assert_eq!(
            set.get(&variant.tag).unwrap().args,
            variant.args,
            "sibling embedded variant tags are untouched by a base override"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn operator_dir_overrides_embedded_and_rejects_duplicates() {
        let dir = std::env::temp_dir().join(format!("capspec-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // override SOME embedded spec wholesale — the tag is discovered from
        // the built-in set, never hardcoded.
        let builtins = builtin_specs();
        let (first, rest) = (&builtins[0].tag, &builtins[1].tag);
        std::fs::write(dir.join("my-override.toml"), spec_toml(first)).unwrap();
        let set = SpecSet::load(Some(&dir)).unwrap();
        assert_eq!(
            set.get(first).unwrap().bin,
            format!("{first}-cli"),
            "operator spec won"
        );
        assert!(set.get(rest).is_some(), "untouched built-in remains");

        // two operator files claiming one tag: hard error naming both files.
        std::fs::write(dir.join("zz-dup.toml"), spec_toml(first)).unwrap();
        let err = SpecSet::load(Some(&dir)).unwrap_err();
        assert!(err.contains("duplicate capability tag"), "got {err:?}");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
