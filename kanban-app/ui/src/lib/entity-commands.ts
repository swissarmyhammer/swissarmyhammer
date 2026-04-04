import { useMemo } from "react";
import { useSchemaOptional } from "@/lib/schema-context";
import { useDispatchCommand } from "@/lib/command-scope";
import { moniker } from "@/lib/moniker";
import type { CommandDef } from "@/lib/command-scope";
import type { Entity, EntityCommand } from "@/types/kanban";

/**
 * Resolve template variables in a command name.
 *
 * Supported templates:
 * - `{{entity.type}}` — capitalized entity type name ("task" → "Task")
 * - `{{entity.<field>}}` — field value from the entity instance
 *
 * Unknown variables are left as-is when they don't match entity fields;
 * missing fields resolve to an empty string. If no entity is provided,
 * field templates resolve to an empty string.
 *
 * @param template - The command name template string, e.g. "Inspect {{entity.type}}"
 * @param entityType - The entity type name used to resolve `{{entity.type}}`
 * @param entity - Optional entity instance for field value lookups
 * @returns The resolved string with template variables substituted
 */
export function resolveCommandName(
  template: string,
  entityType: string,
  entity?: Entity,
): string {
  return template.replace(/\{\{entity\.(\w+)\}\}/g, (_match, key: string) => {
    if (key === "type") {
      return entityType.charAt(0).toUpperCase() + entityType.slice(1);
    }
    if (entity) {
      const val = entity.fields[key];
      if (typeof val === "string") return val;
    }
    return "";
  });
}

/**
 * Build CommandDef[] from entity schema commands without using hooks.
 *
 * For use in callbacks or factories called outside a React render cycle
 * (e.g. per-row command factories in DataTable). Callers must provide the
 * dispatch function from useDispatchCommand.
 *
 * All commands — including inspect — dispatch to the backend via the
 * standard command system. The backend handles side effects and emits
 * events (e.g. ui-state-changed) that the frontend reacts to.
 *
 * @param schemaCommands - Entity commands from the YAML schema
 * @param entityType - The entity type name (e.g. "task")
 * @param entityId - The entity ID
 * @param dispatch - Dispatch function from useDispatchCommand
 * @param entity - Optional entity instance for template resolution
 * @returns Array of CommandDefs scoped to the given entity
 */
export function buildEntityCommandDefs(
  schemaCommands: readonly EntityCommand[],
  entityType: string,
  entityId: string,
  dispatch: (cmd: string, opts?: { target?: string }) => Promise<unknown>,
  entity?: Entity,
): CommandDef[] {
  const entityMoniker = moniker(entityType, entityId);
  return schemaCommands.map((cmd) => ({
    id: cmd.id,
    name: resolveCommandName(cmd.name, entityType, entity),
    target: entityMoniker,
    contextMenu: cmd.context_menu ?? false,
    keys: cmd.keys,
    execute: () => {
      dispatch(cmd.id, { target: entityMoniker }).catch(console.error);
    },
  }));
}

/**
 * Build CommandDefs from schema commands for any type (entity, perspective, view, etc.).
 *
 * Generic alias for `useEntityCommands` that works for any type string —
 * not just entity types. Reads the type's commands from the YAML-defined schema,
 * resolves name templates, and wires up execute handlers.
 *
 * All commands dispatch to the backend via `useDispatchCommand`.
 *
 * @param type - The type name (e.g. "task", "perspective", "view")
 * @param id - The instance ID
 * @param entity - Optional entity instance for template resolution
 * @param extraCommands - Optional additional commands to append
 * @returns Array of CommandDefs ready to pass to FocusScope or CommandScopeProvider
 */
export function useCommands(
  type: string,
  id: string,
  entity?: Entity,
  extraCommands?: CommandDef[],
): CommandDef[] {
  return useEntityCommands(type, id, entity, extraCommands);
}

/**
 * Build CommandDefs from entity schema commands.
 *
 * Reads the entity type's commands from the YAML-defined schema,
 * resolves name templates, and wires up execute handlers.
 *
 * All commands — including inspect — dispatch to the backend via
 * `useDispatchCommand`. The backend handles side effects (e.g. pushing
 * onto the inspector stack) and emits events that the frontend reacts to.
 *
 * @param entityType - The entity type name (e.g. "task", "column", "board")
 * @param entityId - The entity ID
 * @param entity - Optional entity instance for template resolution
 * @param extraCommands - Optional additional commands to append (e.g. task.untag)
 * @returns Array of CommandDefs ready to pass to FocusScope or CommandScopeProvider
 */
export function useEntityCommands(
  entityType: string,
  entityId: string,
  entity?: Entity,
  extraCommands?: CommandDef[],
): CommandDef[] {
  const { getEntityCommands } = useSchemaOptional();
  const dispatch = useDispatchCommand();
  const entityMoniker = moniker(entityType, entityId);
  const schemaCommands = getEntityCommands(entityType);

  return useMemo(() => {
    const cmds: CommandDef[] = schemaCommands.map((cmd) => ({
      id: cmd.id,
      name: resolveCommandName(cmd.name, entityType, entity),
      target: entityMoniker,
      contextMenu: cmd.context_menu ?? false,
      keys: cmd.keys,
      execute: () => {
        dispatch(cmd.id, { target: entityMoniker }).catch(console.error);
      },
    }));

    if (extraCommands) {
      cmds.push(...extraCommands);
    }

    return cmds;
  }, [
    schemaCommands,
    entityType,
    entityId,
    entity,
    entityMoniker,
    dispatch,
    extraCommands,
  ]);
}
