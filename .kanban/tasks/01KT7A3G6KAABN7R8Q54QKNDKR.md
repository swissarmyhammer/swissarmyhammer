---
assignees:
- claude-code
depends_on:
- 01KT7A301VGSDQ0XYM808Z4C9E
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffde80
project: mirdan-install
title: 'mirdan: Profile manifest + one init/deinit installer'
---
The single source of init/install logic. After card 1, mirdan can depend on skills+templating, so it owns the whole installer.

## Profile manifest (data — the only thing that differs per consumer)
Define a `Profile` in mirdan declaring what a CLI/app installs:
- `mcp_server`: the served tool's MCP registration (name + serve command/args), or none.
- `skills`: selector — `All | Profile(name) | Named(&[&str]) | Single(&str)` (subsumes the per-app skill sets; "profile" here = the existing skill-profile filter, e.g. `kanban`).
- `agents`: which subagents to pack (selector, same shape).
- sah-only flags so sah is "just a bigger profile" not a special case: `statusline`, `preamble` (CLAUDE.md), etc. — make these declarative profile fields, not bespoke code.

## One init/deinit
`mirdan::install::init_profile(profile, scope, root?)` / `deinit_profile(...)` that, in priority order:
1. registers `mcp_server` via the existing `register_mcp_server` applier (strategy-aware),
2. renders the profile's builtin skills with Liquid (`swissarmyhammer-templating` + the partial library) and deploys them via the existing **store+symlink** `deploy_skill_to_agents` (NOT copy-into-.sah/skills),
3. deploys the profile's agents via `deploy_agent_to_agents`,
4. applies statusline/preamble when the profile declares them (reuse `settings::*` / `status::preamble_*`).
- **Explicit-root variant**: accept an optional `root: &Path` so the long-running kanban desktop process never touches CWD (this is the ONLY real reason workspace-init existed). Add root-explicit deploy/store entry points as needed so nothing reads `current_dir()`.
- Rendering: skills stay deployment-free (card 1); mirdan owns render→format→deploy. One renderer (Liquid), no simple-template variant.

## Done when
- `Profile` type + `init_profile`/`deinit_profile` exist in mirdan, fully data-driven (no per-consumer branches).
- Builtin skills render via Liquid and deploy via store+symlink through this path.
- Explicit-root operation works with no CWD access.
- Unit/integration tests: a sample profile installs (skills symlinked, MCP registered, agents deployed) and deinits cleanly; an explicit-root install targets the given root.
- `cargo build --workspace` green; clippy clean.

Depends on the edge inversion (card 1).

## Review Findings (2026-06-03 20:35)

Scope: crates/mirdan/src/install.rs (Profile installer + scope-aware appliers). Build green, clippy clean, all 6 `profile_tests` pass. Implementation is data-driven (single `Selector` code path, no per-consumer branches), reuses existing deploy/store/MCP-register helpers, and the explicit-root path is proven CWD-free by test. Meets every "Done when" criterion.

### Nits
- [x] `crates/mirdan/src/install.rs:2422` and `:4180` — The test-only `MirdanConfigGuard` RAII struct (20 lines: struct + `set` + `Drop`) is verbatim-duplicated between the `applier_tests` and `profile_tests` modules in this same file. RESOLVED: hoisted into a shared `#[cfg(test)] mod test_support` and `use`d from both modules so the env-restore logic stays in lockstep.
- [x] `crates/mirdan/src/install.rs:1283` and `:1294` — When a selector matches nothing, `init_profile` still pushes `InitResult::ok(...)` with an empty `join(", ")`, yielding a trailing-blank message. RESOLVED: empty-`targets` results are now skipped (`Ok(targets) if !targets.is_empty()` / `Ok(_) => {}`).

## Review Findings (2026-06-03 16:10)

Scope: crates/mirdan/src/install.rs (Profile installer). Re-review confirms the two prior nits are resolved (shared `test_support::MirdanConfigGuard`; empty-target results skipped). Build green, clippy clean, all 6 `profile_tests` pass. One blocker found: the documented step-4 statusline/preamble behavior is unimplemented, leaving two `Profile` fields inert.

### Blockers
- [x] `crates/mirdan/src/install.rs:1265` vs `1271-1306` — `init_profile`'s doc comment listed "4. apply the statusline / preamble when the profile sets those flags", but the body never read `profile.statusline`/`profile.preamble`. RESOLVED: implemented step 4. `init_profile` now calls `apply_profile_statusline(true, ..)` when `profile.statusline` and `apply_profile_preamble(true, ..)` when `profile.preamble`; `deinit_profile` mirrors both with `install=false`. Both helpers iterate detected agents and resolve the per-agent settings/instructions file root-aware via a new `resolve_agent_file` (reuses the existing `rooted` so project-scope paths join `root`, user-scope uses the absolute global path — proven CWD-free by test). Statusline applies through `settings::set_object`/`remove_key` with the conventional `{type:"command", command:"sah statusline"}` block; preamble applies through ported `ensure_preamble`/`remove_preamble` helpers that delegate the present-check to `status::preamble_present_in` (single source of truth, in lockstep with `mirdan status`). New `#[serial]` test `init_profile_statusline_and_preamble_install_and_deinit` sets both flags with an explicit `root`, asserts the `statusLine` block + CLAUDE.md preamble are written and that nothing leaks into CWD, then asserts both are removed on deinit. `cargo build --workspace` green, `cargo clippy -p mirdan --all-targets` clean, all 381 mirdan tests pass (7 `profile_tests`).

### Nits
- [x] `crates/mirdan/src/install.rs:985-991` and `1068-1074` — The "render `{{`/`{%`-bearing metadata values in place" loop is duplicated between `render_profile_skill` and `install_profile_agents`. Only two occurrences (below the rule-of-three threshold), but if step-4/preamble rendering adds a third metadata-render site, extract a `render_metadata_in_place(&library, &ctx, &mut metadata)` helper then. DEFERRED (deliberate, below rule-of-three): re-confirmed across the 16:44 and 21:55 review rounds — step 4 added no third metadata-render site, so it stays at exactly two occurrences. The finding's own guidance is "extract when a third site appears"; no action is warranted now. Carried forward as a standing note for whichever later card adds a third metadata-render site.

## Review Findings (2026-06-03 16:44)

Scope: crates/mirdan/src/install.rs (Profile installer, focus on the step-4 statusline/preamble round). Re-verified: `cargo clippy -p mirdan --all-targets` clean, all 7 `install::profile_tests` pass. The prior blocker (step 4) is genuinely resolved — install/deinit are symmetric via a single `install: bool` flag, create-on-install/no-op-on-missing semantics are correct, and the explicit-root test proves no CWD leak. `init_profile`/`deinit_profile` having no external callers is expected: they are the public API the three downstream `blocks` cards will consume, exported via `pub mod install`. The metadata-render nit from the prior round is still open (carried forward, not re-flagged). Two new observations below.

### Warnings
- [x] `crates/mirdan/src/install.rs:1170`, `:1241`, `:1428`, `:1481` — `apply_profile_statusline`, `apply_profile_preamble`, `register_mcp_server_at`, and `unregister_mcp_server_at` now share a verbatim detect→`scope_is_global`→loop-agents→resolve-per-agent-path→apply→`changed += 1`→emit Action/Warning→aggregate-single-`InitResult::ok` skeleton (4 sites, above the rule-of-three threshold). RESOLVED: extracted `for_each_detected_agent(scope, reporter, apply, summary)` which owns load-agents/short-circuit, `scope_is_global`, the per-agent loop, Action/Warning emission, the changed count, and the single-`InitResult` aggregate. Each applier's closure returns `Result<Option<AgentAction>, RegistryError>` (a small `{verb, message}` struct so per-agent verbs vary), and a `summary(changed)` closure builds the component-specific `InitResult`. All four appliers now contain only their resolve+apply body. Net ~80 lines of structural duplication removed.

### Nits
- [x] `crates/mirdan/src/install.rs:1446-1453` and `:1498-1505` — The per-agent MCP config-path resolution block is verbatim-identical between `register_mcp_server_at` and `unregister_mcp_server_at`. RESOLVED (refactor landed): extracted `resolve_agent_mcp_config(agent, global, root) -> Option<(&McpConfigDef, PathBuf)>`; both `register_mcp_server_at` and `unregister_mcp_server_at` now call it inside their `for_each_detected_agent` closures.

## Review Findings (2026-06-03 21:55)

Scope: crates/mirdan/src/install.rs (Profile installer — full re-review). Verified: `cargo clippy -p mirdan --all-targets` clean, all 7 `install::profile_tests` pass. Every "Done when" criterion is met. One new low-priority observation.

### Nits
- [x] `crates/mirdan/src/install.rs:1581-1584` and `:1596-1599` — `deinit_profile` always pushes `InitResult::ok("profile-skills", "Removed N skill(s)")` / `"Removed N agent(s)"` even when the selector resolves to zero names (N=0), whereas `init_profile` deliberately skips empty results. RESOLVED: both `deinit_profile` skill/agent result pushes are now guarded on `!names.is_empty()`, mirroring the init path — symmetric with init. Clippy clean, all 7 `profile_tests` pass.