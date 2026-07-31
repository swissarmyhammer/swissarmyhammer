---
assignees:
- claude-code
position_column: todo
position_ordinal: d180
title: crates/mirdan/src/plugin.rs is an orphan file the compiler never sees
---
`crates/mirdan/src/plugin.rs` is not part of any compilation unit. `crates/mirdan/src/lib.rs` never declares `mod plugin`, nothing in the workspace references `mirdan::plugin`, there is no `#[path]` attribute pointing at it, and mirdan declares no extra `[[bin]]` target that could pull it in.

Found while removing profile-based skill selection (^qsr5rdt). The file held a `plugin_catalog()` that built four `PluginSpec` values for packaged Claude Code plugins (sah, kanban, code-context, shelltool), a `build_all` renderer, and its own copy of the `Selector::select` tag-map plumbing. Because it is never compiled, it referenced `Selector::Profile` and `Skill::profiles` long after both were candidates for removal, and `cargo clippy --workspace --all-targets` reported nothing. Dead code the compiler cannot check silently rots.

## Decide and act

Either:

1. **Wire it up** — add `pub mod plugin;` to `crates/mirdan/src/lib.rs`, fix whatever no longer compiles, and give it at least one real-path test (render a plugin into a temp dir, assert the SKILL.md files land). The module docs claim plugin bodies are byte-for-byte identical to the deploy path; nothing proves that today.
2. **Delete it** — if packaged plugins are not a shipping surface, remove the file.

Check the git history first (`git log -- crates/mirdan/src/plugin.rs`) to learn whether it was ever wired and when it fell out. #cleanup #dead-code #cleanup-dead-code