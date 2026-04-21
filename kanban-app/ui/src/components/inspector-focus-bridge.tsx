import { useMemo, useRef } from "react";
import { CommandScopeProvider, type CommandDef } from "@/lib/command-scope";
import type { UseInspectorNavReturn } from "@/hooks/use-inspector-nav";
import type { Entity } from "@/types/kanban";
import { EntityInspector } from "@/components/entity-inspector";
import { useEntityFocus } from "@/lib/entity-focus-context";
import { FocusScope } from "@/components/focus-scope";
import { FocusLayer } from "@/components/focus-layer";
import { useEntityCommands } from "@/lib/entity-commands";

interface InspectorFocusBridgeProps {
  entity: Entity;
}

/**
 * Build inspector navigation commands that broadcast through the focus system.
 *
 * Kept as a factory so the component body stays compact and the
 * command array identity is stable (refs, not render-time values).
 */
function buildInspectorCommands(
  broadcastRef: React.MutableRefObject<(id: string) => void>,
  navRef: React.MutableRefObject<UseInspectorNavReturn | null>,
): CommandDef[] {
  return [
    {
      id: "inspector.moveUp",
      name: "Move Up",
      keys: { vim: "k", cua: "ArrowUp" },
      execute: () => broadcastRef.current("nav.up"),
    },
    {
      id: "inspector.moveDown",
      name: "Move Down",
      keys: { vim: "j", cua: "ArrowDown" },
      execute: () => broadcastRef.current("nav.down"),
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
      execute: () => {
        if (navRef.current?.mode === "edit") navRef.current.exitEdit();
      },
    },
    {
      id: "inspector.moveToFirst",
      name: "Move to First",
      keys: { vim: "g g", cua: "Home" },
      execute: () => broadcastRef.current("nav.first"),
    },
    {
      id: "inspector.moveToLast",
      name: "Move to Last",
      keys: { vim: "G", cua: "End" },
      execute: () => broadcastRef.current("nav.last"),
    },
    {
      id: "inspector.nextField",
      name: "Next Field",
      keys: { cua: "Tab" },
      execute: () => broadcastRef.current("nav.down"),
    },
    {
      id: "inspector.prevField",
      name: "Previous Field",
      keys: { cua: "Shift+Tab" },
      execute: () => broadcastRef.current("nav.up"),
    },
  ];
}

/**
 * Wraps EntityInspector in a FocusLayer + CommandScopeProvider with
 * inspector navigation commands.
 *
 * Navigation between field rows is handled by spatial nav — each row's
 * FocusScope registers its DOM rect with Rust, which resolves directional
 * movement via rect geometry.
 */
export function InspectorFocusBridge({ entity }: InspectorFocusBridgeProps) {
  const navRef = useRef<UseInspectorNavReturn | null>(null);
  const { broadcastNavCommand } = useEntityFocus();
  const broadcastRef = useRef(broadcastNavCommand);
  broadcastRef.current = broadcastNavCommand;

  const entityCommands = useEntityCommands(
    entity.entity_type,
    entity.id,
    entity,
  );
  const commands = useMemo(
    () => buildInspectorCommands(broadcastRef, navRef),
    [],
  );

  return (
    <FocusLayer name="inspector">
      <FocusScope
        moniker={entity.moniker}
        commands={entityCommands}
        showFocusBar={false}
      >
        <CommandScopeProvider commands={commands}>
          <EntityInspector entity={entity} navRef={navRef} />
        </CommandScopeProvider>
      </FocusScope>
    </FocusLayer>
  );
}
