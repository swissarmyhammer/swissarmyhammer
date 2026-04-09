import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useDispatchCommand, type DispatchOptions } from "@/lib/command-scope";
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
    predicates.push({
      command: "nav.down",
      when: (f) => f === cellMonikers[ri - 1][ci],
    });
  if (ri < rowCount - 1)
    predicates.push({
      command: "nav.up",
      when: (f) => f === cellMonikers[ri + 1][ci],
    });
  if (ci > 0)
    predicates.push({
      command: "nav.right",
      when: (f) => f === cellMonikers[ri][ci - 1],
    });
  if (ci < colCount - 1)
    predicates.push({
      command: "nav.left",
      when: (f) => f === cellMonikers[ri][ci + 1],
    });

  if (ci === 0)
    predicates.push({
      command: "nav.rowStart",
      when: (f) => {
        const pos = f ? cellMonikerMap.get(f) : undefined;
        return pos !== undefined && pos.row === ri && pos.col !== 0;
      },
    });
  if (ci === colCount - 1)
    predicates.push({
      command: "nav.rowEnd",
      when: (f) => {
        const pos = f ? cellMonikerMap.get(f) : undefined;
        return pos !== undefined && pos.row === ri && pos.col !== colCount - 1;
      },
    });

  const isGridCell = (mk: string | null) => !!mk && cellMonikerMap.has(mk);
  if (ri === 0 && ci === 0)
    predicates.push({
      command: "nav.first",
      when: (f) => isGridCell(f) && f !== cellMonikers[0][0],
    });
  if (ri === rowCount - 1 && ci === colCount - 1)
    predicates.push({
      command: "nav.last",
      when: (f) =>
        isGridCell(f) && f !== cellMonikers[rowCount - 1][colCount - 1],
    });

  return predicates;
}

/**
 * Pattern for valid entity type identifiers.
 * Entity types are schema-defined slugs (e.g. "task", "column") — reject
 * anything that doesn't match to prevent command-injection via crafted views.
 */
const VALID_ENTITY_TYPE = /^[a-z][a-z0-9_-]*$/;

/**
 * Derive entity data, schema columns, and perspective sorting/grouping.
 *
 * Returns the raw data needed before navigation monikers can be computed.
 */
function useGridData(view: ViewDef) {
  const { getEntities } = useEntityStore();
  const rawEntityType = view.entity_type ?? "";
  const entityType = VALID_ENTITY_TYPE.test(rawEntityType) ? rawEntityType : "";
  const rawEntities = getEntities(entityType);
  const { getSchema, getEntityCommands } = useSchema();
  const schema = getSchema(entityType);
  const fields = schema?.fields ?? [];
  const schemaCommands = getEntityCommands(entityType);

  const { activePerspective, applySort, groupField } = useActivePerspective();
  const entities = useMemo(
    () => applySort(rawEntities),
    [applySort, rawEntities],
  );
  const grouping = useMemo<string[] | undefined>(
    () => (groupField ? [groupField] : undefined),
    [groupField],
  );

  const columns = useMemo<DataTableColumn[]>(() => {
    const fieldNames = view.card_fields ?? [];
    if (fieldNames.length === 0)
      return fields
        .filter((f) => f.section !== "hidden")
        .map((f) => ({ field: f }));
    const fieldMap = new Map<string, FieldDef>();
    for (const f of fields) fieldMap.set(f.name, f);
    return fieldNames
      .map((name) => fieldMap.get(name))
      .filter((f): f is FieldDef => f !== undefined)
      .map((f) => ({ field: f }));
  }, [view.card_fields, fields]);

  return {
    entityType,
    entities,
    columns,
    grouping,
    schemaCommands,
    activePerspective,
  };
}

/**
 * Build cell moniker matrices, cursor tracking, claim predicates, and grid state.
 *
 * Handles the pull-based navigation system: each cell declares predicates
 * for which nav commands it should claim focus on.
 */
function useGridNavigation(entities: Entity[], columns: DataTableColumn[]) {
  const [visibleRowCount, setVisibleRowCount] = useState(entities.length);
  useEffect(() => {
    setVisibleRowCount(entities.length);
  }, [entities.length]);

  const { focusedMoniker, setFocus, broadcastNavCommand } = useEntityFocus();

  const cellMonikerMap = useMemo(() => {
    const map = new Map<string, { row: number; col: number }>();
    for (let r = 0; r < entities.length; r++) {
      for (let c = 0; c < columns.length; c++) {
        map.set(
          fieldMoniker(
            entities[r].entity_type,
            entities[r].id,
            columns[c].field.name,
          ),
          { row: r, col: c },
        );
      }
    }
    return map;
  }, [entities, columns]);

  const cellMonikers = useMemo(
    () =>
      entities.map((e) =>
        columns.map((col) => fieldMoniker(e.entity_type, e.id, col.field.name)),
      ),
    [entities, columns],
  );

  const derivedCursor = useMemo(() => {
    if (!focusedMoniker) return null;
    return cellMonikerMap.get(focusedMoniker) ?? null;
  }, [focusedMoniker, cellMonikerMap]);

  const grid = useGrid({
    rowCount: visibleRowCount,
    colCount: columns.length,
    cursor: derivedCursor ?? undefined,
  });

  const firstCellMoniker = cellMonikers[0]?.[0] ?? null;
  const hasInitialFocusRef = useRef(false);
  useEffect(() => {
    if (!firstCellMoniker || hasInitialFocusRef.current) return;
    if (!derivedCursor) {
      setFocus(firstCellMoniker);
      hasInitialFocusRef.current = true;
    }
  }, [firstCellMoniker, setFocus, derivedCursor]);

  const claimPredicates = useMemo(() => {
    const rowCount = cellMonikers.length;
    const colCount = columns.length;
    return cellMonikers.map((row, ri) =>
      row.map((_, ci) =>
        buildCellPredicates(
          ri,
          ci,
          cellMonikers,
          cellMonikerMap,
          rowCount,
          colCount,
        ),
      ),
    );
  }, [cellMonikers, cellMonikerMap, columns.length]);

  return {
    setVisibleRowCount,
    grid,
    cellMonikers,
    claimPredicates,
    setFocus,
    broadcastNavCommand,
  };
}

/** Build a navigation command that broadcasts a nav event. */
function navCmd(
  id: string,
  name: string,
  navEvent: string,
  broadcastRef: React.RefObject<(cmd: string) => void>,
  keys?: CommandDef["keys"],
): CommandDef {
  return { id, name, keys, execute: () => broadcastRef.current(navEvent) };
}

/** Build navigation CommandDefs for the grid. */
function buildGridNavCommands(
  broadcastRef: React.RefObject<(cmd: string) => void>,
): CommandDef[] {
  return [
    navCmd("grid.moveUp", "Move Up", "nav.up", broadcastRef, {
      vim: "k",
      cua: "ArrowUp",
    }),
    navCmd("grid.moveDown", "Move Down", "nav.down", broadcastRef, {
      vim: "j",
      cua: "ArrowDown",
    }),
    navCmd("grid.moveLeft", "Move Left", "nav.left", broadcastRef, {
      vim: "h",
      cua: "ArrowLeft",
    }),
    navCmd("grid.moveRight", "Move Right", "nav.right", broadcastRef, {
      vim: "l",
      cua: "ArrowRight",
    }),
    navCmd("grid.moveToRowStart", "Row Start", "nav.rowStart", broadcastRef, {
      vim: "0",
      cua: "Home",
    }),
    navCmd("grid.moveToRowEnd", "Row End", "nav.rowEnd", broadcastRef, {
      vim: "$",
      cua: "End",
    }),
    navCmd("grid.firstCell", "First Cell", "nav.first", broadcastRef, {
      cua: "Mod+Home",
    }),
    navCmd("grid.lastCell", "Last Cell", "nav.last", broadcastRef, {
      vim: "Shift+G",
      cua: "Mod+End",
    }),
    navCmd("nav.first", "First Cell", "nav.first", broadcastRef),
    navCmd("nav.last", "Last Cell", "nav.last", broadcastRef),
  ];
}

/** Build editing and row-mutation CommandDefs for the grid. */
function buildGridEditCommands(
  gridRef: React.RefObject<ReturnType<typeof useGrid>>,
  entities: Entity[],
  entityType: string,
  dispatch: (cmd: string, opts?: DispatchOptions) => Promise<unknown>,
): CommandDef[] {
  return [
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
          dispatch(`${entityType}.archive`, {
            args: { id: entities[row].id },
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
  ];
}

/**
 * Compose the full grid CommandDef array from navigation + editing commands.
 */
function useGridCommands(
  broadcastNavCommand: (cmd: string) => void,
  grid: ReturnType<typeof useGrid>,
  entities: Entity[],
  entityType: string,
  dispatch: (cmd: string, opts?: DispatchOptions) => Promise<unknown>,
): CommandDef[] {
  const broadcastRef = useRef(broadcastNavCommand);
  broadcastRef.current = broadcastNavCommand;
  const gridRef = useRef(grid);
  gridRef.current = grid;

  return useMemo<CommandDef[]>(
    () => [
      ...buildGridNavCommands(broadcastRef),
      ...buildGridEditCommands(gridRef, entities, entityType, dispatch),
    ],
    [entities, entityType],
  );
}

/** Props for the GridView component — the view definition that specifies entity type and columns. */
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
/** Status bar showing row count, grid mode, and cursor position. */
function GridStatusBar({
  rowCount,
  mode,
  cursor,
}: {
  rowCount: number;
  mode: string;
  cursor: { row: number; col: number };
}) {
  const label =
    mode === "edit" ? "EDIT" : mode === "visual" ? "VISUAL" : "NORMAL";
  return (
    <div className="flex items-center px-4 py-1.5 border-b border-border bg-muted/30 text-xs text-muted-foreground gap-3">
      <span>{rowCount} rows</span>
      <span className="text-muted-foreground/50">|</span>
      <span>{label}</span>
      {rowCount > 0 && (
        <>
          <span className="text-muted-foreground/50">|</span>
          <span>
            R{cursor.row + 1}:C{cursor.col + 1}
          </span>
        </>
      )}
    </div>
  );
}

/** Grid (spreadsheet-style) view for entities. */
export function GridView({ view }: GridViewProps) {
  const dispatch = useDispatchCommand();
  const {
    entityType,
    entities,
    columns,
    grouping,
    schemaCommands,
    activePerspective,
  } = useGridData(view);
  const {
    setVisibleRowCount,
    grid,
    cellMonikers,
    claimPredicates,
    setFocus,
    broadcastNavCommand,
  } = useGridNavigation(entities, columns);
  const gridCommands = useGridCommands(
    broadcastNavCommand,
    grid,
    entities,
    entityType,
    dispatch,
  );
  const { handleCellClick, buildRowEntityCommands, renderEditor } =
    useGridCallbacks(cellMonikers, setFocus, schemaCommands, entityType);

  if (!view.entity_type) {
    console.warn(
      `[GridView] view "${view.name ?? view.id}" has no entity_type`,
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
        <GridStatusBar
          rowCount={entities.length}
          mode={grid.mode}
          cursor={grid.cursor}
        />
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
