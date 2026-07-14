# Superpowers Working Records

This directory is not an archive dump. Keep files here only when they are still
useful working records: active design records, approved specs, execution plans
that have not clearly been folded into maintained docs/code/tests, or review
findings that still explain current behavior.

Prune a file only after reviewing its content and identifying the current owner
of any durable facts. Valid owners are Vocs pages, ADRs, maintained runbooks,
checked-in tests, code comments, or git history for explicitly obsolete work.

## Reviewed Prune Set

The following files were reviewed and intentionally left pruned:

- `plans/2026-07-04-pluggable-network-entry-phase1-plan.md` — completed sentry
  implementation plan; the maintained operator record is
  `docs/deploy/sentry-deployment.md`.
- `plans/2026-07-05-slice2-coordinator-relay.md` — explicitly marked historical;
  the DERP-style coordinator relay was removed.
- `plans/2026-07-05-slice3-hardening-simnat.md` — explicitly marked historical
  because it referenced the removed relay path.
- `plans/2026-07-05-slice4-deploy-runbook.md` — explicitly marked historical;
  maintained operator material lives under `docs/deploy/` and
  `ops/coordinator/`.
- `specs/2026-07-03-fleet-isolation-finding.md` — solved root-cause note; the
  still-useful operational warning is in `skills/qa/SKILL.md`.
Everything else from the previous deletion was restored because it was marked as
a design of record, approved spec, active/executing plan, planned work, or was
otherwise not proven obsolete by the review.
