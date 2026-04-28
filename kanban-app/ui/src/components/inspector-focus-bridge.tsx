import { useMemo, useRef } from "react";
import { CommandScopeProvider, type CommandDef } from "@/lib/command-scope";
import type { UseInspectorNavReturn } from "@/hooks/use-inspector-nav";
import type { Entity } from "@/types/kanban";
import { EntityInspector } from "@/components/entity-inspector";
import { useFocusActions } from "@/lib/entity-focus-context";
import { FocusScope } from "@/components/focus-scope";
import { asMoniker } from "@/types/spatial";

interface InspectorFocusBridgeProps {
  entity: Entity;
}

/**
 * Wraps EntityInspector in a CommandScopeProvider with inspector navigation commands.
 *
 * Structural focus: each field row is a `<FocusZone>` registered
 * in the spatial-nav graph, and the Rust kernel's unified cascade picks
 * the next focus — within-field nav (e.g. between pills) is iter 0
 * (same-level peers inside the field zone); cross-field nav is iter 1
 * (the cascade escalates to the parent zone and lands on the
 * neighbouring field zone, which the React adapter drills back into).
 * There are no per-row claimWhen predicates.
 *
 * Migration state — vim/arrow/tab nav: the `inspector.move{Up,Down,ToFirst,ToLast}`
 * and tab/shift-tab command handlers below still call `broadcastNavCommand`, but
 * that callback is now a no-op stub on `FocusActions` (it always returns `false`).
 * Those branches exist only to keep the keymap registered while the inspector is
 * rewired; today the only nav inside the inspector is whatever the spatial-nav
 * kernel produces in response to keys handled elsewhere. To restore the
 * documented vim/arrow/tab behaviour, these handlers should call
 * `useSpatialFocusActions().navigate` (with the matching `Direction`) rather
 * than `broadcastNavCommand`.
 *
 * Edit mode is managed by the inspector nav hook exposed via navRef.
 *
 * On unmount (panel close), restores focus to whatever was focused before (via setFocus).
 *
 * @param entity - The entity to inspect
 */
export function InspectorFocusBridge({ entity }: InspectorFocusBridgeProps) {
  const navRef = useRef<UseInspectorNavReturn | null>(null);
  const { broadcastNavCommand } = useFocusActions();
  const broadcastRef = useRef(broadcastNavCommand);
  broadcastRef.current = broadcastNavCommand;

  const entityMoniker = asMoniker(entity.moniker);

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
    <FocusScope moniker={entityMoniker} showFocusBar={false}>
      <CommandScopeProvider commands={commands}>
        <EntityInspector entity={entity} navRef={navRef} />
      </CommandScopeProvider>
    </FocusScope>
  );
}
