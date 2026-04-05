---
assignees:
- claude-code
depends_on:
- 01KND4JJV437GTZ7QFQ3JBM2F1
- 01KNESGZ1B8G4JKAB1231YTM9J
position_column: done
position_ordinal: ffffffffffffffffda80
title: 'VT-4: Populate virtual_tags and filter_tags in enrichment pipeline'
---
## What

Wire virtual tags into the task enrichment pipeline as two separate fields — `virtual_tags` (display) and `filter_tags` (union for filtering). The existing `tags` field stays untouched (body-parsed only, editable).

**Key principle**: `tags` is body-parsed and editable. `virtual_tags` is computed from strategies and display-only. `filter_tags = tags ∪ virtual_tags` and is hidden — used only for backend filtering.

### Enrichment pipeline changes

**Files to modify:**
- `swissarmyhammer-kanban/src/task_helpers.rs` — in `enrich_all_task_entities()`:
  1. Accept a `&VirtualTagRegistry` parameter
  2. After computing ready/blocked/blocks, evaluate virtual tags for each task
  3. Set `virtual_tags` field on entity with matching virtual tag slugs
  4. Read `tags` (body-parsed) and `virtual_tags`, compute union, set `filter_tags`
- `swissarmyhammer-kanban/src/task_helpers.rs` — also update `enrich_task_entity()` (single-task variant)
- All callers of `enrich_all_task_entities` — pass the registry (main caller: `kanban-app/src/commands.rs:226`)

### Migrate filtering to EntityFilterContext

Replace ad-hoc inline `.filter()` closures with `EntityContext::list_where()` from VT-0.

**Files to modify:**
- `swissarmyhammer-kanban/src/task/list.rs` — migrate tag filtering to use `list_where` with `EntityFilterContext`, reading `filter_tags` instead of `task_tags(t)`
- `swissarmyhammer-kanban/src/task/next.rs` — same migration for tag filtering

The kanban layer injects its `VirtualTagRegistry` and terminal column ID into `EntityFilterContext` via `build_ctx`. The predicate reads `filter_tags` for tag matching — this means `list tasks --tag BLOCKED` works transparently.

**No changes to `ParseBodyTags`** — the derive handler stays pure (body-only). Virtual tags are injected at the enrichment layer, not the derive layer.

## Acceptance Criteria
- [ ] `enrich_all_task_entities` accepts and uses a `VirtualTagRegistry`
- [ ] `virtual_tags` field set on each entity with matching virtual tag slugs
- [ ] `filter_tags` field set as union of `tags` + `virtual_tags`
- [ ] `tags` field is NOT modified (still body-parsed only)
- [ ] `list tasks --tag BLOCKED` finds tasks with the BLOCKED virtual tag via `filter_tags`
- [ ] `next task --tag READY` finds ready tasks via `filter_tags`
- [ ] Filtering uses `EntityContext::list_where` with `EntityFilterContext`, not inline closures
- [ ] All callers pass the registry

## Tests
- [ ] Unit test: enrich with mock strategy, verify `virtual_tags` field contains strategy slug
- [ ] Unit test: enrich, verify `filter_tags` is union of `tags` and `virtual_tags`
- [ ] Unit test: enrich, verify `tags` field unchanged (no virtual slugs)
- [ ] Integration test: `list tasks --tag BLOCKED` returns blocked task
- [ ] Integration test: `list tasks --tag bug` still works (body-parsed tag)
- [ ] `cargo nextest run -p swissarmyhammer-kanban` passes

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

#virtual-tags