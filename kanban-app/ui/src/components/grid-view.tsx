import {
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { useActiveBoardPath } from "@/lib/command-scope";
import { useGrid } from "@/hooks/use-grid";
import { useSchema } from "@/lib/schema-context";
import { useEntityStore } from "@/lib/entity-store-context";
import { useInspect } from "@/lib/inspect-context";
import { moniker, fieldMoniker } from "@/lib/moniker";
import {
  useEntityCommands,
  buildEntityCommandDefs,
} from "@/lib/entity-commands";
import {
  CommandScopeProvider,
  CommandScopeContext,
  type CommandDef,
} from "@/lib/command-scope";
import { useEntityFocus } from "@/lib/entity-focus-context";
import { DataTable, type DataTableColumn } from "@/components/data-table";
import { Field } from "@/components/fields/field";
import type { ViewDef, Entity, FieldDef } from "@/types/kanban";

/**
 * Renderless component that bridges the grid cursor to entity focus.
 *
 * Must be rendered inside the appropriate CommandScopeProvider so it picks up
 * entity-level commands. Uses two separate effects: one for scope registration
 * and one for focus (fires only when the moniker changes).
 */
function GridFocusBridge({ moniker: mk }: { moniker: string }) {
  const scope = useContext(CommandScopeContext);
  const { setFocus, registerScope, unregisterScope } = useEntityFocus();
  const prevMonikerRef = useRef<string | null>(null);

  // Register scope — fires on any change to keep registry current
  useEffect(() => {
    if (scope) registerScope(mk, scope);
    return () => unregisterScope(mk);
  }, [mk, scope, registerScope, unregisterScope]);

  // Set focus only on cursor movement (moniker change), not on initial mount.
  useEffect(() => {
    if (prevMonikerRef.current !== null && prevMonikerRef.current !== mk) {
      setFocus(mk);
    }
    prevMonikerRef.current = mk;
  }, [mk, setFocus]);

  return null;
}

interface GridViewProps {
  view: ViewDef;
}

export function GridView({ view }: GridViewProps) {
  const boardPath = useActiveBoardPath();
  const boardPathRef = useRef(boardPath);
  boardPathRef.current = boardPath;
  const { getEntities } = useEntityStore();
  const entityType = view.entity_type ?? "task";
  const entities = getEntities(entityType);
  const { getSchema, getEntityCommands } = useSchema();
  const schema = getSchema(entityType);
  const fields = schema?.fields ?? [];
  // Schema-driven entity commands for per-row context menus
  const schemaCommands = getEntityCommands(entityType);

  // Build columns from view's card_fields (or all visible fields)
  const columns = useMemo<DataTableColumn[]>(() => {
    const fieldNames = view.card_fields ?? [];
    if (fieldNames.length === 0) {
      return fields
        .filter((f) => f.section !== "hidden")
        .map((f) => ({ field: f }));
    }
    const fieldMap = new Map<string, FieldDef>();
    for (const f of fields) fieldMap.set(f.name, f);
    return fieldNames
      .map((name) => fieldMap.get(name))
      .filter((f): f is FieldDef => f !== undefined)
      .map((f) => ({ field: f }));
  }, [view.card_fields, fields]);

  // Visible row count may differ from entities.length when groups are collapsed
  const [visibleRowCount, setVisibleRowCount] = useState(entities.length);
  useEffect(() => {
    setVisibleRowCount(entities.length);
  }, [entities.length]);

  const grid = useGrid({ rowCount: visibleRowCount, colCount: columns.length });
  const gridRef = useRef(grid);
  gridRef.current = grid;

  const inspectEntity = useInspect();

  // Current entity and field from cursor position
  const currentEntity =
    grid.cursor.row >= 0 && grid.cursor.row < entities.length
      ? entities[grid.cursor.row]
      : null;
  const currentField =
    grid.cursor.col >= 0 && grid.cursor.col < columns.length
      ? columns[grid.cursor.col].field
      : null;
  const currentEntityMoniker = currentEntity
    ? moniker(entityType, currentEntity.id)
    : null;
  const currentFieldMoniker =
    currentEntity && currentField
      ? fieldMoniker(entityType, currentEntity.id, currentField.name)
      : null;

  // Grid-level commands (navigation, row creation — not entity-specific)
  // Keys are dispatched by the global KeybindingHandler via extractScopeBindings.
  const gridCommands = useMemo<CommandDef[]>(
    () => [
      {
        id: "grid.moveUp",
        name: "Move Up",
        keys: { vim: "k", cua: "ArrowUp" },
        execute: () => gridRef.current.moveUp(),
      },
      {
        id: "grid.moveDown",
        name: "Move Down",
        keys: { vim: "j", cua: "ArrowDown" },
        execute: () => gridRef.current.moveDown(),
      },
      {
        id: "grid.moveLeft",
        name: "Move Left",
        keys: { vim: "h", cua: "ArrowLeft" },
        execute: () => gridRef.current.moveLeft(),
      },
      {
        id: "grid.moveRight",
        name: "Move Right",
        keys: { vim: "l", cua: "ArrowRight" },
        execute: () => gridRef.current.moveRight(),
      },
      {
        id: "grid.moveToRowStart",
        name: "Row Start",
        keys: { vim: "0", cua: "Home" },
        execute: () => gridRef.current.moveToRowStart(),
      },
      {
        id: "grid.moveToRowEnd",
        name: "Row End",
        keys: { vim: "$", cua: "End" },
        execute: () => gridRef.current.moveToRowEnd(),
      },
      {
        id: "grid.edit",
        name: "Edit Cell",
        keys: { vim: "i", cua: "Enter" },
        execute: () => gridRef.current.enterEdit(),
      },
      {
        id: "grid.editEnter",
        name: "Edit Cell (Enter)",
        keys: { vim: "Enter" },
        execute: () => gridRef.current.enterEdit(),
      },
      {
        id: "grid.exitEdit",
        name: "Exit Edit",
        // No keys — field editors handle Escape via onCancel.
        // Escape falls through to app.dismiss.
        execute: () => {
          if (gridRef.current.mode === "edit") gridRef.current.exitEdit();
          else if (gridRef.current.mode === "visual")
            gridRef.current.exitVisual();
        },
      },
      {
        id: "grid.toggleVisual",
        name: "Toggle Visual Mode",
        keys: { vim: "v" },
        execute: () => {
          if (gridRef.current.mode === "visual") gridRef.current.exitVisual();
          else gridRef.current.enterVisual();
        },
      },
      {
        id: "grid.deleteRow",
        name: "Delete Row",
        execute: () => {
          const row = gridRef.current.cursor.row;
          if (row >= 0 && row < entities.length) {
            const entity = entities[row];
            invoke("dispatch_command", {
              cmd: `${entityType}.archive`,
              args: { id: entity.id },
              ...(boardPathRef.current
                ? { boardPath: boardPathRef.current }
                : {}),
            }).catch((err) => console.error("Failed to delete row:", err));
          }
        },
      },
      {
        id: "grid.newBelow",
        name: "New Row Below",
        keys: { vim: "o", cua: "Mod+Enter" },
        execute: () => {
          invoke("dispatch_command", {
            cmd: `${entityType}.add`,
            args: { title: `New ${entityType}` },
            ...(boardPathRef.current
              ? { boardPath: boardPathRef.current }
              : {}),
          }).catch((err) => console.error("Failed to add row:", err));
        },
      },
      {
        id: "grid.newAbove",
        name: "New Row Above",
        keys: { vim: "O", cua: "Mod+Shift+Enter" },
        execute: () => {
          invoke("dispatch_command", {
            cmd: `${entityType}.add`,
            args: { title: `New ${entityType}` },
            ...(boardPathRef.current
              ? { boardPath: boardPathRef.current }
              : {}),
          }).catch((err) => console.error("Failed to add row:", err));
        },
      },
    ],
    [entities, entityType],
  );

  // Entity-level commands (depend on cursor row — registered via focus bridge)
  // Schema-driven: reads entity commands from YAML schema via useEntityCommands.
  const entityCommands = useEntityCommands(
    entityType,
    currentEntity?.id ?? "",
    currentEntity
      ? {
          entity_type: entityType,
          id: currentEntity.id,
          fields: currentEntity.fields ?? {},
        }
      : undefined,
  );

  const handleCellClick = useCallback(
    (row: number, col: number) => {
      grid.setCursor(row, col);
    },
    [grid],
  );

  /**
   * Factory that builds entity-specific context menu commands for a given row.
   *
   * Used by DataTable to wrap each row's selector cell in its own
   * CommandScopeProvider so right-clicking row N always resolves commands for
   * row N's entity — regardless of the grid cursor position at the time of
   * the right-click.
   *
   * Uses buildEntityCommandDefs (non-hook) because this factory is called
   * inside a callback, not in the React render cycle.
   */
  const buildRowEntityCommands = useCallback(
    (entity: Entity): CommandDef[] => {
      return buildEntityCommandDefs(
        schemaCommands,
        entityType,
        entity.id,
        inspectEntity,
        boardPathRef.current,
        entity,
      );
    },
    [schemaCommands, entityType, inspectEntity],
  );

  const renderEditor = useCallback(
    (
      entity: Entity,
      field: FieldDef,
      onCommit: (value: unknown) => void,
      onCancel: () => void,
    ) => {
      return (
        <Field
          fieldDef={field}
          entityType={entity.entity_type}
          entityId={entity.id}
          mode="compact"
          editing={true}
          onDone={() => onCommit(undefined)}
          onCancel={onCancel}
        />
      );
    },
    [],
  );

  return (
    <CommandScopeProvider commands={gridCommands}>
      <CommandScopeProvider commands={entityCommands}>
        <GridFocusBridge moniker={currentFieldMoniker ?? currentEntityMoniker ?? "grid"} />
      </CommandScopeProvider>
      <main className="flex-1 flex flex-col min-h-0">
        <div className="flex items-center px-4 py-1.5 border-b border-border bg-muted/30 text-xs text-muted-foreground gap-3">
          <span>{entities.length} rows</span>
          <span className="text-muted-foreground/50">|</span>
          <span>
            {grid.mode === "edit"
              ? "EDIT"
              : grid.mode === "visual"
                ? "VISUAL"
                : "NORMAL"}
          </span>
          {entities.length > 0 && (
            <>
              <span className="text-muted-foreground/50">|</span>
              <span>
                R{grid.cursor.row + 1}:C{grid.cursor.col + 1}
              </span>
            </>
          )}
        </div>
        <DataTable
          columns={columns}
          rows={entities}
          grid={grid}
          onCellClick={handleCellClick}
          renderEditor={renderEditor}
          onVisibleRowCount={setVisibleRowCount}
          rowEntityCommands={buildRowEntityCommands}
        />
      </main>
    </CommandScopeProvider>
  );
}

