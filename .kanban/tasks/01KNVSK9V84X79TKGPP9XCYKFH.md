---
assignees:
- claude-code
depends_on:
- 01KNVSHYPEXZTXFZPEB8W5NPJP
position_column: done
position_ordinal: ffffffffffffffffffffffad80
project: expr-filter
title: 'autocomplete: wire $project mention prefix for filter editor'
---
## What

Make `$` trigger project autocomplete inside the CodeMirror filter editor, reusing the existing schema-driven mention completion infrastructure (the same pipeline that powers `#tag` / `@user` / `^ref` completions).

**Verified facts from the double-check pass:**

- `EntityDef` in `swissarmyhammer-fields/src/types.rs` has both `mention_prefix: Option<String>` and `mention_display_field: Option<FieldName>` as first-class YAML fields.
- `EntityDef::mention_prefix` doc comment requires a **single character** — `$` satisfies this.
- `schema-context.tsx::mentionableTypes` (kanban-app/ui/src/lib/schema-context.tsx) only includes an entity in the list if **both** `mention_prefix` AND `mention_display_field` are set: `if (mention_prefix && mention_display_field) { result.push(...) }`. Missing either one silently drops the entity from autocomplete.
- `use-mention-extensions.ts` already iterates `mentionableTypes` with no hardcoded prefix allowlist. No frontend loop changes are required.
- `search_mentions` in `kanban-app/src/commands.rs` is entity-type agnostic — it looks up the entity's `mention_display_field`, falls back to `"name"`, and returns `{id, display_name, color, avatar}`.
- Project entities already have `name` and `color` fields, so no backend command changes are required.

**Files to modify:**

1. **`swissarmyhammer-kanban/builtin/entities/project.yaml`** — add BOTH required mention fields (both are mandatory per the schema loader's check):
   ```yaml
   mention_prefix: "$"
   mention_display_field: name
   ```
   Leave `search_display_field: name` in place — it's used by a different code path (entity search) and remains needed. Preserve all other existing fields (commands, fields list, icon).

2. **Verification-only reads (no edits expected):**
   - `kanban-app/ui/src/lib/schema-context.tsx` — confirm `mentionableTypes` picks up the project entity once both fields are set. No change should be needed; if it IS needed, describe why in the PR.
   - `kanban-app/ui/src/hooks/use-mention-extensions.ts::buildMentionExtensions` — confirm no hardcoded prefix allowlist rejects `$`. Based on a prior read the loop is fully data-driven. Re-verify and document.
   - `kanban-app/src/commands.rs::search_mentions` — confirm `entityType: "project"` resolves `mention_display_field` from the EntityDef correctly. No change should be needed.

3. **Decoration CSS class** — the helper `getDecoInfra` in `use-mention-extensions.ts` auto-derives `cm-${entityType}-pill` so projects automatically get `cm-project-pill`. Confirm the CSS variable `--project-color` (added in card `01KNVSHYPEXZTXFZPEB8W5NPJP`) is referenced at the expected spot in the pill decoration styles. If there's a shared stylesheet that lists pill classes per entity type (e.g. a `cm-tag-pill { ... }` block), mirror it for `cm-project-pill`.

**Context:**
- The completion source calls `search_mentions` with `{ entityType: "project", query }`. The backend lists project entities, filters by display-field substring (case-insensitive), and returns the first 20 matches.
- `buildColorMap` and `buildMetaMap` in `use-mention-extensions.ts` read per-entity `color` and `description` fields directly from `EntityStoreContext`, so projects get pill colors and tooltips for free once they enter `mentionableTypes`.
- Virtual tag merging logic (`mergeVirtualTagColors` etc.) only applies to the `#` prefix. Projects are not affected.

## Acceptance Criteria

- [ ] `swissarmyhammer-kanban/builtin/entities/project.yaml` declares BOTH `mention_prefix: "$"` AND `mention_display_field: name`
- [ ] After restart, the frontend `SchemaContext.mentionableTypes` includes an entry `{ prefix: "$", entityType: "project", displayField: "name" }`
- [ ] Typing `$` in the perspective filter editor opens the autocomplete popover populated with project names
- [ ] Typing `$auth` narrows the list to projects whose name contains "auth" (case-insensitive)
- [ ] Selecting a project inserts `$<slug>` into the editor (the `slug` is derived from `display_name` via `slugify` in `buildAsyncSearch`)
- [ ] The inserted `$slug` renders as a pill with the project's `color` (not the CSS fallback) when the project has a color set
- [ ] When the project has no color, the pill renders with the `--project-color` CSS fallback
- [ ] Hovering an existing `$slug` pill shows a tooltip with the project's name and description
- [ ] An empty project list does not break the autocomplete or the editor
- [ ] `cargo build --workspace` passes and the app starts without schema-loader errors

## Tests

Add to `kanban-app/ui/src/hooks/use-mention-extensions.test.ts` (check whether this file exists; if not, create it following the pattern used by other hook tests in the same directory):
- [ ] Test that when `mentionableTypes` contains a project entry, `buildMentionExtensions` emits a decoration extension, a completion source, and a tooltip extension for the `$` prefix
- [ ] Mock `invoke("search_mentions", { entityType: "project", query })` to return a fixed list and assert the completion source yields matching entries

Add to `kanban-app/ui/src/lib/schema-context.test.tsx`:
- [ ] Test that a schema for an entity with `mention_prefix: "$"` and `mention_display_field: "name"` populates `mentionableTypes` with a matching entry
- [ ] Test that a schema missing EITHER `mention_prefix` OR `mention_display_field` does NOT appear in `mentionableTypes` (negative case — guards the "both required" rule)

Add to `kanban-app/src/commands.rs` (Rust tests for `search_mentions`):
- [ ] If a test module already exists for `search_mentions`, add a case calling it with `entity_type: "project"` against a fixture board containing two projects, and assert the search returns matches filtered by name

Manual verification:
- [ ] Create two projects (e.g. `auth-migration`, `frontend`), open a perspective filter editor, type `$`, confirm both appear. Type `$a`, confirm only `auth-migration` remains. Select it, confirm a `$auth-migration` pill renders with the project's color.

Test commands:
- [ ] `pnpm test` in `kanban-app/ui/`
- [ ] `cargo test -p kanban-app` (or the appropriate test alias)
- [ ] Manual dev-build check described above

## Workflow
- Use `/tdd` — write the schema-context test first (positive + negative cases), then the mention-extensions test, then edit `project.yaml`. The negative test ensures we don't accidentally regress the "both fields required" invariant. #expr-filter