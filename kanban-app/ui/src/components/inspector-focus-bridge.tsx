import { useMemo, useRef } from "react";
import {
  CommandScopeProvider,
  useDispatchCommand,
  type CommandDef,
} from "@/lib/command-scope";
import type { UseInspectorNavReturn } from "@/hooks/use-inspector-nav";
import type { Entity } from "@/types/kanban";
import { EntityInspector } from "@/components/entity-inspector";
import { FocusScope } from "@/components/focus-scope";
import { FocusLayer } from "@/components/focus-layer";
import { useEntityCommands } from "@/lib/entity-commands";

interface InspectorFocusBridgeProps {
  entity: Entity;
}

/** Dispatch signature returned by `useDispatchCommand()` (no bound command). */
type AdHocDispatch = (
  cmd: string,
  opts?: { target?: string; args?: Record<string, unknown> },
) => Promise<unknown>;

/**
 * Build inspector navigation commands that delegate to the unified
 * dispatch pipeline.
 *
 * `inspector.moveUp`/`inspector.moveDown`/`inspector.moveToFirst`/etc.
 * are aliases that dispatch the canonical `nav.*` commands to Rust — no
 * local execute short-circuit. Rust's `NavigateCmd` drives
 * `SpatialState::navigate` for the invoking window and emits
 * `focus-changed`; the React focus store subscribes and re-renders.
 *
 * Kept as a factory so the component body stays compact and the
 * command array identity is stable (refs, not render-time values).
 */
function buildInspectorCommands(
  dispatchRef: React.MutableRefObject<AdHocDispatch>,
  navRef: React.MutableRefObject<UseInspectorNavReturn | null>,
): CommandDef[] {
  const dispatchNav = (id: string) => {
    dispatchRef.current(id).catch((e) => console.error(`${id} failed:`, e));
  };
  return [
    {
      id: "inspector.moveUp",
      name: "Move Up",
      keys: { vim: "k", cua: "ArrowUp" },
      execute: () => dispatchNav("nav.up"),
    },
    {
      id: "inspector.moveDown",
      name: "Move Down",
      keys: { vim: "j", cua: "ArrowDown" },
      execute: () => dispatchNav("nav.down"),
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
      execute: () => dispatchNav("nav.first"),
    },
    {
      id: "inspector.moveToLast",
      name: "Move to Last",
      keys: { vim: "G", cua: "End" },
      execute: () => dispatchNav("nav.last"),
    },
    {
      id: "inspector.nextField",
      name: "Next Field",
      keys: { cua: "Tab" },
      execute: () => dispatchNav("nav.down"),
    },
    {
      id: "inspector.prevField",
      name: "Previous Field",
      keys: { cua: "Shift+Tab" },
      execute: () => dispatchNav("nav.up"),
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
  const dispatch = useDispatchCommand();
  const dispatchRef = useRef(dispatch);
  dispatchRef.current = dispatch;

  const entityCommands = useEntityCommands(
    entity.entity_type,
    entity.id,
    entity,
  );
  const commands = useMemo(
    () => buildInspectorCommands(dispatchRef, navRef),
    [],
  );

  return (
    <FocusLayer name="inspector">
      {/*
       * `spatial={false}` on the entity scope: this outer FocusScope is a
       * container for entity commands and focus claim only — it is NOT a
       * spatial navigation target. Leaving it as `spatial=true` (the
       * default) registers a rect that encloses every field row, and the
       * Rust beam-test can land on that rect when navigating off the
       * last field (or other field-row edges), skipping past the next
       * real field instead of clamping. Mirrors the same pattern used
       * by `DataTableRow` in `data-table.tsx`, where a row container is
       * focus-aware but its cells are the real spatial targets.
       */}
      <FocusScope
        moniker={entity.moniker}
        commands={entityCommands}
        showFocusBar={false}
        spatial={false}
      >
        <CommandScopeProvider commands={commands}>
          <EntityInspector entity={entity} navRef={navRef} />
        </CommandScopeProvider>
      </FocusScope>
    </FocusLayer>
  );
}
