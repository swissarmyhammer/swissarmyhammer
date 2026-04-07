import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useDispatchCommand } from "@/lib/command-scope";
import { useGrid } from "@/hooks/use-grid";
import { useSchema } from "@/lib/schema-context";
import { useEntityStore } from "@/lib/entity-store-context";
import {
  useEntityFocus,
  type ClaimPredicate,
} from "@/lib/entity-focus-context";
import { buildEntityCommandDefs } from "@/lib/entity-commands";
import { CommandScopeProvider, type CommandDef } from "@/lib/command-scope";
import { useActivePerspective } from "@/components/perspective-container";
import { DataTable, type DataTableColumn } from "@/components/data-table";
import { Field } from "@/components/fields/field";
import { fieldMoniker } from "@/lib/moniker";
import type { ViewDef, Entity, FieldDef } from "@/types/kanban";

/**
 * Build navigation claim predicates for a single grid cell.
 *
 * Returns an array of ClaimPredicate entries that let the cell claim focus
 * when adjacent cells are focused and a navigation command fires.
 */
function buildCellPredicates(
  ri: number,
  ci: number,
  cellMonikers: string[][],
  cellMonikerMap: Map<string, { row: number; col: number }>,
  rowCount: number,
  colCount: number,
): ClaimPredicate[] {
  const predicates: ClaimPredicate[] = [];

  if (ri > 0)
    predicates.push({ command: "nav.down", when: (f) => f === cellMonikers[ri - 1][ci] });
  if (ri < rowCount - 1)
    predicates.push({ command: "nav.up", when: (f) => f === cellMonikers[ri + 1][ci] });
  if (ci > 0)
    predicates.push({ command: "nav.right", when: (f) => f === cellMonikers[ri][ci - 1] });
  if (ci < colCount - 1)
    predicates.push({ command: "nav.left", when: (f) => f === cellMonikers[ri][ci + 1] });

  if (ci === 0)
    predicates.push({ command: "nav.rowStart", when: (f) => {
      const pos = f ? cellMonikerMap.get(f) : undefined;
      return pos !== undefined && pos.row === ri && pos.col !== 0;
    }});
  if (ci === colCount - 1)
    predicates.push({ command: "nav.rowEnd", when: (f) => {
      const pos = f ? cellMonikerMap.get(f) : undefined;
      return pos !== undefined && pos.row === ri && pos.col !== colCount - 1;
    }});

  const isGridCell = (mk: string | null) => !!mk && cellMonikerMap.has(mk);
  if (ri === 0 && ci === 0)
    predicates.push({ command: "nav.first", when: (f) => isGridCell(f) && f !== cellMonikers[0][0] });
  if (ri === rowCount - 1 && ci === colCount - 1)
    predicates.push({ command: "nav.last", when: (f) => isGridCell(f) && f !== cellMonikers[rowCount - 1][colCount - 1] });

  return predicates;
}

/**
 * Pattern for valid entity type identifiers.
 * Entity types are schema-defined slugs (e.g. "task", "column") — reject
 * anything that doesn't match to prevent command-injection via crafted views.
 */
const VALID_ENTITY_TYPE = /^[a-z][a-z0-9_-]*$/;

/**
 * Derive grid layout data from entities, schema fields, and the active
 * perspective.  Produces the column definitions, sorted entities, grouping
 * config, cell moniker matrices, and claim predicates consumed by DataTable.
 */
function useGridLayout(view: ViewDef) {
  const { getEntities } = useEntityStore();

  // All hooks must be called unconditionally (React rules of hooks).
  // Use empty-string fallback so hooks always run; we guard before JSX below.
  const rawEntityType = view.entity_type ?? "";
  // Validate entity type against a strict identifier pattern before using it
  // in dispatch template literals or user-visible text.
  const entityType = VALID_ENTITY_TYPE.test(rawEntityType) ? rawEntityType : "";
  const rawEntities = getEntities(entityType);
  const { getSchema, getEntityCommands } = useSchema();
  const schema = getSchema(entityType);
  const fields = schema?.fields ?? [];
  // Schema-driven entity commands for per-row context menus
  const schemaCommands = getEntityCommands(entityType);

  // Sort entities through the active perspective (filtering is server-side).
  const { activePerspective, applySort, groupField } =
    useActivePerspective();
  const entities = useMemo(
    () => applySort(rawEntities),
    [applySort, rawEntities],
  );

  // Derive DataTable grouping from the active perspective's group field.
  const grouping = useMemo<string[] | undefined>(
    () => (groupField ? [groupField] : undefined),
    [groupField],
  );

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

  // --- Pull-based navigation via claimWhen ---
  const { focusedMoniker, setFocus, broadcastNavCommand } = useEntityFocus();

  /**
   * Build a moniker-to-{row,col} lookup from the current grid layout.
   * Used to derive the cursor position from the focused moniker.
   */
  const cellMonikerMap = useMemo(() => {
    const map = new Map<string, { row: number; col: number }>();
    for (let r = 0; r < entities.length; r++) {
      for (let c = 0; c < columns.length; c++) {
        const mk = fieldMoniker(
          entities[r].entity_type,
          entities[r].id,
          columns[c].field.name,
        );
        map.set(mk, { row: r, col: c });
      }
    }
    return map;
  }, [entities, columns]);

  /**
   * Build the flat list of cell monikers in row-major order.
   * cellMonikers[row][col] = moniker string.
   */
  const cellMonikers = useMemo(() => {
    return entities.map((e) =>
      columns.map((col) => fieldMoniker(e.entity_type, e.id, col.field.name)),
    );
  }, [entities, columns]);

  /**
   * Derive the grid cursor position from the currently focused moniker.
   * Returns null if nothing in this grid is focused.
   */
  const derivedCursor = useMemo(() => {
    if (!focusedMoniker) return null;
    return cellMonikerMap.get(focusedMoniker) ?? null;
  }, [focusedMoniker, cellMonikerMap]);

  const grid = useGrid({
    rowCount: visibleRowCount,
    colCount: columns.length,
    cursor: derivedCursor ?? undefined,
  });

  // Focus the first cell on initial mount if no grid cell is focused.
  // Guarded by a ref so it only fires once — without this, entity changes
  // after a cell edit rebuild cellMonikers, which changes firstCellMoniker's
  // reference, re-fires the effect, and snaps the cursor back to (0,0).
  const firstCellMoniker = cellMonikers[0]?.[0] ?? null;
  const hasInitialFocusRef = useRef(false);
  useEffect(() => {
    if (!firstCellMoniker) return;
    if (hasInitialFocusRef.current) return;
    if (!derivedCursor) {
      setFocus(firstCellMoniker);
      hasInitialFocusRef.current = true;
    }
  }, [firstCellMoniker, setFocus, derivedCursor]);

  /**
   * Build claimWhen predicates for each cell in the grid.
   * Returns a 2D array: claimPredicates[row][col] = ClaimPredicate[].
   */
  const claimPredicates = useMemo(() => {
    const rowCount = cellMonikers.length;
    const colCount = columns.length;
    return cellMonikers.map((row, ri) =>
      row.map((_, ci) =>
        buildCellPredicates(ri, ci, cellMonikers, cellMonikerMap, rowCount, colCount),
      ),
    );
  }, [cellMonikers, cellMonikerMap, columns.length]);

  return {
    entityType,
    entities,
    columns,
    grouping,
    visibleRowCount,
    setVisibleRowCount,
    schemaCommands,
    activePerspective,
    grid,
    cellMonikers,
    claimPredicates,
    setFocus,
    broadcastNavCommand,
  };
}

/**
 * Build the CommandDef array for grid-level keyboard commands.
 *
 * Navigation commands broadcast via the pull-based claimWhen system.
 * Editing and row-mutation commands remain push-based.
 */
function useGridCommands(
  broadcastNavCommand: (cmd: string) => void,
  grid: ReturnType<typeof useGrid>,
  entities: Entity[],
  entityType: string,
  dispatch: ReturnType<typeof useDispatchCommand>,
): CommandDef[] {
  // Stable refs so the useMemo closure never re-runs for identity changes
  const broadcastRef = useRef(broadcastNavCommand);
  broadcastRef.current = broadcastNavCommand;
  const gridRef = useRef(grid);
  gridRef.current = grid;

  return useMemo<CommandDef[]>(
    () => [
      {
        id: "grid.moveUp",
        name: "Move Up",
        keys: { vim: "k", cua: "ArrowUp" },
        execute: () => {
          broadcastRef.current("nav.up");
        },
      },
      {
        id: "grid.moveDown",
        name: "Move Down",
        keys: { vim: "j", cua: "ArrowDown" },
        execute: () => {
          broadcastRef.current("nav.down");
        },
      },
      {
        id: "grid.moveLeft",
        name: "Move Left",
        keys: { vim: "h", cua: "ArrowLeft" },
        execute: () => {
          broadcastRef.current("nav.left");
        },
      },
      {
        id: "grid.moveRight",
        name: "Move Right",
        keys: { vim: "l", cua: "ArrowRight" },
        execute: () => {
          broadcastRef.current("nav.right");
        },
      },
      {
        id: "grid.moveToRowStart",
        name: "Row Start",
        keys: { vim: "0", cua: "Home" },
        execute: () => {
          broadcastRef.current("nav.rowStart");
        },
      },
      {
        id: "grid.moveToRowEnd",
        name: "Row End",
        keys: { vim: "$", cua: "End" },
        execute: () => {
          broadcastRef.current("nav.rowEnd");
        },
      },
      {
        id: "grid.firstCell",
        name: "First Cell",
        keys: { cua: "Mod+Home" },
        execute: () => {
          broadcastRef.current("nav.first");
        },
      },
      {
        id: "grid.lastCell",
        name: "Last Cell",
        keys: { vim: "Shift+G", cua: "Mod+End" },
        execute: () => {
          broadcastRef.current("nav.last");
        },
      },
      // nav.first/nav.last -- generic commands from sequence table (gg) and
      // global scope. Grid scope registers these so they resolve here.
      {
        id: "nav.first",
        name: "First Cell",
        execute: () => {
          broadcastRef.current("nav.first");
        },
      },
      {
        id: "nav.last",
        name: "Last Cell",
        execute: () => {
          broadcastRef.current("nav.last");
        },
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
        // No keys -- field editors handle Escape via onCancel.
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
            dispatch(`${entityType}.archive`, {
              args: { id: entity.id },
            }).catch((err) => console.error("Failed to delete row:", err));
          }
        },
      },
      {
        id: "grid.newBelow",
        name: "New Row Below",
        keys: { vim: "o", cua: "Mod+Enter" },
        execute: () => {
          dispatch(`${entityType}.add`, {
            args: { title: `New ${entityType}` },
          }).catch((err) => console.error("Failed to add row:", err));
        },
      },
      {
        id: "grid.newAbove",
        name: "New Row Above",
        keys: { vim: "O", cua: "Mod+Shift+Enter" },
        execute: () => {
          dispatch(`${entityType}.add`, {
            args: { title: `New ${entityType}` },
          }).catch((err) => console.error("Failed to add row:", err));
        },
      },
    ],
    [entities, entityType],
  );
}

interface GridViewProps {
  view: ViewDef;
}

/**
 * Build callback handlers for grid cell interaction and row commands.
 *
 * Returns memoized callbacks for cell clicks, row entity commands, and
 * cell editor rendering.
 */
function useGridCallbacks(
  cellMonikers: string[][],
  setFocus: (mk: string) => void,
  schemaCommands: ReturnType<ReturnType<typeof useSchema>["getEntityCommands"]>,
  entityType: string,
) {
  const handleCellClick = useCallback(
    (row: number, col: number) => {
      // On click, set focus to the clicked cell's moniker
      const mk = cellMonikers[row]?.[col];
      if (mk) setFocus(mk);
    },
    [cellMonikers, setFocus],
  );

  /**
   * Factory that builds entity-specific context menu commands for a given row.
   *
   * Used by DataTable to wrap each row's selector cell in its own
   * CommandScopeProvider so right-clicking row N always resolves commands for
   * row N's entity -- regardless of the grid cursor position at the time of
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
        entity,
      );
    },
    [schemaCommands, entityType],
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

  return { handleCellClick, buildRowEntityCommands, renderEditor };
}

/**
 * Grid (spreadsheet-style) view for entities.
 *
 * Thin orchestrator that delegates layout computation to useGridLayout,
 * keyboard command definitions to useGridCommands, callback construction
 * to useGridCallbacks, and rendering to DataTable.
 */
export function GridView({ view }: GridViewProps) {
  const dispatch = useDispatchCommand();

  const {
    entityType,
    entities,
    columns,
    grouping,
    setVisibleRowCount,
    schemaCommands,
    activePerspective,
    grid,
    cellMonikers,
    claimPredicates,
    setFocus,
    broadcastNavCommand,
  } = useGridLayout(view);

  const gridCommands = useGridCommands(
    broadcastNavCommand,
    grid,
    entities,
    entityType,
    dispatch,
  );

  const { handleCellClick, buildRowEntityCommands, renderEditor } =
    useGridCallbacks(cellMonikers, setFocus, schemaCommands, entityType);

  // Guard: views must declare entity_type. Log a warning so misconfigured
  // views are visible in the unified log, and render an empty state.
  if (!view.entity_type) {
    console.warn(
      `[GridView] view "${view.name ?? view.id}" has no entity_type — cannot render grid without one`,
    );
    return (
      <main className="flex-1 flex items-center justify-center text-muted-foreground text-sm">
        View is missing an entity_type definition.
      </main>
    );
  }

  return (
    <CommandScopeProvider commands={gridCommands}>
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
          cellMonikers={cellMonikers}
          claimPredicates={claimPredicates}
          onCellClick={handleCellClick}
          renderEditor={renderEditor}
          grouping={grouping}
          onVisibleRowCount={setVisibleRowCount}
          rowEntityCommands={buildRowEntityCommands}
          perspectiveSort={activePerspective?.sort}
          perspectiveId={activePerspective?.id}
        />
      </main>
    </CommandScopeProvider>
  );
}
