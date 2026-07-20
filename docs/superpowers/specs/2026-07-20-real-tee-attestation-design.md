# Real TEE attestation — delete mock, ship only real verification

**Date:** 2026-07-20
**Decision (user):** delete mock attestation entirely. No forgeable quote code
ships anywhere; real TDX/SNP verification becomes the only path.

## Problem

Airlock's attestation has two halves. Quote **generation** is already real
(`tsm_gen_quote` reads Linux `configfs-tsm`; `tdx_guest`/`sev_guest`). Quote
**verification** — the half the trust model actually rests on — is mock-shaped:

- `attest.rs` ships `mock_quote`/`mock_verify` signed by the public seed
  `[7u8; 32]`: forgeable by construction, accepted by every production verifier
  when `--attest mock` is chosen.
- The product path (`capability-host` broker) and `airlock-broker` **refuse**
  `tdx`/`snp` — only mock is wired.
- `airlock-cli` has real TDX verify (dcap-qvl) but behind an off-by-default
  feature; SNP is structural-only (no AMD signature chain) with an
  `AIRLOCK_SNP_INSECURE_STRUCTURAL=1` escape hatch.
- `--attest` defaults to `mock` in both CLIs.

## Design

### One real verifier, in the airlock crate

New module `airlock::verify` (feature `verify`: pulls `dcap-qvl`, `sev`,
`reqwest`; enabled by every consumer so nothing is compile-vacuous):

```
pub async fn verify_quote(mode, quote, expected, roots: &TrustRoots)
    -> Result<[u8; REPORT_DATA_LEN]>
```

- **TDX** — the dcap-qvl verify moves here from `airlock-cli`, unchanged in
  substance: fetch collateral (PCCS URL, Intel PCS default), full DCAP verify,
  MRTD == pinned measurement, return REPORTDATA.
- **SNP** — the new real implementation (`sev = { version = "8",
  default-features = false, features = ["snp", "crypto_nossl"] }`, pure
  RustCrypto, no openssl):
  1. Parse the ATTESTATION_REPORT (all report versions the crate supports).
  2. Obtain the VCEK: fetch from AMD KDS
     (`https://kdsintf.amd.com/vcek/v1/<product>/<chip_id>?<SPLs from
     reported_tcb>`), or a local DER file override for air-gapped boxes. A
     VLEK-signed report is refused (named error) — VCEK only in this slice.
  3. Verify ARK → ASK → VCEK against the **pinned AMD root certs built into
     the sev crate** for the operator-pinned product generation
     (`Milan | Genoa | Turin`), then the report signature (ECDSA P-384) under
     the VCEK.
  4. Measurement == pinned measurement; return REPORTDATA.
  Every hop fails closed. `AIRLOCK_SNP_INSECURE_STRUCTURAL` is deleted.
- `AttestMode` loses `Mock`: `{ Tdx, Snp }`.

### TrustRoots: injection, not modes

`TrustRoots::default()` = production trust: Intel PCS/PCCS URL for TDX
collateral; AMD builtin ARK/ASK + KDS (or the VCEK file) for SNP, plus the
pinned SNP product. Tests construct custom roots.

This is the security story that replaces mock: a caller that injects fake
roots (or a gateway that injects a fake quoter) **only fools itself** — the
trust boundary is the *verifying* side, and every production entry point
(broker env config, CLI flags) hard-codes `TrustRoots::default()`. There is no
env var that swaps a trust root. (`PCCS_URL`, the KDS base, and the VCEK file
are *transport* config — whatever they return must still chain to the pinned
Intel/AMD roots.)

### Wiring (the refusal arms die)

- `capability-host::verify_gateway`: `Tdx | Snp` → `airlock::verify`. The
  `airlock` dep gains the `verify` feature unconditionally. New env when
  `attest=snp`: `DUCKTAPE_AIRLOCK_SNP_PRODUCT` (required),
  `DUCKTAPE_AIRLOCK_SNP_VCEK` (optional file override).
- `airlock-broker`: same call; `--attest` becomes required (no default); adds
  `--snp-product`/`--snp-vcek`.
- `airlock-cli`: thin caller of `airlock::verify`; its private tdx/snp modules
  and `tdx`/`snp` cargo features are deleted; `--attest` becomes required.
- `airlock::server`: the mock branch of `build`/`build_seeded` is deleted;
  `GatewayConfig.attest` parses to `tdx | snp | auto` only. A new
  `build_with_quoter(cfg, credential, quoter)` seam lets tests supply quote
  generation (production quoter = configfs-tsm; a process that injects a
  quoter already controls the process, and clients verify anyway).

### Tests without silicon (this box is a Ryzen — no TDX/SNP)

Verification is pure software; only *generation* needs hardware.

1. **Real-fixture verify tests** — vendor real, vendor-signed artifacts and run
   the actual chain code offline:
   - TDX: a published sample quote + collateral (Phala dcap-qvl repo), verified
     at a pinned `now`; tampered variants must fail.
   - SNP: a real attestation report + VCEK chain from the virtee/sev or
     snpguest test data; green path plus tampered-report / wrong-measurement /
     wrong-product red paths.
2. **Custody e2e** (attest → seal → handshake → proxy → credential swap) —
   rewritten on the injection seams: the in-process gateway gets a test quoter,
   the client verifies via the REAL `airlock::verify` path with test roots.
   Preferred quoter: a **minted SNP test chain** (test-generated ARK/ASK/VCEK,
   report bytes carrying the session's live REPORTDATA) so the real parser +
   signature verifier run end-to-end. If minting AMD-shaped certs proves
   infeasible against the sev crate's checks, fallback: the e2e injects a
   test verify closure (custody logic and verify logic are then covered by
   separate tests — still zero mock in shipped code).
3. Affected suites to rewrite: `airlock/tests/e2e.rs`, capability-host's
   in-process airlock tests, `bin/node/tests/airlock_gateway_e2e.rs` (its
   gateway is in-process test code, so injection works; the node binary itself
   never needs a fake).

### What deleting mock costs (accepted)

- A live gateway **process** on a non-TEE box can no longer attest — by
  design: a box that cannot attest cannot serve credentials. The
  podman → gateway → real-API live lane becomes hardware-only (the 07-19 PONG
  result stands as the recorded proof of that chain).
- PR #684's embedded-gateway e2e (node binary runs the gateway) becomes
  hardware-only when that suspended PR is revived; its `attest` default must
  also drop `mock`.
- Real verify adds a network fetch (Intel PCS / AMD KDS) at session-open, with
  file overrides for air-gapped; KDS is rate-limited, so the fetched VCEK is
  cached per chip_id in-process.
- `dcap-qvl` + `sev` join the node's dependency tree (both pure Rust).

## Out of scope

- SNP `auxblob` (configfs-tsm cert-table) serving; VLEK support.
- SSE-over-overlay streaming (tracked in the exec/auth spec §graft).
- Running generation/verification against real silicon — task #26 (full
  2-node + TEE validation) on the TDX box.
