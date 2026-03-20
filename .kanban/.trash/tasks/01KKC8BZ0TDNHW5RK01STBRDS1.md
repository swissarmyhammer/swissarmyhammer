---
depends_on:
- 01KKC8BB5V6RCWGCQH73QREWF2
position_column: todo
position_ordinal: a3
title: 'STATUSLINE-4: Module framework + Claude modules (all with configurable format)'
---
## What
Implement the module framework (registry, dispatch) and all Claude modules that read data from stdin JSON. Every module has a configurable `format` string like starship — e.g., model default is `🧠 $name`.

Key files:
- `swissarmyhammer-statusline/src/module.rs` (new) — `Segment` struct, `ModuleFn` type, `build_registry()`
- `swissarmyhammer-statusline/src/modules/mod.rs` (new) — re-exports
- `swissarmyhammer-statusline/src/modules/directory.rs` — basename of cwd, format: `$path`
- `swissarmyhammer-statusline/src/modules/model.rs` — short model name, default format: `🧠 $name`
- `swissarmyhammer-statusline/src/modules/context_bar.rs` — progress bar with color thresholds, format: `[$bar] $percentage%`
- `swissarmyhammer-statusline/src/modules/cost.rs` — `$$amount`, hide_zero config
- `swissarmyhammer-statusline/src/modules/session.rs` — first 8 chars, format: `$id`
- `swissarmyhammer-statusline/src/modules/vim_mode.rs` — NORMAL/INSERT, format: `$mode`
- `swissarmyhammer-statusline/src/modules/agent.rs` — agent name, format: `🤖 $name`
- `swissarmyhammer-statusline/src/modules/worktree.rs` — worktree branch, format: `🌲 $branch`
- `swissarmyhammer-statusline/src/modules/version.rs` — Claude version, format: `v$version`

Each module signature: `fn render(input: &StatuslineInput, config: &serde_yaml::Value) -> Option<Segment>`

Per-module format strings use the same `$variable` interpolation as the top-level format. Each module defines which variables it exposes and substitutes them before returning.

## Acceptance Criteria
- [ ] Every module has a configurable `format` field in its YAML section
- [ ] Default formats include emoji/icons where appropriate (🧠, 🤖, 🌲)
- [ ] Each Claude module produces correct output for representative inputs
- [ ] Each Claude module returns `None` when its data is absent
- [ ] context_bar respects bar_width and threshold config

## Tests
- [ ] Unit test for each of the 9 Claude modules with present data
- [ ] Unit test for each Claude module with absent data (returns None)
- [ ] Unit test: custom format string overrides default
- [ ] Unit test: context_bar color thresholds (green <50, yellow <80, red >=80)
- [ ] Unit test: model name shortening (claude-opus-4-6 -> Opus 4.6) with 🧠
- [ ] Unit test: cost formatting (hide_zero, $1.23 renders)
- [ ] `cargo test -p swissarmyhammer-statusline`"