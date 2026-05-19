---
assignees:
- claude-code
position_column: todo
position_ordinal: '8780'
project: plugin-tsonly
title: 'SDK: description as a Plugin class prop + set it on the committed bundles'
---
## What

The user wants a plugin's metadata self-described in its TypeScript: name, version, **and description**. Task 01KS041QGRRAK92050H09MWM7G added `name`/`version` to the `Plugin` base class but not `description`. Add it, and set it on the committed example/builtin bundles so the examples model the full metadata triple.

- `crates/swissarmyhammer-plugin/src/sdk/plugin.ts` — add a `description` field to the `Plugin` abstract base class, exactly mirroring how `name`/`version` were added: an overridable `readonly description: string` with a sensible default (e.g. `""` or `"no description"`). Same doc-comment treatment — it is descriptive metadata only, not used for identity/discovery, never sent to the host.
- The 6 committed bundles — `crates/swissarmyhammer-plugin/examples/plugins/{kanban-tasks,file-notes,cli-echo,multi-module}/index.ts`, `builtin/plugins/kanban-builtin-probe/index.ts`, `test/builtin/plugins/builtin-probe/index.ts` — set a one-line `description` prop on each `Plugin` subclass describing what the plugin does.
- `crates/swissarmyhammer-plugin/examples/plugins/README.md` — update the metadata-props section to list `description` alongside `name`/`version`.
- Extend the `tests/sdk.rs` props test to also assert `description` (override + default).

## Acceptance Criteria
- [ ] `Plugin` exposes `description` with a default; a subclass can override it.
- [ ] Each of the 6 committed bundles sets a meaningful `description`.
- [ ] README documents `description` as a metadata prop.
- [ ] The `sdk.rs` props test covers `description`.

## Tests
- [ ] `cargo nextest run -p swissarmyhammer-plugin --test sdk` — passes (the props test now covers `description`).
- [ ] `cargo nextest run -p swissarmyhammer-plugin` — full crate green (the example e2e tests still load the bundles).
- [ ] `cargo nextest run -p kanban-app` — green.
- [ ] `cargo clippy -p swissarmyhammer-plugin --all-targets -- -D warnings` — clean.

## Workflow
- Use `/tdd` — extend the failing SDK test first, then add the prop and set it on the bundles.