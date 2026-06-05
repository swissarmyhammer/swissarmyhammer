---
assignees:
- claude-code
depends_on:
- 01KT57C9AFYCHVVK6VKK4V7W8A
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffd780
project: agent-builtins
title: 'serve: clientInfo-driven Bash deny for Claude (move off sah init)'
---
Apply the Bash deny at serve time, gated on the actual connecting client being Claude — the honest replacement for the init-time, all-detected-agents deny.

## Change
- In the serve path, after MCP `initialize`, read clientInfo. If the client is **Claude** → `mirdan::install::deny_tool(scope, "Bash")` idempotently. If **llama** → no-op (no native Bash). Reuse the existing, Claude-aware mirdan primitive (`apps/swissarmyhammer-cli/src/commands/install/components/mod.rs:151` is the current call site to mirror).
- This pairs with per-client served-set composition (shell is served to Claude here); together they make shell a true replacement for Bash rather than an addition.
- NOT an `Initializable`. Lives in serve, not init.

## Decide
- **Scope**: serve has a working dir — which `InitScope` does the serve-time deny target (Local vs Project)? → RESOLVED: `InitScope::Local` (`.claude/settings.local.json`, non-committed).
- **clientInfo → mirdan Claude AgentDef** mapping (share the mapping introduced by the served-set composition card). → RESOLVED: reuses card #2 `Host::from_client_info`.
- **Self-correct?** (open question): should serve re-allow Bash when a non-Claude client connects, or leave removal solely to `deinit`? Default: leave to deinit (paired card). → RESOLVED: no self-correct; deinit (#7) owns removal.

## Done when
- A Claude client connecting to `sah serve` results in Bash denied in Claude's settings (idempotent).
- A llama client triggers no deny.

## Review Findings (2026-06-03 14:35)

Reviewed card #6 surface: `server.rs` (`apply_serve_time_native_deny`, `initialize`), `tool_registry.rs` (`replacement_natives`), `host.rs` (`Host`/gate), `reporter.rs` (`TracingReporter`), and `tests/integration/serve_time_bash_deny.rs`. Gate, latch, data-driven derivation, error handling, and reuse all verified correct. All related unit + integration tests run green (3/3 serve_time_bash_deny, 7/7 host, 3/3 replacement_natives, 12/12 reporter).

**On the reported "known bug" (test leaking a real `.claude/settings.local.json` into the crate dir): could NOT reproduce — the test as written is correctly isolated, so this is NOT a blocker.** Root cause analysis of the original leak was off: mirdan's `ClaudeCodeStrategy::settings_path` for `InitScope::Local` derives from the agent's *declared* `settings_path` (`agents::agent_project_settings_file` → `agent.settings_path`), not from process CWD. The test's `write_claude_agents_config` declares an absolute `settings_path`/`global_settings_path` under the tempdir, so `MIRDAN_AGENTS_CONFIG` redirects BOTH detection AND the Local-scope write into the tempdir. Empirically confirmed: ran all three tests, no `.claude/` appears in `crates/swissarmyhammer-tools/`. The earlier leak came from a prior test revision that omitted `settings_path`; the current revision fixes it. No finding required here.

### Warnings
- [x] `.gitignore:145` — The safety-net ignore `.claude/settings.local.json` is root-anchored (no leading `**/`), so it covers only the repo-root `/.claude/settings.local.json` and does NOT cover nested-crate paths like `crates/swissarmyhammer-tools/.claude/settings.local.json` (verified: `git check-ignore` reports the nested path is NOT ignored). The serve_time_bash_deny test is now correctly isolated, but that isolation depends entirely on every test in the file always declaring an absolute `settings_path` in its agents.yaml; if a future edit omits it, mirdan falls back and a stray `.claude/settings.local.json` could be committed silently from any nested crate. Cheap defense-in-depth: change the pattern to `**/.claude/settings.local.json`, matching the existing `**/.ralph/`, `**/.sah/`, `**/.shell/` convention three lines below it. → FIXED: added `**/.claude/settings.local.json` to `.gitignore`.

### Nits
- [x] `crates/swissarmyhammer-tools/tests/integration/serve_time_bash_deny.rs:30` — `MirdanConfigGuard` is a verbatim copy of the guard in `crates/mirdan/src/install.rs` applier_tests. Acceptable today (separate crates' private test modules, not shareable without exporting a test-support helper), but if a third copy appears, promote it to a shared `#[cfg(any(test, feature = "test-support"))]` helper in mirdan or swissarmyhammer-common rather than pasting again. → ACCEPTED as-is (rule of three; promote on a third copy).