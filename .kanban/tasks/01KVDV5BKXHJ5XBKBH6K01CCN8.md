---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kw33n418m3ty15h701h00v4a
  text: |-
    Picked up. Research findings:

    - The two helpers are NOT byte-identical anymore (task description's "verbatim identical" was stale). supervisor.rs `fake_spec` uses file_extensions=[], startup_timeout_secs=TEST_STARTUP_TIMEOUT_SECS(1), health_check_interval_secs=1, install_hint=""; daemon.rs `test_spec` uses file_extensions=["txt"], startup_timeout_secs=5, health_check_interval_secs=60, install_hint="install {cmd}".
    - However the fast timeout/health values in `fake_spec` are NOT behaviorally load-bearing: every `fake_spec` call site either inserts an un-started daemon (never spawned) or uses a nonexistent binary (instant BinaryNotFound at spawn, timeout never elapses). The ONE test that spawns a real fast-exiting process ("true") uses an inline spec, not fake_spec.
    - A shared `#[cfg(test)] pub(crate) mod test_support` already exists (src/test_support.rs) — the ideal "common test-support location".

    Plan: hoist daemon's `test_spec` into test_support; daemon + supervisor both import it; delete `fake_spec` and repoint its 14 call sites to `test_spec`. The inline "true" spec stays inline (genuinely needs fast values) and gets the named const. Add `const TEST_HEALTH_CHECK_INTERVAL_SECS: u64 = 1;` next to TEST_STARTUP_TIMEOUT_SECS and use it at the inline site. Note: after deleting fake_spec only one `:1` literal remains (the inline "true" spec), so item 2's "two sites" collapses to one. Pure test-code refactor; verify via existing suite staying green.
  timestamp: 2026-06-26T23:18:49.128646+00:00
- actor: claude-code
  id: 01kw33wjrc4tjxkq90fgx17dcm
  text: |-
    Implemented and verified GREEN. Left in `doing` for /review.

    Changes (3 files, test-code only):
    - crates/swissarmyhammer-lsp/src/test_support.rs — added shared `pub(crate) fn test_spec(command: &str) -> OwnedLspServerSpec` (daemon's existing field values) into the existing #[cfg(test)] test_support module.
    - crates/swissarmyhammer-lsp/src/daemon.rs — removed the local `test_spec` helper; added `use crate::test_support::test_spec;`. Its ~50 call sites unchanged.
    - crates/swissarmyhammer-lsp/src/supervisor.rs — deleted `fake_spec`; added `use crate::test_support::test_spec;`; repointed all 14 `fake_spec(...)` call sites to `test_spec(...)`. Added `const TEST_HEALTH_CHECK_INTERVAL_SECS: u64 = 1;` next to TEST_STARTUP_TIMEOUT_SECS and used it at the inline "true" spec (the lone surviving health_check_interval_secs literal). The "true" spec stays inline because it genuinely needs the fast TEST_STARTUP_TIMEOUT_SECS/health values.

    Behavior preserved: every former fake_spec site inserts an un-started daemon (never spawned) or a nonexistent binary (instant BinaryNotFound, timeout never elapses), so the divergent fast values fake_spec carried are unobservable; the only fast-exiting real-process test ("true") kept its inline fast spec.

    Verification:
    - cargo nextest run -p swissarmyhammer-lsp: 221 passed, 0 failed (~4.08s, well under the <10s budget).
    - cargo clippy -p swissarmyhammer-lsp --all-targets -- -D warnings: clean.
    - cargo fmt applied.
    - Adversarial double-check: PASS (confirmed no assertion drift on install_hint/file_extensions, no dead imports/consts, no scope creep).

    Note on item 2's "two sites": after deleting fake_spec, only one `health_check_interval_secs: 1` literal remained (the inline "true" spec); it now uses the named const. The other site was inside fake_spec, which is gone.
  timestamp: 2026-06-26T23:22:53.580676+00:00
- actor: claude-code
  id: 01kw34jd7j0a9d38p5wgemx9ja
  text: 'Review-nit fix applied to test_support.rs: extracted the two remaining magic literals in the shared `test_spec` builder into named module-level consts and used them:\n- `const TEST_SPEC_STARTUP_TIMEOUT_SECS: u64 = 5;`\n- `const TEST_SPEC_HEALTH_CHECK_INTERVAL_SECS: u64 = 60;`\n\nNamed distinctly (TEST_SPEC_* prefix) to avoid collision/confusion with supervisor.rs''s same-purpose-but-faster `TEST_STARTUP_TIMEOUT_SECS` / `TEST_HEALTH_CHECK_INTERVAL_SECS` (used by its inline real-process \"true\" spec, different values). Each const carries a doc comment noting it''s the shared test_spec builder default and calling out the distinction. Values unchanged (5 / 60); supervisor.rs untouched.\n\nVerification:\n- cargo nextest run -p swissarmyhammer-lsp: 221 passed, 0 failed (~4.08s).\n- cargo clippy -p swissarmyhammer-lsp --all-targets -- -D warnings: clean.\n- cargo fmt applied.\n\nLeft in `doing` for /review.'
  timestamp: 2026-06-26T23:34:48.818167+00:00
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffffff880
title: Dedupe test-spec helpers in swissarmyhammer-lsp (fake_spec/test_spec) + name health-check interval constant
---
## What
Test-scaffolding cleanup surfaced by the `review working` engine while reviewing `^7a5h2bj` (leader-gated LSP spawn). These are **pre-existing test-code** items, not part of the leadership change, so they were deliberately split out rather than churning that task.

## Items
- [ ] `crates/swissarmyhammer-lsp/src/supervisor.rs` — `fake_spec` test helper is verbatim identical to `test_spec` in `daemon.rs` (test module). Two spec-builder fns in the same crate differing only by name → drift risk. Consolidate to one shared test helper (import `daemon::tests::test_spec` or hoist a shared `test_spec` into a common test-support location) and remove `fake_spec`.
- [ ] `crates/swissarmyhammer-lsp/src/supervisor.rs` — hardcoded `health_check_interval_secs: 1` literals in test specs (two sites). Mirror the existing `TEST_STARTUP_TIMEOUT_SECS` pattern: add `const TEST_HEALTH_CHECK_INTERVAL_SECS: u64 = 1;` and use it at both sites.

## Notes
- Pure test-code quality; no production behavior change. Keep tests fast (<10s). Run `cargo test -p swissarmyhammer-lsp` + clippy `-D warnings` green.
- Line numbers in the original findings were stale post-refactor; target by symbol. #diagnostics