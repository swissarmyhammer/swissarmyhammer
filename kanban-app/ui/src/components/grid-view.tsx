import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Plus } from "lucide-react";
import { useDispatchCommand, type DispatchOptions } from "@/lib/command-scope";
import { useGrid } from "@/hooks/use-grid";
import { useSchema } from "@/lib/schema-context";
import { useEntityStore } from "@/lib/entity-store-context";
import {
  useEntityFocus,
  type ClaimPredicate,
} from "@/lib/entity-focus-context";
import { CommandScopeProvider, type CommandDef } from "@/lib/command-scope";
import { useActivePerspective } from "@/components/perspective-container";
import { DataTable, type DataTableColumn } from "@/components/data-table";
import { Field } from "@/components/fields/field";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { fieldMoniker } from "@/lib/moniker";
import type { ViewDef, Entity, FieldDef } from "@/types/kanban";

/**
 * Build navigation claim predicates for a single grid cell.
 *
 * Returns an array of ClaimPredicate entries that let the cell claim focus
 * when adjacent cells are focused and a navigation command fires.
 */
interface CellCtx {
  ri: number;
  ci: number;
  cellMonikers: string[][];
  cellMonikerMap: Map<string, { row: number; col: number }>;
  rowCount: number;
  colCount: number;
}

function orthogonalNavPredicates(c: CellCtx): ClaimPredicate[] {
  const out: ClaimPredicate[] = [];
  if (c.ri > 0)
    out.push({
      command: "nav.down",
      when: (f) => f === c.cellMonikers[c.ri - 1][c.ci],
    });
  if (c.ri < c.rowCount - 1)
    out.push({
      command: "nav.up",
      when: (f) => f === c.cellMonikers[c.ri + 1][c.ci],
    });
  if (c.ci > 0)
    out.push({
      command: "nav.right",
      when: (f) => f === c.cellMonikers[c.ri][c.ci - 1],
    });
  if (c.ci < c.colCount - 1)
    out.push({
      command: "nav.left",
      when: (f) => f === c.cellMonikers[c.ri][c.ci + 1],
    });
  return out;
}

function rowEdgeNavPredicates(c: CellCtx): ClaimPredicate[] {
  const out: ClaimPredicate[] = [];
  if (c.ci === 0)
    out.push({
      command: "nav.rowStart",
      when: (f) => {
        const pos = f ? c.cellMonikerMap.get(f) : undefined;
        return pos !== undefined && pos.row === c.ri && pos.col !== 0;
      },
    });
  if (c.ci === c.colCount - 1)
    out.push({
      command: "nav.rowEnd",
      when: (f) => {
        const pos = f ? c.cellMonikerMap.get(f) : undefined;
        return (
          pos !== undefined && pos.row === c.ri && pos.col !== c.colCount - 1
        );
      },
    });
  return out;
}

function gridEdgeNavPredicates(c: CellCtx): ClaimPredicate[] {
  const out: ClaimPredicate[] = [];
  const isGridCell = (mk: string | null) => !!mk && c.cellMonikerMap.has(mk);
  if (c.ri === 0 && c.ci === 0)
    out.push({
      command: "nav.first",
      when: (f) => isGridCell(f) && f !== c.cellMonikers[0][0],
    });
  if (c.ri === c.rowCount - 1 && c.ci === c.colCount - 1)
    out.push({
      command: "nav.last",
      when: (f) =>
        isGridCell(f) &&
        f !== c.cellMonikers[c.rowCount - 1][c.colCount - 1],
    });
  return out;
}

function buildCellPredicates(
  ri: number,
  ci: number,
  cellMonikers: string[][],
  cellMonikerMap: Map<string, { row: number; col: number }>,
  rowCount: number,
  colCount: number,
): ClaimPredicate[] {
  const c: CellCtx = { ri, ci, cellMonikers, cellMonikerMap, rowCount, colCount };
  return [
    ...orthogonalNavPredicates(c),
    ...rowEdgeNavPredicates(c),
    ...gridEdgeNavPredicates(c),
  ];
}

/**
 * Pattern for valid entity type identifiers.
 * Entity types are schema-defined slugs (e.g. "task", "column") — reject
 * anything that doesn't match to prevent command-injection via crafted views.
 */
const VALID_ENTITY_TYPE = /^[a-z][a-z0-9_-]*$/;

/**
 * De-dupe key set for `unknown card_field` warnings.
 *
 * The view's `card_fields` list is applied on every render via `useMemo`, so
 * a naive `console.warn` in the resolution step would spam the log on each
 * state update. We warn exactly once per `(viewId, fieldName)` pair for the
 * lifetime of the app by recording which pairs we've already complained
 * about in this module-scoped Set. Module scope is fine because the log is
 * a developer-facing diagnostic — surviving a fast-refresh or component
 * remount does not meaningfully change the signal.
 */
const warnedUnknownCardFields = new Set<string>();

/**
 * Emit a single `console.warn` the first time we see a `card_fields` entry
 * that does not resolve to a known `FieldDef` for this view.
 *
 * The warning includes the view id, view name, bad field name, and the list
 * of valid field names for the entity so the author sees the typo and its
 * correction in one place. Uses `console.warn` per the project's
 * `frontend-logging` convention — it surfaces in `log show --predicate
 * 'subsystem == "com.swissarmyhammer.kanban"'`.
 *
 * @param viewId - The stable id of the ViewDef whose `card_fields` has the
 *                 bad entry.
 * @param viewName - Human-readable view name for log context.
 * @param badFieldName - The unresolved `card_fields` entry.
 * @param validFieldNames - The field names that exist on the entity schema,
 *                          offered as the set of correct alternatives.
 */
function warnUnknownCardField(
  viewId: string,
  viewName: string | undefined,
  badFieldName: string,
  validFieldNames: string[],
): void {
  const key = `${viewId}::${badFieldName}`;
  if (warnedUnknownCardFields.has(key)) return;
  warnedUnknownCardFields.add(key);
  // Emit the warn on a single line so the project's acceptance grep matches
  // exactly this call site — splitting it across lines would hide the string
  // literal from a non-multiline grep.
  console.warn(`[GridView] unknown card_field "${badFieldName}" in view ${viewId} (${viewName ?? "<unnamed>"}); valid fields: [${validFieldNames.join(", ")}]`);
}

/**
 * Dispatch `entity.add:{entityType}` to create a new entity of the given type.
 *
 * Relies on each entity type's schema-declared `default` for its title-ish
 * field — we intentionally do NOT pass a `title` override because
 * `AddEntity` silently drops overrides for fields not present on the schema
 * (e.g. `tag` has `tag_name`, `project` has `name`). Passing `title` would
 * be honoured only for `task` and discarded for other types, which is
 * confusing. Using schema defaults gives a consistent "new-{type}" label
 * for every entity type.
 *
 * @param dispatch - The dispatch function from `useDispatchCommand`.
 * @param entityType - The sanitized entity type slug.
 */
function addNewEntity(
  dispatch: (cmd: string, opts?: DispatchOptions) => Promise<unknown>,
  entityType: string,
): void {
  dispatch(`entity.add:${entityType}`).catch((err) =>
    console.error("Failed to add entity:", err),
  );
}

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
  const { getSchema } = useSchema();
  const schema = getSchema(entityType);
  const fields = schema?.fields ?? [];

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
    const resolved: DataTableColumn[] = [];
    for (const name of fieldNames) {
      const fieldDef = fieldMap.get(name);
      if (fieldDef === undefined) {
        // Surface the silent-drop so a future author sees the typo. The helper
        // dedupes per (viewId, fieldName) so the log doesn't repeat on each
        // re-render.
        warnUnknownCardField(
          view.id,
          view.name,
          name,
          fields.map((f) => f.name),
        );
        continue;
      }
      resolved.push({ field: fieldDef });
    }
    return resolved;
  }, [view.card_fields, view.id, view.name, fields]);

  return {
    entityType,
    entities,
    columns,
    grouping,
    activePerspective,
  };
}

/**
 * Build cell moniker matrices, cursor tracking, claim predicates, and grid state.
 *
 * Handles the pull-based navigation system: each cell declares predicates
 * for which nav commands it should claim focus on.
 */
function useCellMonikers(entities: Entity[], columns: DataTableColumn[]) {
  const cellMonikers = useMemo(
    () =>
      entities.map((e) =>
        columns.map((col) => fieldMoniker(e.entity_type, e.id, col.field.name)),
      ),
    [entities, columns],
  );
  const cellMonikerMap = useMemo(() => {
    const map = new Map<string, { row: number; col: number }>();
    cellMonikers.forEach((row, r) => {
      row.forEach((mk, c) => map.set(mk, { row: r, col: c }));
    });
    return map;
  }, [cellMonikers]);
  return { cellMonikers, cellMonikerMap };
}

function useInitialCellFocus(
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

function useGridNavigation(entities: Entity[], columns: DataTableColumn[]) {
  const [visibleRowCount, setVisibleRowCount] = useState(entities.length);
  useEffect(() => {
    setVisibleRowCount(entities.length);
  }, [entities.length]);

  const { focusedMoniker, setFocus, broadcastNavCommand } = useEntityFocus();
  const { cellMonikers, cellMonikerMap } = useCellMonikers(entities, columns);

  const derivedCursor = useMemo(
    () => (focusedMoniker ? cellMonikerMap.get(focusedMoniker) ?? null : null),
    [focusedMoniker, cellMonikerMap],
  );
  const grid = useGrid({
    rowCount: visibleRowCount,
    colCount: columns.length,
    cursor: derivedCursor ?? undefined,
  });

  useInitialCellFocus(cellMonikers[0]?.[0] ?? null, derivedCursor, setFocus);

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
        if (entityType === "") return;
        addNewEntity(dispatch, entityType);
      },
    },
    {
      id: "grid.newAbove",
      name: "New Row Above",
      keys: { vim: "O", cua: "Mod+Shift+Enter" },
      execute: () => {
        if (entityType === "") return;
        addNewEntity(dispatch, entityType);
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
/** Render the inline cell editor — a Field in compact editing mode. */
function renderGridCellEditor(
  entity: Entity,
  field: FieldDef,
  onCommit: (value: unknown) => void,
  onCancel: () => void,
) {
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

function useGridCallbacks(
  cellMonikers: string[][],
  setFocus: (mk: string) => void,
) {
  const handleCellClick = useCallback(
    (row: number, col: number) => {
      const mk = cellMonikers[row]?.[col];
      if (mk) setFocus(mk);
    },
    [cellMonikers, setFocus],
  );

  return {
    handleCellClick,
    renderEditor: renderGridCellEditor,
  };
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
 * Thin action bar below the grid with a "+" button that dispatches
 * `entity.add:{entityType}` to create a new entity of the correct type.
 *
 * Mirrors the add-task button in `column-view.tsx`: plain `<button>`, `Plus`
 * icon, tooltip, muted styling. Aria-label and tooltip read as
 * "Add {EntityType}" with the type title-cased.
 */
function AddEntityBar({
  entityType,
  dispatch,
}: {
  entityType: string;
  dispatch: (cmd: string, opts?: DispatchOptions) => Promise<unknown>;
}) {
  const label = `Add ${entityType.charAt(0).toUpperCase() + entityType.slice(1)}`;
  return (
    <div className="flex items-center px-2 py-1 border-t border-border">
      <Tooltip>
        <TooltipTrigger asChild>
          <button
            type="button"
            aria-label={label}
            className="p-0.5 rounded text-muted-foreground/50 hover:text-muted-foreground hover:bg-muted transition-colors"
            onClick={() => addNewEntity(dispatch, entityType)}
          >
            <Plus className="h-4 w-4" />
          </button>
        </TooltipTrigger>
        <TooltipContent>{label}</TooltipContent>
      </Tooltip>
    </div>
  );
}

/** Grid (spreadsheet-style) view for entities. */
/** Empty-state fallback when the view's entity_type is missing or invalid. */
function GridViewMissingEntityType() {
  return (
    <main className="flex-1 flex items-center justify-center text-muted-foreground text-sm">
      View is missing an entity_type definition.
    </main>
  );
}

interface GridBodyProps {
  data: ReturnType<typeof useGridData>;
  nav: ReturnType<typeof useGridNavigation>;
  callbacks: ReturnType<typeof useGridCallbacks>;
  dispatch: (cmd: string, opts?: DispatchOptions) => Promise<unknown>;
}

function GridBody({ data, nav, callbacks, dispatch }: GridBodyProps) {
  return (
    <main className="flex-1 flex flex-col min-h-0">
      <GridStatusBar
        rowCount={data.entities.length}
        mode={nav.grid.mode}
        cursor={nav.grid.cursor}
      />
      <DataTable
        columns={data.columns}
        rows={data.entities}
        grid={nav.grid}
        cellMonikers={nav.cellMonikers}
        claimPredicates={nav.claimPredicates}
        onCellClick={callbacks.handleCellClick}
        renderEditor={callbacks.renderEditor}
        grouping={data.grouping}
        onVisibleRowCount={nav.setVisibleRowCount}
        perspectiveSort={data.activePerspective?.sort}
        perspectiveId={data.activePerspective?.id}
      />
      <AddEntityBar entityType={data.entityType} dispatch={dispatch} />
    </main>
  );
}

export function GridView({ view }: GridViewProps) {
  const dispatch = useDispatchCommand();
  const data = useGridData(view);
  const nav = useGridNavigation(data.entities, data.columns);
  const gridCommands = useGridCommands(
    nav.broadcastNavCommand,
    nav.grid,
    data.entities,
    data.entityType,
    dispatch,
  );
  const callbacks = useGridCallbacks(nav.cellMonikers, nav.setFocus);

  // Guard on the sanitized `entityType`, not raw `view.entity_type`.
  // `useGridData` reduces invalid values to the empty string via
  // `VALID_ENTITY_TYPE`; dispatching `entity.add:` with no suffix would
  // surface a confusing user-facing failure from the backend.
  if (data.entityType === "") {
    console.warn(
      `[GridView] view "${view.name ?? view.id}" has missing or invalid entity_type: ${JSON.stringify(view.entity_type)}`,
    );
    return <GridViewMissingEntityType />;
  }

  return (
    <CommandScopeProvider commands={gridCommands}>
      <GridBody data={data} nav={nav} callbacks={callbacks} dispatch={dispatch} />
    </CommandScopeProvider>
  );
}
