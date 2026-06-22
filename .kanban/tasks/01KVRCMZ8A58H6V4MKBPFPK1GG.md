---
assignees:
- claude-code
position_column: todo
position_ordinal: fb80
project: pill-via-cm6
title: Project mention pills don't render (mention_slug_field "id" reads empty from fields bag)
---
## What

**Bug**: A task's `project` field value renders as raw text (e.g. `$task-card-fields`) instead of a colored project pill, on cards and in the inspector. Task mentions (`^short`), tag mentions (`#tag`), and actor mentions (`@actor`) all render correctly — only project (`$`) pills are broken.

**Root cause** — the project entity type declares `mention_slug_field: id` in `crates/swissarmyhammer-kanban/builtin/entities/project.yaml`, so the frontend reads the mention slug from the entity field named `id`. But `entityFromBag` in `apps/kanban-app/ui/src/types/kanban.ts:431` destructures `{ entity_type, id, moniker, ...fields }` — it lifts `id` OUT of the `fields` bag onto the top-level `Entity.id` property. So `getStr(entity, "id")` reads `entity.fields["id"]`, which is `undefined`, and returns `""`.

Consequences of the empty slug:
1. `buildMentionMetaMap` (`apps/kanban-app/ui/src/hooks/use-mention-extensions.ts:105`): `const slug = getStr(e, slugField)` → `""` → `if (!slug) continue;` skips every project, so the decoration/widget metaMap has no project entries at all.
2. `mentionSlugFor` (`apps/kanban-app/ui/src/components/mention-view.tsx:188`): `getStr(entity, slugField)` → `""` → falls back to the raw field value, so the CM6 doc becomes `$<project-id>` but there is no matching metaMap key → `findMentionsInText` finds no hit → renders as plain text, no pill.
3. `slugMatchesEntity` (`apps/kanban-app/ui/src/components/mention-view.tsx:165`): `getStr(entity, config.slugField)` → `""`, so slug-based resolution of projects also fails.

Tasks are unaffected because their `mention_slug_field` is `short_id` — a regular enriched field that stays inside the `fields` bag. Only `id` (the one key `entityFromBag` removes) is broken, which is exactly the field projects key on.

### Fix approach

Add a small accessor that resolves a slug-field name against an entity including the top-level identity columns, and use it at the three slug-field read sites. In `apps/kanban-app/ui/src/types/kanban.ts`, beside `getStr`, add:

```ts
/** Read a field that may be a top-level identity column (id/entity_type/moniker) rather than a bag field. */
export function getEntityField(entity: Entity, field: string): string {
  if (field === "id") return entity.id;
  if (field === "entity_type") return entity.entity_type;
  if (field === "moniker") return entity.moniker;
  return getStr(entity, field);
}
```

Then replace `getStr(entity, slugField)` / `getStr(e, slugField)` with `getEntityField(...)` at:
- `apps/kanban-app/ui/src/hooks/use-mention-extensions.ts:105` (`buildMentionMetaMap`)
- `apps/kanban-app/ui/src/components/mention-view.tsx:188` (`mentionSlugFor`)
- `apps/kanban-app/ui/src/components/mention-view.tsx:165` (`slugMatchesEntity`)

Leave the non-slug (`displayField`) reads on `getStr` — those are real bag fields.

### Out of scope
- Changing `entityFromBag` to keep `id` in `fields` (wider blast radius — `id` would then appear in field iteration everywhere; the targeted accessor is safer).
- The pill *label*: with this fix the project pill labels with the project id slug (`$task-card-fields`), matching the `displayName = slug` contract for slug-field types. Relabeling project pills to the project `name` is a separate concern — do NOT bundle it here.

## Acceptance Criteria
- [ ] A task whose `project` field is set renders a colored `$`-prefixed project pill (a CM6 replace widget), not raw `$<slug>` text, in `BadgeDisplay`.
- [ ] `buildMentionMetaMap` returns a non-empty map for a project entity list (keyed by each project's id) when the type's `slugField` is `"id"`.
- [ ] `getEntityField(entity, "id")` returns the top-level `Entity.id`; `getEntityField(entity, someBagField)` matches `getStr` behavior.
- [ ] Task / tag / actor pills are unchanged (no regression).

## Tests
- [ ] In `apps/kanban-app/ui/src/hooks/__tests__/use-mention-extensions.test.ts`, add a `buildMentionMetaMap` case with `slugField: "id"` and a project-shaped entity (top-level `id`, `fields: { name, color }`); assert the returned map has a key equal to the entity's `id` with `displayName === id` and `description === name`. This fails before the fix (map is empty) and passes after.
- [ ] In `apps/kanban-app/ui/src/components/mention-view.test.tsx`, add a test rendering `<MentionView entityType="project" id="<projId>" />` with a project entity in the mock store and a schema where project declares `prefix: "$"`, `displayField: "name"`, `slugField: "id"`; assert the rendered output contains a mention pill (`.cm-mention-pill` or `.cm-project-pill`) rather than plain `$<projId>` text. Fails before, passes after.
- [ ] Run: `cd apps/kanban-app/ui && npm test -- use-mention-extensions mention-view` — all pass.
- [ ] Run the full UI suite: `cd apps/kanban-app/ui && npm test` — no regressions.

## Workflow
- Use `/tdd` — write the two new tests first (they fail against the current empty-slug behavior), then add `getEntityField` and swap the three slug-field read sites so they pass.