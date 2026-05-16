---
assignees:
- claude-code
depends_on:
- 01KRRE967SBZ5TH2JPDMSV21BY
- 01KRRE2K6TTREE37RJEARGRN9K
position_column: todo
position_ordinal: '9180'
project: plugin-arch
title: 'plugin: TypesEmitter — auto-maintained app.d.ts codegen'
---
## What
Implement `TypesEmitter` — the host component that keeps a generated `.d.ts` file in sync with the live server registry, so plugin authors get editor autocomplete with no build step.

In `crates/swissarmyhammer-plugin/src/codegen.rs`:
- `TypesEmitter` subscribes to registry events: server registered → query it via `tools/list`, regenerate; server unregistered → regenerate without it; `notifications/tools/list_changed` → regenerate for that server; plugin load/unload → flush boundary.
- Debounce regeneration ~100ms so a plugin registering many tools during `load()` produces ONE file write. Write atomically (write-then-rename) so language servers never see a half-written file.
- Output: one nested namespace per registered server on an `App` interface. Walk each tool's `Tool` definition:
  - flat tool (no `io.swissarmyhammer/operations` `_meta`) → one method named for the tool, input type from `inputSchema`.
  - operation tool → walk the noun → verb → parameters tree; emit `tool.<noun>.<verb>(input)` per leaf, input type built from that verb's `parameters` map. Emitted shape mirrors the `_meta` tree exactly — no schema inference.
- Output path configurable; default `.swissarmyhammer/types/app.d.ts`. Host only writes it when a dev-mode flag is set; production writes nothing.
- Stale types are safe: types are decoupled from runtime; the emitter does pure metadata → types copying.

## Acceptance Criteria
- [ ] `TypesEmitter` regenerates on register/unregister/`list_changed`/load-unload events, debounced ~100ms, written atomically.
- [ ] A flat tool emits a single typed method; an operation tool emits `<noun>.<verb>(input)` methods mirroring its `_meta` tree.
- [ ] No file is written in production (dev-mode flag off).

## Tests
- [ ] Unit test: feed the emitter a fake registry with one flat tool + one operation tool (operation tool carrying a real `io.swissarmyhammer/operations` `_meta`); assert the emitted `.d.ts` text contains the flat method signature and the nested `noun.verb(input)` signatures with the right parameter object types.
- [ ] Test debounce: register several tools within the window; assert exactly one file write.
- [ ] Test atomic write: the output path never contains a partial file (assert via the write-then-rename path).
- [ ] Run: `cargo test -p swissarmyhammer-plugin` — all green.

## Workflow
- Use `/tdd` — write the emitted-`.d.ts`-content assertions first, then implement.

## Depends on
PluginHost lifecycle (registry events) and the operations `_meta` tree generator.