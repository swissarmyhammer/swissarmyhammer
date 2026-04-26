---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffff9980
title: 'Backend: Add moniker() method to Entity and include moniker in to_json()'
---
## What

The `Entity` struct in `swissarmyhammer-entity/src/entity.rs` has `entity_type` and `id` but no `moniker()` method. The moniker format `"type:id"` is currently constructed ad-hoc by the frontend (`moniker(entity.entity_type, entity.id)`) and parsed by the backend (`parse_moniker()`). The backend should be the single source of truth for moniker construction.

Additionally, entities from the archive or trash should carry a location-qualified moniker so the UI can distinguish them: `"task:01ABC:archive"` or `"task:01ABC:trash"`. Currently `list_archived()` and `read_archived()` return plain `Entity` objects with no indication of origin.

### Files to modify

1. **`swissarmyhammer-entity/src/entity.rs`** — Add a `location` field and `moniker()` method:
   ```rust
   /// Where the entity was loaded from: live storage, archive, or trash.
   #[derive(Debug, Clone, Default, PartialEq, Eq)]
   pub enum EntityLocation { #[default] Live, Archive, Trash }

   pub struct Entity {
       pub entity_type: EntityTypeName,
       pub id: EntityId,
       pub fields: HashMap<String, Value>,
       pub location: EntityLocation,
   }

   impl Entity {
       /// Return the canonical moniker string for this entity.
       /// Live: "type:id", Archive: "type:id:archive", Trash: "type:id:trash"
       pub fn moniker(&self) -> String {
           match self.location {
               EntityLocation::Live => format!("{}:{}", self.entity_type, self.id),
               EntityLocation::Archive => format!("{}:{}:archive", self.entity_type, self.id),
               EntityLocation::Trash => format!("{}:{}:trash", self.entity_type, self.id),
           }
       }
   }
   ```
   Default is `Live` so all existing `Entity::new()` callers are unaffected.

2. **`swissarmyhammer-entity/src/entity.rs`** — Update `to_json()` to include `moniker`:
   ```rust
   map.insert("moniker".into(), Value::String(self.moniker()));
   ```

3. **`swissarmyhammer-entity/src/context.rs`** — In `list_archived()` (line 499) and `read_archived()` (line 518), set `entity.location = EntityLocation::Archive` on each returned entity. Similarly for any trash list/read methods.

4. **`kanban-app/ui/src/types/kanban.ts`** — Add `moniker` to the `Entity` interface:
   ```typescript
   export interface Entity {
     entity_type: string;
     id: string;
     moniker: string;
     fields: Record<string, unknown>;
   }
   ```

### parse_moniker update

`parse_moniker()` in `swissarmyhammer-commands/src/context.rs` (line 163) currently splits on first `:` giving `(type, id)`. It needs to handle the `type:id:location` format — either by stripping the suffix or by returning it as a third component. The simplest approach: keep `parse_moniker()` returning `(type, rest)` where `rest` is `"id:archive"` — the colon in the ID portion is already handled since it splits only on the first colon. But downstream callers that use the ID to look up entities would get `"01ABC:archive"` which won't match storage. This needs a `parse_moniker_parts()` that returns `(type, id, Option<location>)`.

## Acceptance Criteria

- [ ] `EntityLocation` enum with `Live`, `Archive`, `Trash` variants (default `Live`)
- [ ] `Entity::moniker()` returns `"type:id"` for live, `"type:id:archive"` for archived, `"type:id:trash"` for trashed
- [ ] `to_json()` output includes a `moniker` field
- [ ] `list_archived()` / `read_archived()` set `location = Archive` on returned entities
- [ ] TypeScript `Entity` interface has a `moniker` field
- [ ] `cargo test -p swissarmyhammer-entity` passes

## Tests

- [ ] `swissarmyhammer-entity/src/entity.rs` — test `moniker_live` asserting `Entity::new("task", "01ABC").moniker() == "task:01ABC"`
- [ ] `swissarmyhammer-entity/src/entity.rs` — test `moniker_archive` asserting archive-location entity returns `"task:01ABC:archive"`
- [ ] `swissarmyhammer-entity/src/entity.rs` — test `moniker_trash` asserting trash-location entity returns `"task:01ABC:trash"`
- [ ] `swissarmyhammer-entity/src/entity.rs` — update `to_json_includes_id_and_type` to also check `json["moniker"]`
- [ ] Run `cargo test -p swissarmyhammer-entity` — all pass

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass.
