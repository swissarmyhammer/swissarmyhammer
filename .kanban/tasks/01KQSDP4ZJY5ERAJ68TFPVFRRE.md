---
assignees:
- claude-code
depends_on:
- 01KQQSXM2PEYR1WAQ7QXW3B8ME
- 01KQQTXDHP3XBHZ8G40AC4FG4D
- 01KQQTY2HZBX7M9TW95862JVQ3
- 01KQQTYKS63RPM62PPVJ7S7J60
- 01KQQTZ7PSXEQF1WWX14ST8WRT
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffff8780
project: spatial-nav
title: 'Spatial-nav follow-up: collapse FocusZone into FocusScope (single primitive with optional children)'
---
## STATUS: Split into 4 sub-tasks (2026-05-04)

The single-PR collapse was attempted by the implementer and reported as too large for a single execution context — measured at 143 source files affected, with intermediate states that don't compile. Per user direction, this task is now a **planning placeholder**. Execution work is split across four sequential sub-tasks, each landing a coherent compile-clean state:

| Order | Sub-task | ID | Net |
|---|---|---|---|
| A | Kernel only — collapse `FocusZone`/`is_zone` in scope/registry/navigate/lib | `01KQSEA6J8BCE1CAQ1S9XK7TFF` | `cargo test -p swissarmyhammer-focus` standalone green; other crates intentionally break |
| B | Tauri IPC + React adapter — unified `register_scope` surface | `01KQSEC2KJ1K1CVTHYNXGZZG2C` | `cargo build --workspace` clean; adapter typecheck passes |
| C | React component sweep — `<FocusZone>` JSX → `<FocusScope>`; delete `focus-zone.tsx` | `01KQSEDYSJT9J8Y1N8JYX7TQ12` | Component-tree typecheck passes; tests still partial |
| D | Test sweep + README rewrite — ~120 test files, single-primitive prose | `01KQSEFZ8VQ67KFA0B4QE84Z2X` | Full vitest + clippy + tsc clean; final acceptance grep checks pass |

Decisions approved by user (carried into all four sub-tasks):

1. Wire-format `RegisterEntry` discriminator (`kind: "scope"|"zone"`) is dropped (intra-process IPC, no external consumers).
2. `last_focused` moves onto every `FocusScope`, defaulting to `None`.
3. The `scope-not-leaf` validation path (~150 LOC) becomes vacuous and is deleted.
4. `ChildScope`, `FocusEntry`, `ScopeKind`, `BatchRegisterError::KindMismatch` all collapse or disappear.

---

## Original task body (for reference)

(Below kept verbatim from the original filing — the four sub-tasks restate the relevant pieces inline so an implementer reading only their sub-task has full context.)

---

## Reference

Follow-up to the spatial-nav redesign (design `01KQQSXM2PEYR1WAQ7QXW3B8ME`). The geometric cardinal algorithm and drill / first / last operations have landed; the type-level `FocusZone` / `FocusScope` split was kept during the redesign as a deliberate staging choice. This task collapses them into one primitive.

**Decision:** keep `FocusScope` as the single primitive name. Delete `FocusZone` entirely. `FocusScope` already accepts children (its prop type has `children: ReactNode`). The "is this a container or a leaf?" question becomes "does it have registered children?" — answered by querying the registry, not by a type tag.

This is **pure refactor** — no behaviour change. The geometric algorithm doesn't distinguish kind; `showFocusBar={false}` is still a per-instance prop; last-focused memory is still keyed by FQM; drill / first / last still work on the focused scope's children.

(See sub-tasks A/B/C/D for the full file lists and acceptance criteria.)

#spatial-nav-redesign