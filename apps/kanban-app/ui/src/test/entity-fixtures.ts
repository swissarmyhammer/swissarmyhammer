/**
 * Shared entity fixtures for component tests.
 *
 * These builders mint minimal, well-formed `Entity` values so individual
 * test files do not each re-declare the same boilerplate. Keep them tiny
 * and additive — a fixture earns its place here only once a third call
 * site needs it.
 */

import type { Entity } from "@/types/kanban";

/**
 * Create a minimal `actor` entity.
 *
 * @param id - Stable actor id (also used to derive the `actor:<id>` moniker).
 * @param name - Display name stored in `fields.name`.
 * @param overrides - Extra `fields` entries merged after `name` (e.g.
 *   `{ avatar: "data:..." }`). Defaults to `{}` so the common
 *   id-plus-name call site needs no third argument.
 * @returns A fully-formed actor `Entity`.
 */
export function makeActor(
  id: string,
  name: string,
  overrides: Record<string, unknown> = {},
): Entity {
  return {
    entity_type: "actor",
    id,
    moniker: `actor:${id}`,
    fields: {
      name,
      ...overrides,
    },
  };
}
