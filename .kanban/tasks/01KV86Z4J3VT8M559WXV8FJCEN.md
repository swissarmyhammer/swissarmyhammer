---
assignees:
- claude-code
position_column: todo
position_ordinal: ae80
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