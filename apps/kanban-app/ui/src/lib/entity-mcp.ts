/**
 * Typed wrappers over the in-process `entity` MCP server.
 *
 * The Rust-side `EntityServer`
 * (`crates/swissarmyhammer-entity-mcp/src/server.rs`) advertises one
 * operation tool named `entity` and dispatches on the `op` verb
 * (`"get entity"`, `"list entities"`, …). These wrappers are the single
 * seam the React tree uses to reach those verbs — components never build a
 * raw `command_tool_call` payload themselves.
 *
 * Verbs are added on demand. Today only `getEntity` is wired (it replaces
 * the legacy `invoke("get_entity", …)` Tauri command); the rest of the
 * entity surface is still driven through the Command service's
 * `execute command` verb by the registered `entity.*` commands.
 */

import { callMcpTool } from "@/lib/mcp-transport";

/** The MCP tool name (and module id) for the entity server. */
export const ENTITY_TOOL = "entity" as const;

/** Verb constant for the entity server's `get entity` op. */
export const GET_ENTITY_OP = "get entity" as const;

/** Envelope shape returned by the `entity` server's `get entity` op. */
interface GetEntityResult {
  ok: boolean;
  entity: Record<string, unknown>;
}

/**
 * Read one entity by `(type, id)` from the in-process `entity` MCP server.
 *
 * Routes `tools/call("entity", { op: "get entity", type, id })` through
 * the generic MCP transport and unwraps the envelope so callers receive
 * the raw entity field bag — behaviorally identical to the legacy
 * `invoke<Record<string, unknown>>("get_entity", { entityType, id })`
 * Tauri command this replaces.
 *
 * @param entityType - The entity type (e.g. `"task"`, `"tag"`).
 * @param id - The entity id within that type.
 * @returns The entity's JSON field bag.
 */
export async function getEntity(
  entityType: string,
  id: string,
): Promise<Record<string, unknown>> {
  const result = await callMcpTool<GetEntityResult | null>(
    ENTITY_TOOL,
    GET_ENTITY_OP,
    { type: entityType, id },
  );
  // Tolerate null/undefined envelopes from test stubs that didn't model
  // the `{ ok, entity }` wrap shape — return an empty bag so the caller
  // can branch on missing fields rather than crash on a null entity.
  return result?.entity ?? {};
}
