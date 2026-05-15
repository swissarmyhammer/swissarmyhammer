import type { Entity } from "@/types/kanban";

/**
 * Replace an entity in the list by id, or append it if not found (upsert).
 *
 * Used by event handlers to apply entity updates to the store.
 * The upsert behavior prevents silent patch drops when an
 * entity-field-changed event arrives before entity-created.
 */
export function upsertEntity(entities: Entity[], entity: Entity): Entity[] {
  let found = false;
  const next = entities.map((e) => {
    if (e.id !== entity.id) return e;
    found = true;
    return entity;
  });
  if (!found) {
    return [...next, entity];
  }
  return next;
}
