---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffff9f80
title: Add smart `status_date` computed field for tasks
---
## What

A single computed field on `task` that surfaces *the* most salient date at any moment in the task's lifecycle, tagged so the frontend can render the right icon + phrasing. Lives in the **header** section of the task inspector and card.

### Shape (tagged value — option B from discussion)

```json
{ "kind": "completed" | "overdue" | "started" | "scheduled" | "created", "timestamp": "2026-04-12T10:23:00Z" }
```

`updated` is intentionally excluded (it's a "last touched" signal, rarely the most salient). No "due-soon" — binary overdue/not-past only, no arbitrary threshold.

### Priority ladder (first match wins)

| # | kind | Condition |
|---|---|---|
| 1 | `completed` | `completed` field is set |
| 2 | `overdue` | `due` is set AND `due < now()` |
| 3 | `started` | `started` field is set AND not in `completed` |
| 4 | `scheduled` | `scheduled` is set AND `scheduled > now()` |
| 5 | `created` | `created` is set (fallback — always available) |

If none match, the field resolves to `null` and the inspector row hides.

`due` set-but-in-future does NOT drive status; the task's work state (`started`/`scheduled`/`created`) wins. Past-due always escalates.

### Mechanism — centralized aggregator, matches existing derive pattern

No per-field "contribution" metadata. One new derive function reads already-resolved values via `depends_on`, same as `parse-body-progress` reads `body`. The priority ladder is the derive function body — single readable source of truth.

### Files created / modified

1. **NEW** `swissarmyhammer-kanban/builtin/definitions/status_date.yaml`
2. `swissarmyhammer-kanban/builtin/entities/task.yaml` — `status_date` added to the `fields:` list at the END (not between title/tags as originally specified). See implementation note below.
3. `swissarmyhammer-kanban/src/defaults.rs` — added `register_derive_status_date`, `compute_status_date` (pure, testable), and the parsing/formatting helpers; wired into `kanban_compute_engine()`.
4. `swissarmyhammer-kanban/src/context.rs` — field count assertions bumped 18 → 19 and 29/30 → 30/31.
5. **NEW** `kanban-app/ui/src/components/fields/displays/status-date-display.tsx` — display with kind-specific Lucide icons (`CheckCircle`, `AlertTriangle`, `Play`, `Clock`, `PlusCircle`), relative-time phrasing, title tooltip. Exports `parseStatusDateValue` + `parseDateOrDatetime` for reuse by the emptiness predicate.
6. **NEW** `kanban-app/ui/src/components/fields/displays/status-date-empty.ts` — `isStatusDateEmpty` predicate composing the display's narrowing functions; mirrors `progress-empty.ts`.
7. **NEW** `kanban-app/ui/src/components/fields/displays/status-date-empty.test.ts` — 13 unit tests covering null/undefined, primitives, arrays, missing/non-string fields, unknown kinds, unparseable timestamps, and well-formed payloads.
8. **NEW** `kanban-app/ui/src/components/fields/registrations/status-date.tsx` — wires display into the registry WITH `{ isEmpty: isStatusDateEmpty }` so the inspector row is suppressed when the derivation returns null.
9. `kanban-app/ui/src/components/fields/registrations/index.ts` — imports `./status-date`.
10. `kanban-app/ui/src/components/fields/editors/editor-save.test.tsx` — TEST_ENTITY gains a `status_date` value so the entity-field-coverage matrix can render a non-null display.

### Implementation note on field ordering

The card instructed placing `status_date` "first in header ordering (after title, before tags)". This conflicts with how `derive_all` works — it resolves computed fields in template order and mutates the fields map as it goes, so any field that reads another computed field's value must appear AFTER its dependencies in the entity's `fields:` list. `status_date` depends on `completed`/`started`/`created`, which are themselves computed from `_changelog` and positioned late in the task's field list. Placing `status_date` between `title` and `tags` would cause `derive_all` to see its dependencies still unresolved (null), breaking the priority ladder at runtime.

The field is therefore listed LAST in `task.yaml`. Its `section: header` still places it visually in the header group — the UI orders within a section by the `fields:` list position, so `status_date` appears after `title`, `tags`, `progress`. A YAML comment explains the ordering constraint for future editors.

### Non-goals (explicit)

- No change to how the individual date fields (`due`, `scheduled`, `created`, etc.) render or where they live in sections.
- No configurable priority or thresholds — hardcoded priority, no "due-soon" window.
- No changes to `board`/`actor`/`column` entity definitions — tasks only.
- No i18n of relative-time strings.

## Acceptance Criteria

- [x] `builtin/definitions/status_date.yaml` exists and loads via the `include_dir!` registry.
- [x] `kanban_compute_engine()` registers `"derive-status-date"` — `engine.has("derive-status-date")` returns true.
- [x] For a task with `completed = 2026-04-10T00:00:00Z`, `status_date` resolves to `{ kind: "completed", timestamp: "2026-04-10T00:00:00Z" }`.
- [x] For a task with past `due` and no `completed`, `status_date` resolves to `{ kind: "overdue", timestamp: ... }`.
- [x] For a task with no `completed`, no past `due`, `started` set → `{ kind: "started", timestamp: ... }`.
- [x] For a task with only future `scheduled` → `{ kind: "scheduled", timestamp: ... }`.
- [x] For a brand-new task (only `created`) → `{ kind: "created", timestamp: ... }`.
- [x] Future `due` with no `started`/`completed` does NOT produce overdue — falls through to `created`.
- [x] Task inspector renders status_date in the header section with kind-appropriate icon + relative-time label.
- [x] Task card renders status_date in compact mode with icon + short relative phrase.
- [x] `assert_eq!(task_fields.len(), 19)` in `context.rs` is updated and green.

## Tests

- [x] `derive_status_date_prefers_completed` — all dates set, kind is `completed`.
- [x] `derive_status_date_overdue_when_due_past` — past due, kind is `overdue`.
- [x] `derive_status_date_started_over_future_due` — future due + started → `started`.
- [x] `derive_status_date_scheduled_when_future` — created + future scheduled → `scheduled`.
- [x] `derive_status_date_created_fallback` — only created → `created`.
- [x] `derive_status_date_empty_inputs_returns_null` — no dates → `Value::Null`.
- [x] `derive_status_date_due_future_no_started_falls_through` — future due only → `created` (not `overdue`).
- [x] Bonus: `derive_status_date_registered_runs_via_engine` — confirms engine dispatch.
- [x] Bonus: `derive_status_date_resolves_after_its_dependencies_in_task_order` — integration test covering the real `derive_all` pipeline over the builtin task field list.
- [x] `status-date-display.test.tsx` — 14 tests covering all 5 kinds, null/invalid shapes, compact/full modes, and bare `YYYY-MM-DD` timestamps.
- [x] `status-date-empty.test.ts` — 13 tests covering null/undefined, primitives, arrays, missing fields, non-string fields, unknown kinds, unparseable timestamps, valid RFC 3339 datetimes, bare `YYYY-MM-DD` dates, extra fields.
- [x] `cargo nextest run -p swissarmyhammer-kanban derive_status_date` — **9 tests green**.
- [x] `cargo nextest run -p swissarmyhammer-kanban` — **1025 tests, all green**.
- [x] `npx vitest run status-date-display` — **14 tests green**.
- [x] `pnpm test -- status-date` — **27 tests green** (13 empty + 14 display).
- [x] `cargo clippy -p swissarmyhammer-kanban --all-targets` — clean.
- [x] `tsc --noEmit` — clean.

## Workflow

- Followed the TDD order from the card: RED (tests reference an unregistered derivation name) → GREEN (register + YAML + entity + Rust unit tests pass) → frontend display + tests → integration test.

## Review Findings (2026-04-12 19:55)

Prior review findings (2026-04-12 18:24) were invalidated by a transient mid-flight tree — card 3's `SectionDef`/`EntityDef.sections` work was being written to disk concurrently. Those findings have been cleared because:

- `cargo check --all-targets` is now clean (verified).
- `cargo nextest run -p swissarmyhammer-kanban derive_status_date` → 9/9 pass.
- `npx vitest run status-date-display` → 14/14 pass.
- `EntityDef.sections` and `SectionDef` are legitimate landed API from card 3 (01KP24RHG1FARV7J1F4VMAN59F), not scope creep on this card.

One real finding remains against the current tree:

### Warnings

- [x] `kanban-app/ui/src/components/fields/registrations/status-date.tsx:31` — Registration omits the `isEmpty` option. The card's own description states "If none match, the field resolves to `null` and the inspector row hides", and `useVisibleFields` in `entity-inspector.tsx:106-115` only suppresses non-editable rows when the display registers an `isEmpty` predicate (`getDisplayIsEmpty(field.display)`). Without this, a task whose `compute_status_date` returns `Value::Null` (no completed/overdue/started/scheduled/created) will still render an empty inspector row: the `target` icon from `status_date.yaml` + tooltip + flex gap with nothing in the content slot. This is the exact bug pattern that card 01KP23V1 fixed for the `progress` display — mirror the `progress.tsx` solution: extract an `isStatusDateEmpty` helper (or reuse `parseStatusDateValue` with `=== null`) and pass `{ isEmpty: isStatusDateEmpty }` as the third argument to `registerDisplay`. Add a unit test in `status-date-display.test.tsx` (or a sibling `status-date-empty.test.ts`) covering `null`, `undefined`, primitive values, arrays, `{ kind, timestamp }` with unknown kind, and missing fields — mirroring `progress-empty.test.ts`.

## Review Remediation (2026-04-12 — follow-up)

Fixed the warning above:

- Exported `parseStatusDateValue` and `parseDateOrDatetime` from `status-date-display.tsx` so the narrowing rules have a single source of truth.
- Added `kanban-app/ui/src/components/fields/displays/status-date-empty.ts` — `isStatusDateEmpty(value)` delegates to both exported parsers. Returns `true` for null/undefined, primitives, arrays, objects missing or with non-string `kind`/`timestamp`, unknown kinds (not completed/overdue/started/scheduled/created), and objects whose `timestamp` is not a parseable ISO-8601 or bare `YYYY-MM-DD` string. Design mirrors `progress-empty.ts`.
- Updated `kanban-app/ui/src/components/fields/registrations/status-date.tsx` to pass `{ isEmpty: isStatusDateEmpty }` as the third argument to `registerDisplay` — now consistent with `progress.tsx`.
- Added `status-date-empty.test.ts` (13 tests) mirroring `progress-empty.test.ts`, covering every bullet in the warning.
- Verified: `pnpm test -- status-date` → 27/27 pass; full vitest suite → 977/977 pass; `tsc --noEmit` → clean.
