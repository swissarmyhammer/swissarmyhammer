import { useContext, useMemo } from "react";
import { useSchemaOptional } from "@/lib/schema-context";
import { useInspectOptional } from "@/lib/inspect-context";
import {
  useActiveBoardPath,
  backendDispatch,
  scopeChainFromScope,
  CommandScopeContext,
} from "@/lib/command-scope";
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
 * schema commands and inspect function directly.
 *
 * - `entity.inspect` → calls the provided inspectEntity function
 * - All other commands → dispatched to Rust via `dispatch_command`
 *
 * @param schemaCommands - Entity commands from the YAML schema
 * @param entityType - The entity type name (e.g. "task")
 * @param entityId - The entity ID
 * @param inspectEntity - Callback to open the inspect panel for a moniker
 * @param boardPath - Optional board path for dispatch_command calls
 * @param entity - Optional entity instance for template resolution
 * @param scopeChain - Scope chain monikers for window-scoped dispatch
 * @returns Array of CommandDefs scoped to the given entity
 */
export function buildEntityCommandDefs(
  schemaCommands: readonly EntityCommand[],
  entityType: string,
  entityId: string,
  inspectEntity: (moniker: string) => void,
  boardPath?: string | null,
  entity?: Entity,
  scopeChain?: string[],
): CommandDef[] {
  const entityMoniker = moniker(entityType, entityId);
  return schemaCommands.map((cmd) => ({
    id: cmd.id,
    name: resolveCommandName(cmd.name, entityType, entity),
    target: entityMoniker,
    contextMenu: cmd.context_menu ?? false,
    keys: cmd.keys,
    execute: () => {
      // entity.inspect is the ONE command handled client-side by design:
      // it opens the inspector panel, which is purely a UI concern with no
      // backend state change. All other commands dispatch to Rust via IPC.
      // This is intentional, not a field-special-case — the command's
      // execution mode (client vs backend) is an inherent property of the
      // inspect action. See also focus-scope.tsx which resolves
      // entity.inspect for the double-click gesture.
      if (cmd.id === "ui.inspect" || cmd.id === "entity.inspect") {
        inspectEntity(entityMoniker);
      } else {
        backendDispatch({
          cmd: cmd.id,
          target: entityMoniker,
          ...(boardPath ? { boardPath } : {}),
          scopeChain: scopeChain ?? [],
        }).catch(console.error);
      }
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
 * - `entity.inspect` → calls the inspect function from InspectContext
 * - All other commands → dispatched to Rust via `dispatch_command`
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
 * - `entity.inspect` → calls the inspect function from InspectContext
 * - All other commands → dispatched to Rust via `dispatch_command`
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
  const inspect = useInspectOptional();
  const boardPath = useActiveBoardPath();
  const scope = useContext(CommandScopeContext);
  const scopeChain = useMemo(() => scopeChainFromScope(scope), [scope]);
  const entityMoniker = moniker(entityType, entityId);
  const schemaCommands = getEntityCommands(entityType);

  return useMemo(() => {
    const cmds: CommandDef[] = schemaCommands.map((cmd) => {
      const resolved: CommandDef = {
        id: cmd.id,
        name: resolveCommandName(cmd.name, entityType, entity),
        target: entityMoniker,
        contextMenu: cmd.context_menu ?? false,
        keys: cmd.keys,
        execute: () => {
          if (cmd.id === "ui.inspect" || cmd.id === "entity.inspect") {
            inspect?.(entityMoniker);
          } else {
            backendDispatch({
              cmd: cmd.id,
              target: entityMoniker,
              ...(boardPath ? { boardPath } : {}),
              scopeChain,
            }).catch(console.error);
          }
        },
      };
      return resolved;
    });

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
    inspect,
    boardPath,
    extraCommands,
  ]);
}
