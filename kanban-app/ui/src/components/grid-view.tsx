import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { ReactNode } from "react";
import { Plus } from "lucide-react";
import { useDispatchCommand, type DispatchOptions } from "@/lib/command-scope";
import { useContextMenu } from "@/lib/context-menu";
import { useGrid } from "@/hooks/use-grid";
import { useSchema } from "@/lib/schema-context";
import { useEntityStore } from "@/lib/entity-store-context";
import {
  useFocusActions,
  useFocusedMoniker,
  useFocusBySegmentPath,
} from "@/lib/entity-focus-context";
import { CommandScopeProvider, type CommandDef } from "@/lib/command-scope";
import { useActivePerspective } from "@/components/perspective-container";
import { DataTable, type DataTableColumn } from "@/components/data-table";
import { Field } from "@/components/fields/field";
import { Button } from "@/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { FocusScope } from "@/components/focus-scope";
import { useOptionalEnclosingLayerFq } from "@/components/layer-fq-context";
import {
  useOptionalSpatialFocusActions,
  type SpatialFocusActions,
} from "@/lib/spatial-focus-context";
import {
  asFq,
  asSegment,
  composeFq,
  type FullyQualifiedMoniker,
} from "@/types/spatial";
import { gridCellMoniker, parseGridCellMoniker } from "@/lib/moniker";
import type { ViewDef, Entity, FieldDef } from "@/types/kanban";

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
  console.warn(
    `[GridView] unknown card_field "${badFieldName}" in view ${viewId} (${viewName ?? "<unnamed>"}); valid fields: [${validFieldNames.join(", ")}]`,
  );
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
 * Resolve the cell-cursor coordinates implied by the currently focused moniker.
 *
 * Returns `{ row, col }` (zero-based grid indices) when the focused moniker is
 * a `grid_cell:R:K` whose `K` matches one of the grid's columns and `R` is
 * within the row count. Returns `null` otherwise — focus is outside the grid
 * (`ui:navbar`, an entity moniker, or no focus). The grid uses a `null`
 * cursor to suppress its ring, instead of falling back to a stale `{0, 0}`
 * default that would highlight the top-left cell whenever focus moves
 * elsewhere.
 *
 * Reads:
 *   - `focusedMoniker` — the currently focused entity-focus moniker.
 *   - `columns` — the grid's column list, used to translate `colKey` to
 *     a numeric column index for the `useGrid` cursor input.
 *   - `rowCount` — the number of data rows in the grid; coordinates
 *     beyond this range are rejected.
 */
function resolveCursorFromFocus(
  focusedMoniker: string | null,
  columns: DataTableColumn[],
  rowCount: number,
): { row: number; col: number } | null {
  if (!focusedMoniker) return null;
  const parsed = parseGridCellMoniker(focusedMoniker);
  if (!parsed) return null;
  const colIdx = columns.findIndex((c) => c.field.name === parsed.colKey);
  if (colIdx === -1) return null;
  if (parsed.row < 0 || parsed.row >= rowCount) return null;
  return { row: parsed.row, col: colIdx };
}

function useInitialCellFocus(
  firstCellMoniker: string | null,
  derivedCursor: { row: number; col: number } | null,
  focusCellSegment: (cellSegment: string) => void,
) {
  const hasInitialFocusRef = useRef(false);
  useEffect(() => {
    if (!firstCellMoniker || hasInitialFocusRef.current) return;
    if (!derivedCursor) {
      focusCellSegment(firstCellMoniker);
      hasInitialFocusRef.current = true;
    }
  }, [firstCellMoniker, focusCellSegment, derivedCursor]);
}

function useGridNavigation(entities: Entity[], columns: DataTableColumn[]) {
  const [visibleRowCount, setVisibleRowCount] = useState(entities.length);
  useEffect(() => {
    setVisibleRowCount(entities.length);
  }, [entities.length]);

  const focusCellSegment = useFocusBySegmentPath();
  // Adapt the multi-segment focus helper to a single-cell-segment caller.
  // Cell-focus mutations in the grid land at FQM
  // `<gridZone>/<rowEntityMk>/grid_cell:R:K` — the row's outer
  // `<FocusZone moniker={asSegment(entityMk)} renderContainer={false}>`
  // publishes its FQM through `FullyQualifiedMonikerContext`, so the
  // cell's composed FQM nests under the row entity. Parse the cell
  // segment to recover the data-row index, look up the row's entity
  // moniker, and dispatch the two-segment compose `[<entityMk>, <cellSeg>]`
  // through the multi-segment helper. When the segment can't be parsed
  // (callers that pre-validated) or the row is out of range, fall back
  // to a single-segment compose so the dispatch still reaches the
  // kernel — the kernel will log an `unknown FQM` error on a malformed
  // target rather than silently dropping the keystroke.
  const focusCell = useCallback(
    (cellSegment: string) => {
      const parsed = parseGridCellMoniker(cellSegment);
      if (parsed === null || parsed.row < 0 || parsed.row >= entities.length) {
        focusCellSegment(asSegment(cellSegment));
        return;
      }
      const rowEntityMk = entities[parsed.row].moniker;
      focusCellSegment(asSegment(rowEntityMk), asSegment(cellSegment));
    },
    [focusCellSegment, entities],
  );
  const focusedMoniker = useFocusedMoniker();

  // Cursor derivation: the focused moniker is the single source of truth.
  // The two derived shapes below answer different questions and read the
  // moniker independently:
  //
  //   - `gridCellCursor: {row, colKey}` — what the rendering layer needs
  //     to stamp the `data-cell-cursor` debug/e2e attribute. Parsed
  //     straight from the moniker; matches on column field name, no
  //     numeric column index required. The visible focus decoration is
  //     not driven from this — the cell's `<FocusScope>` renders
  //     `<FocusIndicator>` from its own React focus state.
  //   - `derivedCursor: {row, col}` — what `useGrid` needs (a numeric
  //     row/col cursor input). Built by mapping `colKey` to its column
  //     index in the current `columns` array.
  //
  // When the focused moniker is not a `grid_cell:R:K` whose column key is
  // present and whose row is in range, both shapes are `null` so the grid
  // suppresses the `data-cell-cursor` attribute instead of falling back
  // to the internal `{0, 0}` default.
  const gridCellCursor = useMemo<{ row: number; colKey: string } | null>(() => {
    if (!focusedMoniker) return null;
    const parsed = parseGridCellMoniker(focusedMoniker);
    if (!parsed) return null;
    if (parsed.row < 0 || parsed.row >= entities.length) return null;
    const exists = columns.some((c) => c.field.name === parsed.colKey);
    if (!exists) return null;
    return { row: parsed.row, colKey: parsed.colKey };
  }, [focusedMoniker, columns, entities.length]);

  const derivedCursor = useMemo<{ row: number; col: number } | null>(
    () => resolveCursorFromFocus(focusedMoniker, columns, entities.length),
    [focusedMoniker, columns, entities.length],
  );

  const grid = useGrid({
    rowCount: visibleRowCount,
    colCount: columns.length,
    cursor: derivedCursor ?? undefined,
  });

  // Seed initial focus on the top-left cell once when the grid has rows but
  // no grid_cell focus has been established yet. After this, focus is
  // entirely driven by the spatial-nav layer.
  const firstCellMoniker = useMemo<string | null>(() => {
    if (entities.length === 0 || columns.length === 0) return null;
    return gridCellMoniker(0, columns[0].field.name);
  }, [entities.length, columns]);

  useInitialCellFocus(firstCellMoniker, derivedCursor, focusCell);

  return {
    setVisibleRowCount,
    grid,
    focusCell,
    gridCellCursor,
  };
}

/**
 * Snapshot of the data the row-extreme / grid-extreme commands need at
 * execute time.
 *
 * Held in a ref so the command closures can be minted once per `useMemo`
 * without re-binding on every cursor move. The ref is updated synchronously
 * on each render so the closures always read fresh data when they fire.
 */
interface GridExtremeContext {
  entities: Entity[];
  columns: DataTableColumn[];
  spatial: SpatialFocusActions | null;
  setFocus: (fq: FullyQualifiedMoniker | null) => void;
}

/**
 * Strip the trailing segment from a fully-qualified moniker, returning the
 * parent FQM.
 *
 * Returns `null` when the input has no separator (a malformed FQM the
 * kernel does not produce in well-formed code) so the caller can
 * short-circuit gracefully.
 *
 * @param fq - The fully-qualified moniker to walk one level up.
 */
function fqDropLastSegment(
  fq: FullyQualifiedMoniker,
): FullyQualifiedMoniker | null {
  const idx = fq.lastIndexOf("/");
  if (idx <= 0) return null;
  return asFq(fq.slice(0, idx));
}

/**
 * Compute the row index from the currently focused FQM, falling back to the
 * grid's internal cursor when focus is outside the grid (or when the grid
 * is empty).
 *
 * Used by the row-extreme commands (`grid.moveToRowStart` /
 * `grid.moveToRowEnd`) to determine which row's first/last cell to jump to.
 * The focused FQM is the source of truth — `useGrid`'s cursor is a derived
 * mirror of it, but the focused FQM survives focus moves outside the grid
 * (e.g. into the inspector) and the cursor would have been clamped.
 */
function rowFromFocus(focusedFq: FullyQualifiedMoniker | null): number | null {
  if (focusedFq === null) return null;
  const parsed = parseGridCellMoniker(focusedFq);
  if (!parsed) return null;
  return parsed.row;
}

/**
 * Move focus to the cell at `(row, colKey)` inside the grid that currently
 * owns the focused cell.
 *
 * Walks up two segments from the currently focused cell FQM to recover
 * the `ui:grid` zone FQM — every cell FQM has the shape
 * `/window/.../ui:grid/<rowEntityMk>/grid_cell:R:K` because the row's
 * `<FocusZone moniker={asSegment(entityMk)} renderContainer={false}>`
 * publishes its FQM through `FullyQualifiedMonikerContext`. Looks up
 * the destination row's entity moniker from `ctx.entities[row]` and
 * composes the destination cell FQM as `<gridZone>/<destEntityMk>/<cellSeg>`.
 * Dispatches through `setFocus`, which routes through the spatial-nav
 * kernel via `spatial_focus(fq)` — exactly what a click on the
 * destination cell would do.
 *
 * Silently returns when there is no focused cell to derive the parent FQM
 * from (focus is outside the grid) or when the destination row/column is
 * out of range — the keystroke becomes a visible no-op rather than a
 * runtime error.
 *
 * @param ctx - The grid extreme context (entities, columns, setFocus, spatial actions).
 * @param row - Destination row index.
 * @param colKey - Destination column field name.
 */
function focusGridCell(
  ctx: GridExtremeContext,
  row: number,
  colKey: string,
): void {
  if (ctx.spatial === null) return;
  if (row < 0 || row >= ctx.entities.length) return;
  if (!ctx.columns.some((c) => c.field.name === colKey)) return;

  const focusedFq = ctx.spatial.focusedFq();
  if (focusedFq === null) return;

  // Walk up two segments: cell → row → ui:grid. The row Zone is
  // `renderContainer={false}` so it does not appear in the kernel
  // registry, but its FQM is still part of the cell's path because
  // descendant scopes compose against the FQM context the row Zone
  // publishes.
  const rowFq = fqDropLastSegment(focusedFq);
  if (rowFq === null) return;
  const gridZoneFq = fqDropLastSegment(rowFq);
  if (gridZoneFq === null) return;

  const destEntityMk = ctx.entities[row].moniker;
  const cellSegment = asSegment(gridCellMoniker(row, colKey));
  ctx.setFocus(
    composeFq(composeFq(gridZoneFq, asSegment(destEntityMk)), cellSegment),
  );
}

/**
 * Build the grid-scope commands that have no global `nav.*` counterpart.
 *
 * These commands route through the spatial-nav kernel via `setFocus` —
 * never via the legacy broadcast path. The cardinal directions
 * (`up`/`down`/`left`/`right`) and the global `first`/`last` (vim `Shift+G`,
 * cua `Home`/`End` outside the grid scope) are owned by the global
 * `nav.*` commands in `app-shell.tsx` and intentionally NOT shadowed here.
 *
 * The four commands kept here:
 *
 *   - `grid.moveToRowStart` (vim `0`, cua `Home`) — first cell of the
 *     focused row. Shadows the global `nav.first` cua `Home` binding so
 *     `Home` inside the grid means "row start", not "grid start".
 *   - `grid.moveToRowEnd` (vim `$`, cua `End`) — last cell of the focused
 *     row. Shadows the global `nav.last` cua `End` binding so `End` inside
 *     the grid means "row end", not "grid end".
 *   - `grid.firstCell` (cua `Mod+Home`) — absolute first cell of the grid.
 *     Fills a gap: the global `nav.first` only binds `Home` (cua), not
 *     `Mod+Home`.
 *   - `grid.lastCell` (cua `Mod+End`) — absolute last cell of the grid.
 *     Fills a gap: the global `nav.last` binds `Shift+G` (vim) and `End`
 *     (cua), but not `Mod+End`.
 */
function buildGridExtremeCommands(
  ctxRef: React.RefObject<GridExtremeContext>,
): CommandDef[] {
  return [
    {
      id: "grid.moveToRowStart",
      name: "Row Start",
      keys: { vim: "0", cua: "Home" },
      execute: () => {
        const ctx = ctxRef.current;
        const row = rowFromFocus(ctx.spatial?.focusedFq() ?? null);
        if (row === null || ctx.columns.length === 0) return;
        focusGridCell(ctx, row, ctx.columns[0].field.name);
      },
    },
    {
      id: "grid.moveToRowEnd",
      name: "Row End",
      keys: { vim: "$", cua: "End" },
      execute: () => {
        const ctx = ctxRef.current;
        const row = rowFromFocus(ctx.spatial?.focusedFq() ?? null);
        if (row === null || ctx.columns.length === 0) return;
        focusGridCell(ctx, row, ctx.columns[ctx.columns.length - 1].field.name);
      },
    },
    {
      id: "grid.firstCell",
      name: "First Cell",
      keys: { cua: "Mod+Home" },
      execute: () => {
        const ctx = ctxRef.current;
        if (ctx.columns.length === 0 || ctx.entities.length === 0) return;
        focusGridCell(ctx, 0, ctx.columns[0].field.name);
      },
    },
    {
      id: "grid.lastCell",
      name: "Last Cell",
      keys: { cua: "Mod+End" },
      execute: () => {
        const ctx = ctxRef.current;
        if (ctx.columns.length === 0 || ctx.entities.length === 0) return;
        focusGridCell(
          ctx,
          ctx.entities.length - 1,
          ctx.columns[ctx.columns.length - 1].field.name,
        );
      },
    },
  ];
}

/** Grid mode-switching commands (edit, exit, visual). */
function buildGridModeCommands(
  gridRef: React.RefObject<ReturnType<typeof useGrid>>,
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
  ];
}

/** Grid row-mutation commands (delete row, new row above/below). */
function buildGridRowCommands(
  gridRef: React.RefObject<ReturnType<typeof useGrid>>,
  entities: Entity[],
  entityType: string,
  dispatch: (cmd: string, opts?: DispatchOptions) => Promise<unknown>,
): CommandDef[] {
  return [
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

/** Build editing and row-mutation CommandDefs for the grid. */
function buildGridEditCommands(
  gridRef: React.RefObject<ReturnType<typeof useGrid>>,
  entities: Entity[],
  entityType: string,
  dispatch: (cmd: string, opts?: DispatchOptions) => Promise<unknown>,
): CommandDef[] {
  return [
    ...buildGridModeCommands(gridRef),
    ...buildGridRowCommands(gridRef, entities, entityType, dispatch),
  ];
}

/**
 * Compose the full grid CommandDef array.
 *
 * The grid scope owns two non-overlapping command families:
 *
 *   - Edit / mode / row-mutation commands (`grid.edit`, `grid.toggleVisual`,
 *     `grid.deleteRow`, `grid.newAbove`/`grid.newBelow`, …) — these have no
 *     equivalent at any other scope.
 *   - Row-extreme and grid-extreme cell-jump commands
 *     (`grid.moveToRowStart`, `grid.moveToRowEnd`, `grid.firstCell`,
 *     `grid.lastCell`) — these route through the spatial-nav kernel via
 *     `setFocus`. The cardinal-direction nav commands (`nav.up` /
 *     `nav.down` / `nav.left` / `nav.right`) live at the global scope in
 *     `app-shell.tsx` and intentionally are NOT shadowed here — the global
 *     versions correctly dispatch `spatial_navigate` against the focused
 *     cell's FQM.
 */
function useGridCommands(
  grid: ReturnType<typeof useGrid>,
  entities: Entity[],
  columns: DataTableColumn[],
  entityType: string,
  dispatch: (cmd: string, opts?: DispatchOptions) => Promise<unknown>,
): CommandDef[] {
  const gridRef = useRef(grid);
  gridRef.current = grid;

  // Read the spatial-focus actions (for `focusedFq()`) and the entity-focus
  // `setFocus` once and stash them in a context bag for the row-extreme
  // commands. The bag is held in a ref so the commands minted in `useMemo`
  // below can read fresh values (cursor row, visible columns) without
  // re-binding on every keystroke.
  const spatial = useOptionalSpatialFocusActions();
  const { setFocus } = useFocusActions();
  const extremeCtxRef = useRef<GridExtremeContext>({
    entities,
    columns,
    spatial,
    setFocus,
  });
  extremeCtxRef.current = { entities, columns, spatial, setFocus };

  return useMemo<CommandDef[]>(
    () => [
      ...buildGridExtremeCommands(extremeCtxRef),
      ...buildGridEditCommands(gridRef, entities, entityType, dispatch),
    ],
    [entities, entityType, dispatch],
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
  _columns: DataTableColumn[],
  _focusCell: (cellSegment: string) => void,
) {
  // The cell-click → focus update is owned by the per-cell `<FocusScope>`'s
  // `onClick` handler in `GridCellFocusable`, which calls `focus(fq)` on
  // the cell's FQM. The inner-div click handler in `<GridCellFocusable>`
  // exists for non-focus side effects (e.g. `enterEdit` on double-click).
  // Calling `focusCell` here would dispatch a redundant `spatial_focus`
  // for the same FQM, which the kernel would short-circuit but tests
  // counting IPC calls would see as a double-fire.
  const handleCellClick = useCallback((_row: number, _col: number) => {
    // No-op — FocusScope owns focus updates for the cell.
  }, []);

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
 * Title-case an entity type slug for display (e.g. `tag` -> `Tag`,
 * `my-entity-type` -> `My Entity Type`).
 *
 * Entity types are constrained by `VALID_ENTITY_TYPE` to `[a-z][a-z0-9_-]*`,
 * which permits hyphen- and underscore-separated multi-word slugs. Splits on
 * those separators, upper-cases each word's first char, and joins with spaces.
 *
 * For the four builtin single-word types (task, tag, project, column) this
 * matches the prior single-char upper-case behavior; multi-word slugs now
 * render cleanly instead of producing strings like "My-entity-type".
 */
function titleCaseEntityType(entityType: string): string {
  if (entityType.length === 0) return entityType;
  return entityType
    .split(/[-_]/)
    .map((word) =>
      word.length === 0 ? word : word.charAt(0).toUpperCase() + word.slice(1),
    )
    .join(" ");
}

/**
 * Prominent empty-state for a grid view with zero rows.
 *
 * Centered block with a large `Plus` icon, "No {EntityType}s yet" text,
 * and a primary-styled "New {EntityType}" button. Clicking the button
 * dispatches `entity.add:{entityType}` to create the first entity.
 *
 * Right-clicking anywhere in the empty-state block opens the view-scoped
 * context menu (same pipeline as right-click on a row) — this surfaces
 * "New {EntityType}" and whatever other view-level commands exist.
 *
 * Rendered in place of `<DataTable>` + `<AddEntityBar>` when
 * `entities.length === 0`. Do NOT render both — `AddEntityBar` is a
 * secondary affordance for non-empty grids; with no rows, the centered
 * primary button is the single call-to-action.
 */
function GridEmptyState({
  entityType,
  dispatch,
  onContextMenu,
}: {
  entityType: string;
  dispatch: (cmd: string, opts?: DispatchOptions) => Promise<unknown>;
  /**
   * View-scoped context-menu handler. Passed in from `GridBody` which owns
   * the single `useContextMenu()` call site for the grid body — keeps the
   * empty and non-empty branches computing one scope chain per render.
   */
  onContextMenu: (e: React.MouseEvent) => void;
}) {
  const typeTitle = titleCaseEntityType(entityType);
  const label = `New ${typeTitle}`;
  // Trivial pluralisation works for all four builtin entity types that
  // have grid views: tasks, tags, projects, columns. If a future entity
  // type breaks this (e.g. "person" -> "persons" not "people"), schema
  // metadata can add an explicit plural later.
  const plural = `${entityType}s`;
  return (
    <div
      data-testid="grid-empty-state"
      className="flex-1 flex flex-col items-center justify-center gap-4 p-8 text-center"
      onContextMenu={onContextMenu}
    >
      <Plus className="h-12 w-12 text-muted-foreground/40" aria-hidden="true" />
      <p className="text-sm text-muted-foreground">No {plural} yet</p>
      <Button
        type="button"
        variant="default"
        size="default"
        onClick={() => addNewEntity(dispatch, entityType)}
      >
        <Plus className="h-4 w-4" />
        {label}
      </Button>
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
  const label = `Add ${titleCaseEntityType(entityType)}`;
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
  // When a row-level `onContextMenu` runs, `useContextMenu` calls
  // `e.stopPropagation()` — so this handler only fires from the grid
  // whitespace (between rows, below the last row). It surfaces the same
  // view-scoped command set as the empty-state right-click: "New
  // <EntityType>" plus whatever other commands the view declares.
  const containerContextMenu = useContextMenu();

  const isEmpty = data.entities.length === 0;

  return (
    <main className="flex-1 flex flex-col min-h-0">
      <GridStatusBar
        rowCount={data.entities.length}
        mode={nav.grid.mode}
        cursor={nav.grid.cursor}
      />
      {isEmpty ? (
        <GridEmptyState
          entityType={data.entityType}
          dispatch={dispatch}
          onContextMenu={containerContextMenu}
        />
      ) : (
        <GridSpatialZone>
          <DataTable
            columns={data.columns}
            rows={data.entities}
            grid={nav.grid}
            onCellClick={callbacks.handleCellClick}
            renderEditor={callbacks.renderEditor}
            grouping={data.grouping}
            onVisibleRowCount={nav.setVisibleRowCount}
            perspectiveSort={data.activePerspective?.sort}
            perspectiveId={data.activePerspective?.id}
            onContainerContextMenu={containerContextMenu}
            gridCellCursor={nav.gridCellCursor}
          />
          <AddEntityBar entityType={data.entityType} dispatch={dispatch} />
        </GridSpatialZone>
      )}
    </main>
  );
}

/**
 * Wrap the grid body in a `<FocusZone moniker={asSegment("ui:grid")}>` when
 * the surrounding tree mounts the spatial-nav stack.
 *
 * `<FocusZone>` enforces a strict contract — it throws when no `<FocusLayer>`
 * ancestor is present. That contract is correct for the production tree
 * (`App.tsx` always mounts the providers) but would force every `GridView`
 * unit test that doesn't care about spatial nav to set up the providers.
 * Conditionally rendering the zone when both context lookups succeed keeps
 * the strict contract intact for direct `<FocusZone>` usage while letting
 * the existing test suite keep its narrow provider tree.
 *
 * Mirrors the `BoardSpatialZone` / `PerspectiveSpatialZone` pattern used
 * elsewhere in the project. The zone renders directly inside the
 * surrounding `ui:perspective` zone so its `parent_zone` is
 * `ui:perspective` — the inner view body has no intermediate chrome
 * wrapper of its own. Cells register as `<FocusScope>` leaves under this
 * zone in `data-table.tsx`.
 *
 * The wrapper renders `<>` (a fragment) when the spatial stack is absent so
 * the inner DOM tree (DataTable's scroll container + AddEntityBar) keeps the
 * same flex sibling relationship it always had with `GridStatusBar`.
 */
function GridSpatialZone({ children }: { children: ReactNode }) {
  const layerKey = useOptionalEnclosingLayerFq();
  const actions = useOptionalSpatialFocusActions();
  if (!layerKey || !actions) {
    return <>{children}</>;
  }
  return (
    <FocusScope
      moniker={asSegment("ui:grid")}
      // Suppress the visible focus bar around the grid body. The grid is a
      // viewport-filling zone — every cell already advertises its own focus
      // via the per-cell `<FocusIndicator>`, and rendering a second bar
      // around the entire scroll container would be visual noise wrapped
      // around (and competing with) the cell-level decoration. The
      // `data-focused` attribute on the zone still flips so e2e selectors
      // and debugging tooling can observe the zone-level claim; only the
      // visible bar is suppressed.
      // showFocus=false: viewport-filling grid; per-cell indicators own focus.
      showFocus={false}
      className="flex-1 flex flex-col min-h-0"
    >
      {children}
    </FocusScope>
  );
}

export function GridView({ view }: GridViewProps) {
  const dispatch = useDispatchCommand();
  const data = useGridData(view);
  const nav = useGridNavigation(data.entities, data.columns);
  const gridCommands = useGridCommands(
    nav.grid,
    data.entities,
    data.columns,
    data.entityType,
    dispatch,
  );
  const callbacks = useGridCallbacks(data.columns, nav.focusCell);

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
      <GridBody
        data={data}
        nav={nav}
        callbacks={callbacks}
        dispatch={dispatch}
      />
    </CommandScopeProvider>
  );
}
