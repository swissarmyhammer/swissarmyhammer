---
assignees:
- claude-code
position_column: todo
position_ordinal: fb80
title: Cut/copy/paste shown on views & perspectives that don't support them — gate command availability through the plugin system, hide when unsupported
---
## What

Views and perspectives expose cut/copy/paste commands that are not supported for their entity type. Today they show up anyway (and presumably no-op or error). They should NOT be displayed when the focused entity type doesn't support them.

The gating must come from the **plugin system** — i.e. the command's availability/applicability for the focused entity type is declared/resolved by the plugin that owns the command (the same metadata-driven path that decides which commands apply to a scope), NOT a hardcoded React check that special-cases "view" / "perspective".

## Expected

- For a focused entity type that does not support cut/copy/paste (views, perspectives), those commands are simply absent from the command surface (palette / context menu / keybinding availability) — not shown-and-disabled, not shown-and-erroring.
- For entity types that DO support them (tasks/cards), they continue to appear and work.
- Availability is computed from plugin/command metadata for the focused scope, not branched on entity-type strings in the UI.

## Design / investigate

- This is a metadata-driven-UI concern: the UI is an interpreter of command metadata for the focused scope chain, never hardcoding entity logic in React. Reuse the existing command-applicability/scope-resolution path that already decides which commands render for a focused entity (the same machinery behind `{{entity.type}}` caption rendering and per-scope command lists).
- Find where cut/copy/paste commands are declared: likely entity.yaml / a clipboard or edit command YAML, plus the paste path (PasteMatrix — internal vs external drag distinction already exists). Determine why they leak onto view/perspective scopes — are they declared at a too-broad scope, or is the UI listing all commands regardless of applicability?
- The fix is to have the plugin/command declaration express which entity types support cut/copy/paste (capability/applicability), and the command surface filter by that — so unsupported entities never list them.
- Confirm copy/cut/paste are genuinely unsupported for views & perspectives (the user says it's OK that they're unsupported — the ask is purely to stop showing them).

## Acceptance Criteria
- [ ] Cut/copy/paste do not appear on the command surface when a view or perspective is focused
- [ ] Cut/copy/paste still appear and work for entity types that support them (tasks/cards)
- [ ] Availability is resolved from plugin/command metadata for the focused scope — no hardcoded entity-type branch in React
- [ ] No regression to internal-drag (task.move) vs external-drag (paste) dispatch separation

## Tests
- [ ] vitest red-first: with a view/perspective focused, the command surface (palette/menu) does NOT include cut/copy/paste
- [ ] vitest: with a task/card focused, cut/copy/paste ARE present
- [ ] command-metadata/applicability test proving the gate is driven by declared capability, not a UI string check
- [ ] tsc + touched vitest green; relevant cargo nextest packages green if command YAML/declaration changes touch a Rust crate

## Constraints
- NO whole-workspace cargo build/clippy. Never touch .kanban/actors/wballard.jsonl.
- Metadata-driven only — do not hardcode view/perspective special-casing in React (see metadata-driven-ui feedback). Cross-cutting commands live once in entity.yaml/ui.yaml; type-specific commands live in their noun's YAML — gate via applicability, don't re-list.

## Workflow
- /tdd — failing test first: focused view/perspective excludes cut/copy/paste from the command surface.