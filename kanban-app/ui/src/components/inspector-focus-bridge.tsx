import { useMemo, useRef } from "react";
import { CommandScopeProvider, type CommandDef } from "@/lib/command-scope";
import type { UseInspectorNavReturn } from "@/hooks/use-inspector-nav";
import type { Entity } from "@/types/kanban";
import { EntityInspector } from "@/components/entity-inspector";
import { useEntityFocus } from "@/lib/entity-focus-context";

interface InspectorFocusBridgeProps {
  entity: Entity;
}

/**
 * Wraps EntityInspector in a CommandScopeProvider with inspector navigation commands.
 *
 * Navigation is pull-based: vim/arrow/tab keys broadcast nav commands (nav.up, nav.down,
 * nav.first, nav.last) via broadcastNavCommand. Each field row's FocusScope uses claimWhen
 * predicates to claim focus when the command matches its position.
 *
 * Edit mode is managed by the inspector nav hook exposed via navRef.
 *
 * On unmount (panel close), restores focus to whatever was focused before (via setFocus).
 *
 * @param entity - The entity to inspect
 */
export function InspectorFocusBridge({ entity }: InspectorFocusBridgeProps) {
  const navRef = useRef<UseInspectorNavReturn | null>(null);
  const { broadcastNavCommand } = useEntityFocus();
  const broadcastRef = useRef(broadcastNavCommand);
  broadcastRef.current = broadcastNavCommand;

  // Commands with keys — resolved by the global KeybindingHandler via scope bindings
  const commands = useMemo<CommandDef[]>(
    () => [
      {
        id: "inspector.moveUp",
        name: "Move Up",
        keys: { vim: "k", cua: "ArrowUp" },
        execute: () => {
          broadcastRef.current("nav.up");
        },
      },
      {
        id: "inspector.moveDown",
        name: "Move Down",
        keys: { vim: "j", cua: "ArrowDown" },
        execute: () => {
          broadcastRef.current("nav.down");
        },
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
        execute: () => {
          broadcastRef.current("nav.first");
        },
      },
      {
        id: "inspector.moveToLast",
        name: "Move to Last",
        keys: { vim: "G", cua: "End" },
        execute: () => {
          broadcastRef.current("nav.last");
        },
      },
      {
        id: "inspector.nextField",
        name: "Next Field",
        keys: { cua: "Tab" },
        execute: () => {
          broadcastRef.current("nav.down");
        },
      },
      {
        id: "inspector.prevField",
        name: "Previous Field",
        keys: { cua: "Shift+Tab" },
        execute: () => {
          broadcastRef.current("nav.up");
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
