---
assignees:
- claude-code
comments:
- actor: wballard
  id: 01kvg6qvh31sb4mnr1y3vft09d
  text: |-
    Picked up. Research done on all three rule systems:

    1. builtin/shell/config.yaml — the LIVE deny list. Production path is execute_command/mod.rs:326 → validate_command → load_validator → load_shell_config (YAML stack). This is single source of truth for the live tool.

    2. security.rs ShellSecurityPolicy::default().blocked_commands — hardcoded Rust duplicate of YAML + extra `sed\s+.*` (drifted). NOT on the live path (only used by with_default_policy() and unit tests). Plan: replace the hand-maintained list by deriving from BUILTIN_CONFIG_YAML so there is one source of truth.

    3. hardening.rs ThreatDetector / HardenedSecurityValidator — confirmed DEAD. grep across whole workspace: validate_command_comprehensive / analyze_command / HardenedSecurityValidator are referenced ONLY inside hardening.rs's own tests + re-exported in lib.rs. No production consumer gates on SecurityAssessment. Plan: remove the entire hardening module + its lib.rs re-exports.

    Pattern decisions per task:
    - ELIMINATE from YAML: format\s+, eval\s+, exec\s+/bin/, ssh\s+.*@, /etc/passwd, /etc/shadow.
    - KEEP: rm -rf /, rm -rf *, dd if=...of=/dev/, mkfs, fdisk, parted, chmod +s, nc -l, wget|sh, curl|sh.
    - Open-question patterns (shutdown/reboot/sudo/systemctl/crontab): KEEP as mistake-guards. sudo override is already handled by permit overlays (config_stacking_test relies on it), so keeping sudo deny + permit-override is the documented design. Not dropping these — they are low-FP mistake guards and removing sudo would break the permit-override story the tests pin.

    Tests to update: config_stacking_test hot_reload (eval→ switch to a kept pattern), config.rs sed permit/deny unit tests, security.rs default-list test expectations, hardening.rs tests (deleted with module).
  timestamp: 2026-06-19T15:07:10.243885+00:00
- actor: wballard
  id: 01kvg7j82s33kksa4ygj7gtz7j
  text: |-
    Implementation landed. Changes:

    - builtin/shell/config.yaml: removed false-positive magnets (format\s+, eval\s+, exec\s+/bin/, ssh\s+.*@, /etc/passwd, /etc/shadow). Kept catastrophic-mistake guards + system-state/download-exec guards. Added a header comment documenting that these are mistake guards, NOT a security boundary.
    - security.rs ShellSecurityPolicy::default(): now derives blocked_commands from parse_shell_config(BUILTIN_CONFIG_YAML) — single source of truth, drift eliminated. The hand-copied list (incl. the drifted sed\s+.* that was never in YAML) is gone.
    - hardening.rs: DELETED entirely (ThreatDetector / HardenedSecurityValidator / SecurityAssessment). Confirmed dead — no production consumer gated on it. Removed lib.rs module decl + re-exports + doc mentions.

    Tests:
    - NEW crates/swissarmyhammer-shell/tests/builtin_deny_patterns_test.rs — TDD RED-first, pins acceptance criteria through the LIVE builtin config path (parse → compile → evaluate_command). 3 tests: legit dev commands allowed, catastrophic guards blocked, eliminated patterns absent.
    - config_stacking_test hot_reload: re-pointed eval→sudo (kept pattern), permit-override still exercised.
    - config.rs builtin test: now asserts eliminated patterns absent instead of count>=19.
    - tools execute_command: removed eliminated patterns from the two builtin-policy blocked-command lists; kept the explicit-policy and disabled-validation tests as-is.

    Verification (really-done):
    - cargo nextest run -p swissarmyhammer-shell → 106 passed, 0 failed.
    - cargo nextest run -p swissarmyhammer-tools shell:: → 176 passed.
    - cargo clippy -p swissarmyhammer-shell AND -p swissarmyhammer-tools --all-targets → clean.
    - double-check agent: PASS, no findings. Confirmed hardening dead via workspace-wide grep, single-source-of-truth confirmed, new test non-vacuous.

    Moving to review.
  timestamp: 2026-06-19T15:21:35.065094+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffc880
title: 'Shell rules: eliminate pointless substring deny patterns (false-positive magnets) and consolidate the three rule systems'
---
## Why

The shell tool runs **AI-generated** commands. Substring deny patterns can't be a real security boundary against an agent that composes arbitrary commands (trivially evadable via quoting, `$IFS`, base64, here-strings, etc.) — but they **do** constantly false-positive on legit dev commands. So they impose cost (blocked work, confusing "BLOCKED" errors) with ~zero security benefit.

Concrete repro:
```
cargo test eval 2>&1 | tail -30   → BLOCKED: "Dynamic code evaluation"
```
because the deny pattern `eval\s+` matches the **test-name substring** "eval ". Same class of bug for any command containing `eval`, `sed`, `format`, `exec`, etc. as an argument.

## The real scope: three overlapping rule systems

There isn't one rule list — there are **three**, which is also a consolidation/drift problem:

1. **`builtin/shell/config.yaml`** — embedded `deny:` list, hard block, default-allow (lowest precedence; user/project `.shell/config.yaml` overlay on top).
2. **`crates/swissarmyhammer-shell/src/security.rs` → `ShellSecurityPolicy::default().blocked_commands`** (~lines 124–153) — a **hardcoded Rust duplicate** of the YAML list, **plus** an extra `sed\s+.*` ("sed ends in pain, use the editing tools") that is **not** in the YAML. The two lists have already drifted.
3. **`crates/swissarmyhammer-shell/src/hardening.rs` → `ThreatDetector`** (`malicious_patterns` / `suspicious_patterns`, ~lines 261–277) — a separate threat-**scorer**. `malicious_patterns` includes `[;&|` + "`" + `$()]` which matches **almost every compound command** (`a && b`, `x | tail`, `$(...)`, backticks); `suspicious_patterns` flags `grep -r`, `find -exec`, `ssh|scp|rsync`, `base64|xxd|hexdump`.

## Pointless / FP-prone — eliminate

- `eval\s+` ("Dynamic code evaluation") — matches `cargo test eval`, any `eval` arg. (YAML + security.rs)
- `sed\s+.*` — blocks **all** sed, including in pipes. (security.rs only)
- `format\s+` — Windows disk-format; irrelevant on dev boxes, matches `...format ...` substrings.
- `exec\s+/bin/` — theater.
- `ssh\s+.*@` (deny) and `ssh\s+|scp\s+|rsync\s+` (suspicious) — block/flag legit remote + file-transfer ops.
- `/etc/passwd`, `/etc/shadow` — **reading** is harmless and these FP when grepping docs/fixtures that merely mention them.
- hardening `[;&|`$()]` (malicious) — flags nearly every real command → constant High-threat noise.
- hardening suspicious: `grep -r`, `find -exec`, `base64|xxd|hexdump` — all common and benign.

## Keep (catastrophic-mistake guards, low FP — advisory, NOT a security boundary)

- `rm\s+-rf\s+/`, `rm\s+-rf\s+\*`
- `dd\s+if=.*of=/dev/`
- `mkfs`, `fdisk`, `parted` (raw disk)
- Open question to decide in the task: `shutdown`/`reboot`/`sudo`/`systemctl`/`crontab` (mistake-guards, but `sudo` blocks legit installs in some flows) and `wget|sh`/`curl|sh` (download-and-execute — keep as advisory or drop?).

## Structural fix (do this, not just delete lines)

- **Single source of truth.** `security.rs`'s `Default` should derive the deny list from the embedded YAML, not maintain a hand-copied parallel list. Eliminate the drift (the `sed` divergence is the proof it already drifted).
- **Decide `hardening.rs ThreatDetector`'s fate.** Confirm whether anything actually consumes its `SecurityAssessment` to block/gate, or whether it's noise-only/dead. If dead → remove the layer; if live → prune the absurd patterns. (Investigate consumers before editing.)

## Tests that PIN these rules (must update)

- `crates/swissarmyhammer-shell/tests/config_stacking_test.rs:117–147` — asserts the builtin denies `eval\s+` and that a project overlay can permit it. Re-point the permit-override test at a *kept* pattern (e.g. `rm -rf /`) or remove.
- `crates/swissarmyhammer-shell/src/config.rs` unit tests (~600–700) — `sed -i ...` asserted `is_err()` at ~line 700; update when dropping `sed`.
- Any `hardening.rs` tests asserting malicious/suspicious matches.

## Acceptance criteria

- [ ] These all PASS validation (not blocked, not High-threat): `cargo test eval`, `cargo nextest run eval`, `grep -r foo .`, `cd a && cargo build | tail`, `sed -n '1,5p' f` in a pipe, a command mentioning `/etc/passwd` in a doc grep.
- [ ] Catastrophic-mistake guards still blocked: `rm -rf /`, `dd if=x of=/dev/disk0`, `mkfs ...`.
- [ ] Deny patterns have a **single source of truth** — no duplicated hand-maintained list in `security.rs`.
- [ ] `hardening.rs` ThreatDetector either removed or its over-broad patterns pruned, based on whether a consumer actually gates on it.
- [ ] Full `swissarmyhammer-shell` suite green (cargo nextest), clippy clean; pinned tests updated rather than deleted-around. #shell-config #tech-debt