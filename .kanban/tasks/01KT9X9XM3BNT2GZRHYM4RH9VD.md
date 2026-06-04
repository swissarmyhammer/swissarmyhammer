---
assignees:
- claude-code
depends_on:
- 01KT9X89EXYAM6GPM6DV7E5WJ6
position_column: todo
position_ordinal: '8480'
project: claude-hooks
title: 'Loader: read the .claude/settings.json chain into a HookConfig'
---
Add the missing piece: a loader that reads the real Claude Code settings files for a given working directory and produces a `HookConfig` (the existing type in agent-client-protocol-extras). This is what "support reading exact Claude Code hook settings" means.

## Scope
New module in `crates/agent-client-protocol-extras` (e.g. `hook_settings.rs`): `load_hook_config(cwd: &Path) -> HookConfig` (plus a `try_load_hook_config -> Result<HookConfig, _>` if useful).

Precedence chain (lowest → highest):
1. User:    `~/.claude/settings.json`
2. Project: `<cwd>/.claude/settings.json`
3. Local:   `<cwd>/.claude/settings.local.json`

## Path resolution (IMPORTANT — verified)
- Do NOT use `swissarmyhammer-directory` for this — it only manages tool-OWNED dirs (`.swissarmyhammer`, `.avp`, …) via git-root/user-home modes and has NO knowledge of `.claude`.
- The home dir comes from `dirs::home_dir()` (the same crate mirdan uses; cf. `crates/mirdan/src/agents.rs` `expand_tilde` and the `global_settings_path` = `~/.claude/settings.json` / `settings_path` = `.claude/settings.json` conventions). The project paths are resolved relative to the SESSION cwd passed in (sessions carry their cwd — `create_session_with_cwd(request.cwd)`).
- Resolve the project `.claude` relative to the given `cwd` directly. Do NOT implement an ancestor walk-up in v1 (the ACP session cwd already IS the project/workspace dir). Note this as the chosen behavior; a walk-up to the nearest `.claude`-bearing ancestor can be a follow-up if a real need appears.

## Behavior
- Read each file via the shared JSONC reader (dependency task). Missing file → skip. Blank → skip.
- From each file, extract ONLY the top-level `hooks` key (a Map<EventName, Vec<MatcherGroup>>). Ignore all other keys (permissions, env, statusLine, model, etc.).
- Honor `disableAllHooks: true` → that level contributes nothing; document the exact cross-file semantics chosen (suggested: if true in ANY applicable file, hooks are disabled overall, matching Claude's intent).
- MERGE additively across files: for each event name, concatenate the matcher groups from all sources (Claude runs every matching hook from every settings source). Order: user → project → local.
- Malformed `hooks` in one file → log a warning and skip THAT file's hooks; never panic or fail the agent.
- Deserialize into the existing `HookConfig` (forward-compat unknown event kinds are already tolerated and skipped at `build_registrations`).

## Explicitly OUT of scope
- Plugin hooks (`hooks/hooks.json`), managed-policy settings, and skill/agent frontmatter hooks. Only the three settings.json files above.

## Acceptance criteria + tests
- Precedence/merge: a PreToolUse group in user settings AND one in project settings both end up in the resulting HookConfig.
- Only `hooks` is read: a file with `permissions`/`env`/`statusLine` plus `hooks` yields just the hooks; a file with no `hooks` contributes nothing.
- `disableAllHooks: true` → empty (per documented rule).
- JSONC tolerated (comments, trailing commas).
- Missing/blank/malformed files are skipped without error.
- Home resolution uses `dirs::home_dir()`; project resolution uses the passed cwd (hermetically testable via a temp HOME + temp cwd).
- Returned HookConfig round-trips through `build_registrations` (with an evaluator) without error.