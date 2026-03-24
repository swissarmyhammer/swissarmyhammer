import { useContext, useEffect, useMemo, useRef } from "react";
import {
  CommandScopeContext,
  CommandScopeProvider,
  type CommandDef,
} from "@/lib/command-scope";
import { useEntityFocus } from "@/lib/entity-focus-context";
import type { UseInspectorNavReturn } from "@/hooks/use-inspector-nav";
import type { Entity } from "@/types/kanban";
import { EntityInspector } from "@/components/entity-inspector";

interface InspectorFocusBridgeProps {
  entity: Entity;
}

/**
 * Wraps EntityInspector in a CommandScopeProvider with inspector navigation commands,
 * claims entity focus on mount, and installs a keydown listener for field navigation.
 *
 * On unmount (panel close), restores focus to whatever was focused before.
 *
 * @param entity - The entity to inspect
 */
export function InspectorFocusBridge({ entity }: InspectorFocusBridgeProps) {
  const navRef = useRef<UseInspectorNavReturn | null>(null);

  // Commands with keys — resolved by the global KeybindingHandler via scope bindings
  const commands = useMemo<CommandDef[]>(
    () => [
      {
        id: "inspector.moveUp",
        name: "Move Up",
        keys: { vim: "k", cua: "ArrowUp" },
        execute: () => navRef.current?.moveUp(),
      },
      {
        id: "inspector.moveDown",
        name: "Move Down",
        keys: { vim: "j", cua: "ArrowDown" },
        execute: () => navRef.current?.moveDown(),
      },
      {
        id: "inspector.edit",
        name: "Edit Field",
        keys: { vim: "i", cua: "Enter" },
        execute: () => navRef.current?.enterEdit(),
      },
      {
        id: "inspector.editEnter",
        name: "Edit Field (Enter)",
        keys: { vim: "Enter" },
        execute: () => navRef.current?.enterEdit(),
      },
      {
        id: "inspector.exitEdit",
        name: "Exit Edit",
        // No keys — field editors handle Escape internally via onCancel.
        // Escape falls through to app.dismiss which closes the panel.
        execute: () => {
          if (navRef.current?.mode === "edit") navRef.current.exitEdit();
        },
      },
      {
        id: "inspector.moveToFirst",
        name: "Move to First",
        keys: { vim: "g g", cua: "Home" },
        execute: () => navRef.current?.moveToFirst(),
      },
      {
        id: "inspector.moveToLast",
        name: "Move to Last",
        keys: { vim: "G", cua: "End" },
        execute: () => navRef.current?.moveToLast(),
      },
      {
        id: "inspector.nextField",
        name: "Next Field",
        keys: { cua: "Tab" },
        execute: () => navRef.current?.moveDown(),
      },
      {
        id: "inspector.prevField",
        name: "Previous Field",
        keys: { cua: "Shift+Tab" },
        execute: () => navRef.current?.moveUp(),
      },
    ],
    [],
  );

  return (
    <CommandScopeProvider commands={commands}>
      <InspectorFocusClaimer entity={entity} />
      <EntityInspector entity={entity} navRef={navRef} />
    </CommandScopeProvider>
  );
}

/**
 * Renderless component that claims entity focus for the inspector scope on mount
 * and restores the previous focus on unmount.
 *
 * Must be rendered inside the CommandScopeProvider so it sees the inspector's scope.
 */
function InspectorFocusClaimer({ entity }: { entity: Entity }) {
  const scope = useContext(CommandScopeContext);
  const { focusedMoniker, setFocus, registerScope, unregisterScope } =
    useEntityFocus();
  const prevFocusRef = useRef<string | null>(null);
  const moniker = `inspector:${entity.entity_type}:${entity.id}`;

  useEffect(() => {
    if (!scope) return;

    // Save whatever was focused before we claim it
    prevFocusRef.current = focusedMoniker;

    // Register our scope and claim focus
    registerScope(moniker, scope);
    setFocus(moniker);

    return () => {
      unregisterScope(moniker);
      // Restore previous focus
      setFocus(prevFocusRef.current);
    };
    // Only run on mount/unmount — focusedMoniker is read once at mount time
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [moniker, scope, registerScope, unregisterScope, setFocus]);

  return null;
}
