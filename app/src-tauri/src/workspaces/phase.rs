//! Onboarding-phase classification from the node's `daemon.log` markers, plus
//! the small parsing helpers that read process facts back.

use std::fs;
use std::io::{Read as _, Seek as _, SeekFrom};
use std::path::Path;

use serde::Serialize;

/// the onboarding phase `workspace_phase` reads back from the node log.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhaseReport {
    /// one of: starting | parked | admitted | synced | promoted | fatal.
    pub phase: String,
    /// the trailing text of the marker line, for a live status string.
    pub detail: Option<String>,
}

/// map the node's stable stdout markers to a phase. the log only appends and,
/// within a boot, prints these markers in phase order — so the latest
/// non-regressing marker is the current phase. fatal still wins when it is
/// latest, but late `joining:` retry noise cannot move an already admitted /
/// synced / promoted boot back to the first step.
pub(super) fn classify(log: &str) -> PhaseReport {
    // (phase, marker substring). the strings are a contract with
    // bin/node/src/main.rs (asserted by bin/node/tests/invite_e2e.rs).
    // "parked" is the phase id the webview already maps; since auto-
    // redemption the underlying markers read "joining:" (no member approval
    // step — the invite redeems itself).
    const MARKERS: &[(&str, &str)] = &[
        ("parked", "joiner mode:"),
        ("parked", "joining:"),
        // the synchronous join gate (ADR §3.3) prints this the instant a member
        // answers Admitted — the authoritative admission, ahead of any sync.
        ("admitted", "ADMITTED at height"),
        ("admitted", "admitted at epoch"),
        ("admitted", "resident: standing granted"),
        ("synced", "synced app_hash="),
        ("synced", "resident: pre-synced boundary"),
        ("promoted", "promoted:"),
        ("fatal", "FATAL"),
        // a raw Rust panic on boot ("thread 'main' panicked at …") prints no
        // node marker — catch it so a crashed node stops reading as "starting".
        ("fatal", "panicked at"),
    ];
    let mut latest: Option<(&str, String)> = None;
    for line in log.lines() {
        if let Some((phase, _)) = MARKERS.iter().find(|(_, needle)| line.contains(needle)) {
            if *phase == "parked"
                && matches!(
                    latest.as_ref().map(|(phase, _)| *phase),
                    Some("admitted" | "synced" | "promoted")
                )
            {
                continue;
            }
            let detail = line
                .split_once("] ")
                .map(|(_, rest)| rest)
                .unwrap_or(line)
                .trim()
                .to_string();
            latest = Some((phase, detail));
        }
    }
    match latest {
        Some((phase, detail)) => PhaseReport {
            phase: phase.to_string(),
            detail: Some(detail),
        },
        // no marker yet: a founder never parks, so this is just early boot.
        None => PhaseReport {
            phase: "starting".into(),
            detail: None,
        },
    }
}

/// parse the `join-state` verb's JSON (`{"phase","detail","height"?}`, or the
/// literal `null` when the node has no join-state projection) into a
/// [`PhaseReport`]. `None` when the output is not a phase object — the caller
/// then falls back to the log classification. Only the node's four
/// positive-ladder phases (`parked|admitted|synced|promoted`) come from here;
/// `starting`/`fatal` have no RPC source and stay log/pid-derived.
pub(super) fn parse_join_state(out: &str) -> Option<PhaseReport> {
    let value: serde_json::Value = serde_json::from_str(out.trim()).ok()?;
    let phase = value.get("phase")?.as_str()?;
    if !matches!(phase, "parked" | "admitted" | "synced" | "promoted") {
        return None;
    }
    let detail = value
        .get("detail")
        .and_then(|d| d.as_str())
        .filter(|d| !d.is_empty())
        .map(|d| {
            match value.get("height").and_then(|h| h.as_u64()) {
                Some(h) => format!("{d} (height {h})"),
                None => d.to_string(),
            }
        });
    Some(PhaseReport {
        phase: phase.to_string(),
        detail,
    })
}

/// the last `max` bytes of a file as lossy utf-8; empty string if absent.
pub(crate) fn read_tail(path: &Path, max: u64) -> Result<String, String> {
    let mut file = match fs::File::open(path) {
        Ok(f) => f,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(String::new()),
        Err(err) => return Err(format!("open {path:?}: {err}")),
    };
    let len = file.metadata().map_err(|err| err.to_string())?.len();
    let start = len.saturating_sub(max);
    file.seek(SeekFrom::Start(start))
        .map_err(|err| err.to_string())?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).map_err(|err| err.to_string())?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

/// parse a `ps -o etime` field ("[[dd-]hh:]mm:ss") into whole seconds. returns
/// `None` for any shape it doesn't recognize (blank, out-of-range, too many
/// colon groups) rather than guessing.
#[cfg(any(unix, test))]
pub(super) fn parse_etime(raw: &str) -> Option<u64> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let (days, hms) = match raw.split_once('-') {
        Some((d, rest)) => (d.parse::<u64>().ok()?, rest),
        None => (0u64, raw),
    };
    let mut fields = hms.split(':').rev();
    let secs: u64 = fields.next()?.parse().ok()?;
    let mins: u64 = fields.next()?.parse().ok()?;
    let hours: u64 = match fields.next() {
        Some(h) => h.parse().ok()?,
        None => 0,
    };
    // a valid etime is at most dd-hh:mm:ss — a fourth colon group is malformed.
    if fields.next().is_some() || secs >= 60 || mins >= 60 {
        return None;
    }
    Some(((days * 24 + hours) * 60 + mins) * 60 + secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_etime_handles_every_field_width() {
        assert_eq!(parse_etime("05"), None); // ss alone is not a valid etime
        assert_eq!(parse_etime("01:05"), Some(65)); // mm:ss
        assert_eq!(parse_etime("02:01:05"), Some(2 * 3600 + 65)); // hh:mm:ss
        assert_eq!(parse_etime("3-02:01:05"), Some(3 * 86_400 + 2 * 3600 + 65)); // dd-hh:mm:ss
        assert_eq!(parse_etime("  01:05  "), Some(65)); // ps pads the field
    }

    #[test]
    fn parse_etime_rejects_malformed() {
        assert_eq!(parse_etime(""), None);
        assert_eq!(parse_etime("nope"), None);
        assert_eq!(parse_etime("99:99"), None); // out-of-range mm/ss
        assert_eq!(parse_etime("1:2:3:4"), None); // too many groups
    }

    #[test]
    fn classify_ranks_latest_phase() {
        let log = "[node ab] joiner mode: parking on the mesh\n\
                   [node ab] parked: awaiting admission (epoch 0 has 1 validators)\n\
                   [node ab] admitted at epoch 1 boundary 4 — syncing 16 modules\n\
                   [node ab] synced app_hash=deadbeef\n\
                   [node ab] promoted: validator at epoch 1 boundary 4 — rebooting\n";
        let r = classify(log);
        assert_eq!(r.phase, "promoted");
    }

    #[test]
    fn classify_parked_holds_until_admitted() {
        let log = "[node ab] joiner mode: announcing this key with the invite token\n\
                   [node ab] joining: awaiting redemption (epoch 0 has 1 validators)\n";
        let r = classify(log);
        assert_eq!(r.phase, "parked");
        assert!(r.detail.unwrap().contains("awaiting redemption"));
    }

    #[test]
    fn classify_empty_is_starting() {
        assert_eq!(classify("").phase, "starting");
    }

    #[test]
    fn classify_recovers_from_a_stale_fatal() {
        // an old fatal, then a restart that reparks and promotes on the same
        // appended log — the latest line wins, not the scariest one.
        let log = "[node ab] FATAL: still no standing after 900 attempts\n\
                   [node ab] joiner mode: announcing this key with the invite token\n\
                   [node ab] joining: awaiting redemption (epoch 0 has 1 validators)\n\
                   [node ab] promoted: validator at epoch 1 boundary 4 — rebooting\n";
        assert_eq!(classify(log).phase, "promoted");
    }

    #[test]
    fn classify_does_not_regress_after_sync_retry_noise() {
        let log = "[node ab] joiner mode: announcing this key with the invite token\n\
                   [node ab] admitted at epoch 1 boundary 4 — syncing 16 modules\n\
                   [node ab] synced app_hash=deadbeef\n\
                   [node ab] joining: redemption not landed yet (or the mesh is unreachable) — \
                   the announce keeps retrying and a member node redeems it automatically. \
                   retrying (server error: no finalized boundary to serve yet)\n";
        let report = classify(log);
        assert_eq!(report.phase, "synced");
        assert!(report.detail.as_deref().unwrap_or("").contains("app_hash"));
    }

    #[test]
    fn classify_gate_admitted_then_synced() {
        // the synchronous join gate (ADR §3.3): a tokened joiner prints
        // "ADMITTED at height N" the instant a member answers Admitted, then
        // pre-syncs. the admitted phase must show between parked and synced.
        let admitted = "[node ab] joiner mode: parking on the mesh\n\
                        [node ab] invite announce sent to member 11223344 — awaiting the gate (round 1)\n\
                        [node ab] ADMITTED at height 7 by member 55667788\n";
        assert_eq!(classify(admitted).phase, "admitted");
        let synced = format!(
            "{admitted}[node ab] resident: pre-synced boundary 9 app_hash=deadbeef\n"
        );
        assert_eq!(classify(&synced).phase, "synced");
    }

    #[test]
    fn classify_resident_presync_as_synced() {
        let log = "[node ab] joiner mode: announcing this key with the invite token\n\
                   [node ab] resident: standing granted — following boundaries and serving local reads\n\
                   [node ab] resident: pre-synced boundary 9 app_hash=deadbeef\n\
                   [node ab] joining: redemption not landed yet (or the mesh is unreachable) — \
                   the announce keeps retrying and a member node redeems it automatically. \
                   retrying (server error: no finalized boundary to serve yet)\n";
        let report = classify(log);
        assert_eq!(report.phase, "synced");
        assert!(
            report
                .detail
                .as_deref()
                .unwrap_or("")
                .contains("pre-synced")
        );
    }

    #[test]
    fn classify_flags_a_raw_panic_as_fatal() {
        // a boot panic prints no node marker; the "panicked at" catch-all must
        // still classify it fatal so the join room stops spinning over a corpse.
        let log = "[node ab] joiner mode: parking on the mesh\n\
                   thread 'main' panicked at bin/node/src/main.rs:42:9:\n\
                   called `Result::unwrap()` on an `Err` value: AddrInUse\n";
        let report = classify(log);
        assert_eq!(report.phase, "fatal");
        assert!(
            report.detail.as_deref().unwrap_or("").contains("panicked"),
            "detail: {:?}",
            report.detail
        );
    }

    #[test]
    fn parse_join_state_reads_the_positive_ladder_and_rejects_the_rest() {
        // the four rpc phases parse, with height folded into the detail.
        let synced = r#"{"phase":"synced","detail":"serving reads","height":9}"#;
        let r = parse_join_state(synced).expect("synced parses");
        assert_eq!(r.phase, "synced");
        assert_eq!(r.detail.as_deref(), Some("serving reads (height 9)"));

        let admitted = r#"{"phase":"admitted","detail":"standing granted — syncing"}"#;
        assert_eq!(parse_join_state(admitted).unwrap().phase, "admitted");
        assert_eq!(parse_join_state(r#"{"phase":"parked","detail":"awaiting"}"#).unwrap().phase, "parked");
        assert_eq!(parse_join_state(r#"{"phase":"promoted","detail":"validator","height":3}"#).unwrap().phase, "promoted");

        // starting/fatal have no rpc source — never accepted from here;
        // null (no projection) and junk fall back to the log classification.
        assert!(parse_join_state(r#"{"phase":"starting"}"#).is_none());
        assert!(parse_join_state(r#"{"phase":"fatal"}"#).is_none());
        assert!(parse_join_state("null").is_none());
        assert!(parse_join_state("not json").is_none());
    }

    #[test]
    fn classify_ignores_ordinary_log_lines() {
        // an ordinary info line must not trip any phase — only real markers do.
        let log = "[node ab] listening on 127.0.0.1:8844\n\
                   [node ab] indexed 12 blocks\n";
        assert_eq!(classify(log).phase, "starting");
    }
}
