import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useDispatchCommand, type DispatchOptions } from "@/lib/command-scope";
import { useGrid } from "@/hooks/use-grid";
import { useSchema } from "@/lib/schema-context";
import { useEntityStore } from "@/lib/entity-store-context";
import { useEntityFocus, useFocusedMoniker } from "@/lib/entity-focus-context";
import { buildEntityCommandDefs } from "@/lib/entity-commands";
import { CommandScopeProvider, type CommandDef } from "@/lib/command-scope";
import { useActivePerspective } from "@/components/perspective-container";
import { DataTable, type DataTableColumn } from "@/components/data-table";
import { Field } from "@/components/fields/field";
import { fieldMoniker } from "@/lib/moniker";
import type { ViewDef, Entity, FieldDef } from "@/types/kanban";

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
 * Build the cell moniker matrix and reverse-lookup map for the grid.
 *
 * The matrix is [row][col] of moniker strings. The map is moniker -> {row, col}
 * for deriving the cursor from the focused moniker.
 */
function useCellMonikers(entities: Entity[], columns: DataTableColumn[]) {
  const cellMonikerMap = useMemo(() => {
    const map = new Map<string, { row: number; col: number }>();
    for (let r = 0; r < entities.length; r++) {
      for (let c = 0; c < columns.length; c++) {
        map.set(
          fieldMoniker(entities[r].entity_type, entities[r].id, columns[c].field.name),
          { row: r, col: c },
        );
      }
    }
    return map;
  }, [entities, columns]);

  const cellMonikers = useMemo(
    () => entities.map((e) =>
      columns.map((col) => fieldMoniker(e.entity_type, e.id, col.field.name)),
    ),
    [entities, columns],
  );

  return { cellMonikerMap, cellMonikers };
}

/**
 * Set focus to the first grid cell once on mount, if nothing else is focused.
 *
 * No-ops after the initial seed so manual focus changes are preserved.
 */
function useGridInitialFocus(
  firstCellMoniker: string | null,
  derivedCursor: { row: number; col: number } | null,
  setFocus: (mk: string) => void,
) {
  const hasInitialFocusRef = useRef(false);
  useEffect(() => {
    if (!firstCellMoniker || hasInitialFocusRef.current) return;
    if (!derivedCursor) {
      setFocus(firstCellMoniker);
      hasInitialFocusRef.current = true;
    }
  }, [firstCellMoniker, setFocus, derivedCursor]);
}

/**
 * Build cell moniker matrices, cursor tracking, and grid state.
 *
 * Spatial navigation handles all directional movement via DOM rects in Rust,
 * so no manual claim predicates are needed for the grid.
 */
function useGridNavigation(entities: Entity[], columns: DataTableColumn[]) {
  const [visibleRowCount, setVisibleRowCount] = useState(entities.length);
  useEffect(() => { setVisibleRowCount(entities.length); }, [entities.length]);

  const { setFocus, broadcastNavCommand } = useEntityFocus();
  const focusedMoniker = useFocusedMoniker();
  const { cellMonikerMap, cellMonikers } = useCellMonikers(entities, columns);

  const derivedCursor = useMemo(() => {
    if (!focusedMoniker) return null;
    return cellMonikerMap.get(focusedMoniker) ?? null;
  }, [focusedMoniker, cellMonikerMap]);

  const grid = useGrid({
    rowCount: visibleRowCount,
    colCount: columns.length,
    cursor: derivedCursor ?? undefined,
  });

  useGridInitialFocus(cellMonikers[0]?.[0] ?? null, derivedCursor, setFocus);

  return { setVisibleRowCount, grid, cellMonikers, setFocus, broadcastNavCommand };
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

/**
 * Build the execute callback for a grid editing command.
 *
 * Each command id maps to a specific grid or dispatch action. Row-mutation
 * commands (`deleteRow`, `newBelow`, `newAbove`) dispatch to the backend.
 */
function gridEditExecutor(
  id: string,
  gridRef: React.RefObject<ReturnType<typeof useGrid>>,
  entities: Entity[],
  entityType: string,
  dispatch: (cmd: string, opts?: DispatchOptions) => Promise<unknown>,
): () => void {
  const g = () => gridRef.current;
  const addRow = () =>
    dispatch(`${entityType}.add`, { args: { title: `New ${entityType}` } })
      .catch((err) => console.error("Failed to add row:", err));
  switch (id) {
    case "grid.edit":
    case "grid.editEnter":
      return () => g().enterEdit();
    case "grid.exitEdit":
      return () => { if (g().mode === "edit") g().exitEdit(); else if (g().mode === "visual") g().exitVisual(); };
    case "grid.toggleVisual":
      return () => { if (g().mode === "visual") g().exitVisual(); else g().enterVisual(); };
    case "grid.deleteRow":
      return () => {
        const row = g().cursor.row;
        if (row >= 0 && row < entities.length)
          dispatch(`${entityType}.archive`, { args: { id: entities[row].id } })
            .catch((err) => console.error("Failed to delete row:", err));
      };
    case "grid.newBelow":
    case "grid.newAbove":
      return addRow;
    default:
      return () => {};
  }
}

/** Descriptor table for grid editing commands: id, name, optional key bindings. */
const GRID_EDIT_DESCRIPTORS: [string, string, CommandDef["keys"]?][] = [
  ["grid.edit",         "Edit Cell",           { vim: "i", cua: "Enter" }],
  ["grid.editEnter",    "Edit Cell (Enter)",   { vim: "Enter" }],
  ["grid.exitEdit",     "Exit Edit"],
  ["grid.toggleVisual", "Toggle Visual Mode",  { vim: "v" }],
  ["grid.deleteRow",    "Delete Row"],
  ["grid.newBelow",     "New Row Below",       { vim: "o", cua: "Mod+Enter" }],
  ["grid.newAbove",     "New Row Above",       { vim: "O", cua: "Mod+Shift+Enter" }],
];

/** Build editing and row-mutation CommandDefs for the grid. */
function buildGridEditCommands(
  gridRef: React.RefObject<ReturnType<typeof useGrid>>,
  entities: Entity[],
  entityType: string,
  dispatch: (cmd: string, opts?: DispatchOptions) => Promise<unknown>,
): CommandDef[] {
  return GRID_EDIT_DESCRIPTORS.map(([id, name, keys]) => ({
    id, name, keys,
    execute: gridEditExecutor(id, gridRef, entities, entityType, dispatch),
  }));
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
 * Render a Field in inline-edit mode for a grid cell.
 *
 * Pure helper (no hooks) so it can be wrapped in a stable useCallback.
 */
function renderCellEditor(
  entity: Entity,
  field: FieldDef,
  onCommit: (value: unknown) => void,
  onCancel: () => void,
): React.ReactNode {
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
      const mk = cellMonikers[row]?.[col];
      if (mk) setFocus(mk);
    },
    [cellMonikers, setFocus],
  );

  /**
   * Factory that builds entity-specific context menu commands for a given row.
   *
   * Uses buildEntityCommandDefs (non-hook) because this factory is called
   * inside a callback, not in the React render cycle.
   */
  const buildRowEntityCommands = useCallback(
    (entity: Entity): CommandDef[] =>
      buildEntityCommandDefs(schemaCommands, entityType, entity.id, entity),
    [schemaCommands, entityType],
  );

  const renderEditor = useCallback(renderCellEditor, []);

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

/**
 * Compose all grid hooks into a single state bundle for GridView.
 *
 * Centralizes hook orchestration so the component body stays under the
 * line limit while keeping every hook call at the top level.
 */
function useGridViewState(view: ViewDef) {
  const dispatch = useDispatchCommand();
  const { entityType, entities, columns, grouping, schemaCommands, activePerspective } =
    useGridData(view);
  const { setVisibleRowCount, grid, cellMonikers, setFocus, broadcastNavCommand } =
    useGridNavigation(entities, columns);
  const gridCommands = useGridCommands(broadcastNavCommand, grid, entities, entityType, dispatch);
  const { handleCellClick, buildRowEntityCommands, renderEditor } =
    useGridCallbacks(cellMonikers, setFocus, schemaCommands, entityType);

  return {
    entityType, entities, columns, grouping, activePerspective,
    setVisibleRowCount, grid, cellMonikers, gridCommands,
    handleCellClick, buildRowEntityCommands, renderEditor,
  };
}

/** Grid (spreadsheet-style) view for entities. */
export function GridView({ view }: GridViewProps) {
  const {
    entities, columns, grouping, activePerspective,
    setVisibleRowCount, grid, cellMonikers, gridCommands,
    handleCellClick, buildRowEntityCommands, renderEditor,
  } = useGridViewState(view);

  if (!view.entity_type) {
    console.warn(`[GridView] view "${view.name ?? view.id}" has no entity_type`);
    return (
      <main className="flex-1 flex items-center justify-center text-muted-foreground text-sm">
        View is missing an entity_type definition.
      </main>
    );
  }

  return (
    <CommandScopeProvider commands={gridCommands}>
      <main className="flex-1 flex flex-col min-h-0">
        <GridStatusBar rowCount={entities.length} mode={grid.mode} cursor={grid.cursor} />
        <DataTable
          columns={columns}
          rows={entities}
          grid={grid}
          cellMonikers={cellMonikers}
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
