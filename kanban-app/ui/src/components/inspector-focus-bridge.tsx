import { useMemo, useRef } from "react";
import {
  CommandScopeProvider,
  type CommandDef,
} from "@/lib/command-scope";
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
      {
        id: "inspector.pillLeft",
        name: "Pill Left",
        keys: { vim: "h", cua: "ArrowLeft" },
        execute: () => {
          if (navRef.current?.mode === "normal") navRef.current?.movePillLeft();
        },
      },
      {
        id: "inspector.pillRight",
        name: "Pill Right",
        keys: { vim: "l", cua: "ArrowRight" },
        execute: () => {
          if (navRef.current?.mode === "normal")
            navRef.current?.movePillRight();
        },
      },
    ],
    [],
  );

  return (
    <CommandScopeProvider commands={commands}>
      <EntityInspector entity={entity} navRef={navRef} />
    </CommandScopeProvider>
  );
}
