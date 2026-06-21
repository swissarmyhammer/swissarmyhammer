---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvepp7ekqsqzkfr9fnd7cev5
  text: 'Picked up by /finish $diagnostics, after ^w0nwx3r → done. Dependency satisfied: inline-on-edit fold-in landed (the redirect target `files edit` already attaches diagnostics). Config+docs task, no model in loop. Ship an INSTALLABLE Claude Code settings fragment via swissarmyhammer''s init/config surface (NOT hand-written): permission deny on Edit/Write/MultiEdit + a PreToolUse hook redirecting those to the `files edit` MCP op; valid settings.json; no-op on hosts without hook support. Docs must state the shell-closing prerequisite (an open Bash writes via cat>/sed -i, bypassing the tool — so shell-shorting is the prerequisite for a truly closed write surface; the leader watcher is the async backstop) and the latency/reliability tradeoff. Out of scope: shell-closing itself. Use the update-config skill''s settings.json conventions. Driving implement → test → review.'
  timestamp: 2026-06-19T01:07:25.267689+00:00
- actor: claude-code
  id: 01kveqgg8kty8mp66cys62zfve
  text: |-
    IMPLEMENTED (TDD). Extended the EXISTING init/config surface — mirdan's data-driven `Profile` + `init_profile`/`deinit_profile` (the same path that ships statusline/preamble and the serve-time `deny_tool`). NOT a new deployer. No second config mechanism.

    WHAT:
    - crates/mirdan/src/install.rs: new `Profile.edit_redirect: bool` flag; `desired_edit_redirect_fragment()` (single source of truth for the fragment), `apply_edit_redirect_at(path, install)` (merges via the existing `settings::ensure_array_contains`/`remove_from_array` primitives — idempotent, non-clobbering), `apply_profile_edit_redirect()` (writes to every detected agent's settings file, mirroring `apply_profile_statusline`). Wired into init/deinit behind the flag.
    - apps/swissarmyhammer-cli/src/commands/profile.rs: `sah_profile()` sets `edit_redirect: true`. Kept the mirdan consistency-test mirror (`profile_consistency_tests::sah_profile`) in sync.
    - crates/mirdan/src/plugin.rs: added `edit_redirect: false` to the one exhaustive Profile literal.

    DENY+HOOK JSON SHAPE (validated against the Claude Code settings.json schema from the update-config skill):
    - permissions.deny = ["Edit","Write","MultiEdit"]
    - hooks.PreToolUse = [ { "matcher": "Edit|Write|MultiEdit", "hooks": [ { "type":"command", "command": "printf %s '<json>'" } ] } ] where <json> is hookSpecificOutput {hookEventName:"PreToolUse", permissionDecision:"deny", permissionDecisionReason: "...use the `files` MCP tool (op \"edit file\"/\"write file\")..."}. Verified the `files edit` target: the MCP tool is named `files` (served by the `sah` server), edit op dispatched via {"op":"edit file"} (crates/swissarmyhammer-tools/src/mcp/tools/files/mod.rs). Valid settings.json; no-op on hosts without hook support (unrecognized `hooks` block is ignored, not an error).

    DEAD END considered + rejected: a `mcp_tool` hook type (auto-invoking the files server) — rejected because it can't cleanly map MultiEdit's edits[] and adds a server dependency; the command-hook deny+redirect is host-support-agnostic and covers all three tools uniformly. Also did NOT hook this onto `replacement_natives()` (that derives from ToolCategory::Replacement; `files` is Agent and Edit/Write/MultiEdit are three natives, a distinct concern).

    DOCS: doc/src/concepts/closed-write-surface.md (linked in SUMMARY.md) — states the closed-write-surface goal, the shell-closing prerequisite (open Bash writes via cat>/sed -i bypass the tool; leader watcher = async backstop), and the latency/reliability tradeoff (native Edit fast + model tuned to it; MCP adds latency + we own edit reliability; worth it only while files edit stays >= native reliability). Out of scope (shell-closing) explicitly noted.

    TESTS: mirdan lib 403 passed / 0 failed (incl. 5 new edit_redirect tests covering valid-fragment shape, idempotency, non-clobber merge, clean removal, full init/deinit round-trip). CLI profile test passes. clippy -p mirdan and -p swissarmyhammer-cli --all-targets -D warnings: clean. cargo fmt --check: clean. .config/nextest.toml untouched (fail-fast=false intact). Running really-done adversarial double-check next.
  timestamp: 2026-06-19T01:21:46.259527+00:00
- actor: claude-code
  id: 01kveqreknvv1we2nsxrtb3179
  text: 'really-done: PASS. Verification commands green (mirdan lib 403/0; CLI profile test pass; clippy -p mirdan & -p swissarmyhammer-cli --all-targets -D warnings clean; cargo fmt --check clean; nextest.toml untouched). Adversarial double-check returned PASS on all 6 scrutiny points (existing surface not a new deployer; valid Claude Code settings.json; redirect targets real `files` MCP tool/op; idempotent + non-clobbering merge; consistency-test mirror in sync; docs cover all three required points). One non-blocking observation: deinit removes the three deny strings individually (matches existing deny/statusline removal semantics; the meaningful unit — the PreToolUse matcher group — is added/removed atomically by full-object equality). Acceptable, no change. Moving to review.'
  timestamp: 2026-06-19T01:26:06.709959+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffc480
project: diagnostics
title: 'Close the write surface: Claude Code deny+redirect host-config fragment'
---
## What
The hard lever that promotes the foreign-host native-edit case to model-facing: force every mutation through the instrumented `files edit` tool so diagnostics always ride the result. An MCP server cannot disable a host's built-in tools, so this ships as a **host-config fragment the user installs**.

- For Claude Code: a settings fragment that sets permission `deny` on `Edit`/`Write`/`MultiEdit` plus a `PreToolUse` hook that redirects those to the `files edit` MCP op. Ship the fragment via swissarmyhammer's init/config surface (the `update-config`/init path) so it is installable, not hand-written. Hook-capable hosts only.
- **Document the prerequisite explicitly:** editing cannot be closed without shell already closed — an open `Bash` writes files via `cat >`/`sed -i`, bypassing the tool and diagnostics. So shell-shorting is the prerequisite for a truly **closed write surface**; the leader watcher remains the async backstop for what still leaks. This task ships only the editing-surface fragment + docs; it does NOT implement shell-closing (out of scope, separate initiative).
- Note the tradeoff in docs: native `Edit` is fast and the model is tuned to it; routing through MCP adds latency and makes us own edit reliability — worth it only while `files edit` stays at least as reliable as the tool it displaces.

## Depends on
- "Inline-on-edit: mutated_paths + shared diagnostics fold-in step" (the redirect target must already attach diagnostics)

## Acceptance Criteria
- [ ] An installable Claude Code settings fragment denies `Edit`/`Write`/`MultiEdit` and adds a `PreToolUse` redirect to `files edit`, shipped via the init/config surface.
- [ ] Docs state the shell-closing prerequisite and the closed-write-surface goal, and the latency/reliability tradeoff.
- [ ] Fragment is valid settings.json and a no-op on hosts without hook support.

## Tests
- [ ] `cargo test` (config crate / tools): the generated fragment parses as valid settings JSON and contains the deny entries + PreToolUse redirect; an installer test asserts it merges into a settings chain without clobbering unrelated keys.

## Workflow
- Use `/tdd`. This is config + docs; no model in the loop. Use the `update-config` skill's settings.json conventions. #diagnostics