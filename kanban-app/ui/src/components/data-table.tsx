import { useRef, useEffect, useCallback, useMemo, useState } from "react";
import { ArrowUp, ArrowDown, ChevronRight, ChevronDown } from "lucide-react";
import {
  useReactTable,
  getCoreRowModel,
  getSortedRowModel,
  getGroupedRowModel,
  getExpandedRowModel,
  flexRender,
  type ColumnDef,
  type SortingState,
  type GroupingState,
  type Row,
  type Header,
  type HeaderContext,
  type Table as TanTable,
} from "@tanstack/react-table";
import {
  Table,
  TableHeader,
  TableBody,
  TableRow,
  TableHead,
  TableCell,
} from "@/components/ui/table";
import { cn } from "@/lib/utils";
import { useContextMenu } from "@/lib/context-menu";
import { useDispatchCommand, type CommandDef } from "@/lib/command-scope";
import { FocusScope, useFocusScopeElementRef } from "@/components/focus-scope";
import { Field } from "@/components/fields/field";
import type { UseGridReturn } from "@/hooks/use-grid";
import { useEntityFocus } from "@/lib/entity-focus-context";
import {
  columnHeaderMoniker,
  fieldMoniker,
  ROW_SELECTOR_FIELD,
} from "@/lib/moniker";
import type { Entity, FieldDef, PerspectiveSortEntry } from "@/types/kanban";

const ROW_SELECTOR_WIDTH = 40;

/** Column definition for a data table: a field and optional width override. */
export interface DataTableColumn {
  field: FieldDef;
  width?: number;
}

interface DataTableProps {
  columns: DataTableColumn[];
  rows: Entity[];
  grid: UseGridReturn;
  /**
   * 2D array of cell monikers: cellMonikers[row][col] = moniker string.
   * Used for click-to-focus mapping in grid cells.
   */
  cellMonikers?: string[][];
  onCellClick?: (row: number, col: number) => void;
  onRowContextMenu?: (entity: Entity, e: React.MouseEvent) => void;
  renderEditor?: (
    entity: Entity,
    field: FieldDef,
    onCommit: (value: unknown) => void,
    onCancel: () => void,
  ) => React.ReactNode;
  grouping?: string[];
  /** Called when the visible data row count changes (e.g. group collapse). */
  onVisibleRowCount?: (count: number) => void;
  /** Show a leading selector column before field columns (default true). */
  showRowSelector?: boolean;
  /**
   * Optional factory that returns entity-specific commands for a given row.
   * When provided, each row is wrapped in a FocusScope with these commands
   * so right-click, inspect, and palette resolve for that row's entity.
   */
  rowEntityCommands?: (entity: Entity) => CommandDef[];
  /**
   * Active perspective sort entries. When provided, column headers show
   * sort indicators and dispatch `perspective.sort.toggle` on click.
   */
  perspectiveSort?: readonly PerspectiveSortEntry[];
  /** Active perspective ID — required for dispatching sort commands. */
  perspectiveId?: string;
}

// ---------------------------------------------------------------------------
// Sort map type used by header cells
// ---------------------------------------------------------------------------

type SortMap = Map<string, { direction: "asc" | "desc"; priority: number }>;

// ---------------------------------------------------------------------------
// Hooks
// ---------------------------------------------------------------------------

/**
 * Build a lookup: field name to { direction, priority } from perspective sort entries.
 *
 * Returns an empty map when no perspective sort is active.
 */
function usePerspectiveSortMap(
  perspectiveSort: readonly PerspectiveSortEntry[] | undefined,
): SortMap {
  return useMemo(() => {
    const map: SortMap = new Map();
    if (!perspectiveSort) return map;
    for (let i = 0; i < perspectiveSort.length; i++) {
      map.set(perspectiveSort[i].field, {
        direction: perspectiveSort[i].direction,
        priority: i + 1,
      });
    }
    return map;
  }, [perspectiveSort]);
}

function buildColumnDef(col: DataTableColumn): ColumnDef<Entity> {
  return {
    id: col.field.name,
    accessorFn: (row: Entity) => row.fields[col.field.name],
    header: col.field.name.replace(/_/g, " "),
    size: col.width,
    cell: ({ row: tanRow }) => {
      const entity = tanRow.original;
      return (
        <Field
          fieldDef={col.field}
          entityType={entity.entity_type}
          entityId={entity.id}
          mode="compact"
          editing={false}
        />
      );
    },
    sortingFn: (rowA: Row<Entity>, rowB: Row<Entity>, columnId: string) => {
      return compareValues(
        rowA.original.fields[columnId],
        rowB.original.fields[columnId],
        col.field,
      );
    },
    aggregationFn: "count" as const,
  };
}

function useTanstackColumns(columns: DataTableColumn[]) {
  return useMemo<ColumnDef<Entity>[]>(
    () => columns.map(buildColumnDef),
    [columns],
  );
}

function useGroupingState(groupingProp: string[] | undefined) {
  const [grouping, setGrouping] = useState<GroupingState>(groupingProp ?? []);
  // Sync external grouping prop — reset to [] when cleared so the grid
  // reverts to a flat layout instead of retaining stale grouping state.
  useEffect(() => {
    setGrouping(groupingProp ?? []);
  }, [groupingProp]);
  return [grouping, setGrouping] as const;
}

/**
 * Build TanStack column definitions and instantiate the table.
 *
 * Encapsulates column def construction, sorting, grouping, and all
 * TanStack row-model plugins so the main component stays declarative.
 */
function useDataTableConfig(
  columns: DataTableColumn[],
  rows: Entity[],
  groupingProp: string[] | undefined,
) {
  const [sorting, setSorting] = useState<SortingState>([]);
  const [grouping, setGrouping] = useGroupingState(groupingProp);
  const tanstackColumns = useTanstackColumns(columns);

  return useReactTable({
    data: rows,
    columns: tanstackColumns,
    state: { sorting, grouping },
    onSortingChange: setSorting,
    onGroupingChange: setGrouping,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
    getGroupedRowModel: getGroupedRowModel(),
    getExpandedRowModel: getExpandedRowModel(),
    enableGrouping: true,
  });
}

/**
 * Compute data-row indices and visible-row count from TanStack flat rows.
 *
 * Maps each visual row index to a data-row index (-1 for group headers).
 * The grid cursor operates on data-row indices, not visual row indices.
 */
function useDataRowIndices(flatRows: Row<Entity>[]) {
  const dataRowIndices = useMemo(() => {
    const indices: number[] = [];
    let di = 0;
    for (const row of flatRows) {
      if (row.getIsGrouped()) {
        indices.push(-1);
      } else {
        indices.push(di++);
      }
    }
    return indices;
  }, [flatRows]);

  const visibleDataRowCount = useMemo(
    () => dataRowIndices.filter((i) => i >= 0).length,
    [dataRowIndices],
  );

  return { dataRowIndices, visibleDataRowCount };
}

// ---------------------------------------------------------------------------
// Header components
// ---------------------------------------------------------------------------

interface HeaderCellProps {
  /** TanStack header object for this column. */
  header: Header<Entity, unknown>;
  /** Zero-based column index within the header group. */
  colIndex: number;
  perspectiveSortMap: SortMap;
  perspectiveId: string | undefined;
  dispatchSortToggle: ReturnType<typeof useDispatchCommand>;
}

/**
 * Render a single column header cell with sort indicators and click handlers.
 *
 * Wraps the `<TableHead>` in a per-header `FocusScope` (with
 * `renderContainer={false}`) so each column header registers as its own
 * spatial-nav target. Without this scope, pressing `k` (up) from a body
 * cell would skip past the header row to whatever sat above the grid
 * (e.g. the perspective bar), because the engine had no intermediate
 * rect to beam-test against.
 *
 * The inner `HeaderCellTh` consumes the scope's `elementRef` via
 * `useFocusScopeElementRef()` and attaches it to the `<TableHead>` —
 * mirroring the `DataTableCellTd` / `RowSelectorTd` pattern. This keeps
 * the table HTML structure valid (no wrapper `<div>` inside `<tr>`) and
 * lets `ResizeObserver` measure the rect for spatial navigation.
 *
 * Supports both perspective-driven and TanStack-native sort toggling,
 * and right-click grouping toggle.
 */
function resolveSortState(
  header: Header<Entity, unknown>,
  perspectiveSortMap: SortMap,
) {
  const columnId = header.column.id;
  const pSort = perspectiveSortMap.get(columnId);
  const isSorted = pSort ? pSort.direction : header.column.getIsSorted();
  const sortPriority = pSort?.priority;
  const showPriority =
    sortPriority !== undefined && perspectiveSortMap.size > 1;
  return { isSorted, sortPriority, showPriority };
}

function buildSortClickHandler(
  header: Header<Entity, unknown>,
  perspectiveId: string | undefined,
  dispatchSortToggle: ReturnType<typeof useDispatchCommand>,
) {
  if (!perspectiveId) return header.column.getToggleSortingHandler();
  const columnId = header.column.id;
  return () => {
    dispatchSortToggle({
      args: { field: columnId, perspective_id: perspectiveId },
    }).catch(console.error);
  };
}

function HeaderCell({
  header,
  colIndex,
  perspectiveSortMap,
  perspectiveId,
  dispatchSortToggle,
}: HeaderCellProps) {
  const columnId = header.column.id;
  const headerMoniker = columnHeaderMoniker(columnId);
  const { isSorted, sortPriority, showPriority } = resolveSortState(
    header,
    perspectiveSortMap,
  );
  const handleClick = buildSortClickHandler(
    header,
    perspectiveId,
    dispatchSortToggle,
  );
  const handleContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    header.column.toggleGrouping();
  };

  return (
    <FocusScope moniker={headerMoniker} commands={[]} renderContainer={false}>
      <HeaderCellTh
        columnId={columnId}
        headerMoniker={headerMoniker}
        colIndex={colIndex}
        width={header.column.getSize() || undefined}
        onClick={handleClick}
        onContextMenu={handleContextMenu}
      >
        <SortIndicator
          columnId={columnId}
          isGrouped={header.column.getIsGrouped()}
          isSorted={isSorted}
          sortPriority={sortPriority}
          showPriority={showPriority}
          headerDef={header.column.columnDef.header}
          headerContext={header.getContext()}
        />
      </HeaderCellTh>
    </FocusScope>
  );
}

interface HeaderCellThProps {
  columnId: string;
  headerMoniker: string;
  colIndex: number;
  width: number | undefined;
  onClick: ((e: React.MouseEvent) => void) | undefined;
  onContextMenu: (e: React.MouseEvent) => void;
  children: React.ReactNode;
}

/**
 * Render the `<th>` for a column header, wiring the enclosing
 * `FocusScope`'s `elementRef` to the `<TableHead>` and focusing the
 * scope on mouse click.
 *
 * Reads `elementRef` from `FocusScopeElementRefContext` (populated by
 * the enclosing `FocusScope` with `renderContainer={false}`) and
 * assigns it to the `<th>` via a ref callback. Attaches
 * `setFocus(headerMoniker)` to `onClickCapture` so spatial focus lands
 * on the header before the sort/group `onClick` handler fires — the
 * capture phase runs first and updates focus state, while the bubble
 * phase still toggles the column's sort order.
 *
 * The `data-table-header-focus` class pulls the left-edge focus bar
 * inside the `<th>` (same rationale as `cell-focus` on data cells —
 * the enclosing `<table>` / view container clips horizontal overflow,
 * so a negative-left bar would never render). See the matching rule
 * in `index.css`.
 */
function HeaderCellTh({
  columnId,
  headerMoniker,
  colIndex,
  width,
  onClick,
  onContextMenu,
  children,
}: HeaderCellThProps) {
  const scopeElementRef = useFocusScopeElementRef();
  const { setFocus } = useEntityFocus();

  const refCallback = useCallback(
    (node: HTMLTableCellElement | null) => {
      if (scopeElementRef) scopeElementRef.current = node;
    },
    [scopeElementRef],
  );

  return (
    <TableHead
      ref={refCallback}
      data-testid={`column-header-${columnId}`}
      data-moniker={headerMoniker}
      className={cn(
        "data-table-header-focus text-left px-3 py-1.5 text-xs font-medium text-muted-foreground uppercase tracking-wide select-none cursor-pointer hover:bg-muted/60 transition-colors h-auto",
        colIndex === 0 && "pl-4",
      )}
      style={width !== undefined ? { width } : undefined}
      onClickCapture={() => setFocus(headerMoniker)}
      onClick={onClick}
      onContextMenu={onContextMenu}
    >
      {children}
    </TableHead>
  );
}

interface SortIndicatorProps {
  columnId: string;
  isGrouped: boolean;
  isSorted: false | "asc" | "desc";
  sortPriority: number | undefined;
  showPriority: boolean;
  headerDef: ColumnDef<Entity>["header"];
  headerContext: HeaderContext<Entity, unknown>;
}

/** Render the column label with sort direction and priority badge. */
function SortIndicator({
  columnId,
  isGrouped,
  isSorted,
  sortPriority,
  showPriority,
  headerDef,
  headerContext,
}: SortIndicatorProps) {
  return (
    <span className="flex items-center gap-1">
      {isGrouped && <ChevronRight className="h-3 w-3 text-primary" />}
      {flexRender(headerDef, headerContext)}
      {isSorted === "asc" && (
        <ArrowUp
          className="h-3 w-3"
          data-testid={`sort-indicator-${columnId}`}
        />
      )}
      {isSorted === "desc" && (
        <ArrowDown
          className="h-3 w-3"
          data-testid={`sort-indicator-${columnId}`}
        />
      )}
      {showPriority && (
        <span
          className="text-[9px] text-muted-foreground/70"
          data-testid={`sort-priority-${columnId}`}
        >
          {sortPriority}
        </span>
      )}
    </span>
  );
}

interface DataTableHeaderProps {
  table: TanTable<Entity>;
  showRowSelector: boolean;
  perspectiveSortMap: SortMap;
  perspectiveId: string | undefined;
  dispatchSortToggle: ReturnType<typeof useDispatchCommand>;
}

/** Render the sticky table header with sortable/groupable column heads. */
function DataTableHeader({
  table,
  showRowSelector,
  perspectiveSortMap,
  perspectiveId,
  dispatchSortToggle,
}: DataTableHeaderProps) {
  return (
    <TableHeader className="sticky top-0 z-[1] bg-muted/80 backdrop-blur-sm">
      {table.getHeaderGroups().map((headerGroup) => (
        <TableRow
          key={headerGroup.id}
          className="border-b border-border hover:bg-transparent"
        >
          {/* Row-selector header is intentionally not wrapped in a
              `FocusScope`: it is an empty spacer cell (no sort, no group,
              no label) whose only job is to align the row-selector
              column width with the body rows. Registering it as a
              spatial target would add a keyboard stop with nothing to
              act on — `h` from the leftmost data header cleanly stays
              put, matching what happens today with `h` from the
              leftmost body cell of the first column when no selector
              exists. */}
          {showRowSelector && (
            <TableHead
              data-testid="row-selector-header"
              className="w-10 px-0 py-1.5 bg-muted/80"
              style={{ width: ROW_SELECTOR_WIDTH }}
            />
          )}
          {headerGroup.headers.map((header, ci) => (
            <HeaderCell
              key={header.id}
              header={header}
              colIndex={ci}
              perspectiveSortMap={perspectiveSortMap}
              perspectiveId={perspectiveId}
              dispatchSortToggle={dispatchSortToggle}
            />
          ))}
        </TableRow>
      ))}
    </TableHeader>
  );
}

// ---------------------------------------------------------------------------
// Body components
// ---------------------------------------------------------------------------

interface GroupHeaderRowProps {
  row: Row<Entity>;
  colSpan: number;
}

/** Expandable group header row that shows the grouping value and subrow count. */
function GroupHeaderRow({ row, colSpan }: GroupHeaderRowProps) {
  return (
    <TableRow
      key={row.id}
      className="border-b border-border/50 bg-muted/20 hover:bg-muted/40 cursor-pointer"
      onClick={() => row.toggleExpanded()}
    >
      <TableCell colSpan={colSpan} className="px-4 py-1.5 font-medium text-sm">
        <span className="flex items-center gap-1.5">
          {row.getIsExpanded() ? (
            <ChevronDown className="h-3.5 w-3.5" />
          ) : (
            <ChevronRight className="h-3.5 w-3.5" />
          )}
          {String(row.groupingValue ?? "\u2014")}
          <span className="text-muted-foreground text-xs">
            ({row.subRows.length})
          </span>
        </span>
      </TableCell>
    </TableRow>
  );
}

interface DataTableCellProps {
  col: DataTableColumn;
  colIndex: number;
  entity: Entity;
  isCursor: boolean;
  isSel: boolean;
  isEditing: boolean;
  cursorRef: React.RefObject<HTMLTableCellElement | null>;
  renderEditor: DataTableProps["renderEditor"];
  grid: UseGridReturn;
  onCellClick: (row: number, col: number) => void;
  dataRowIndex: number;
}

/**
 * Render a single table cell, switching between display and editor mode.
 *
 * Wraps the `<td>` in a per-cell `FocusScope` with
 * `renderContainer={false}` so the scope's `elementRef` is exposed via
 * `FocusScopeElementRefContext`. The inner `DataTableCellTd` consumes
 * that ref and attaches it to the `<td>`, which lets the Rust spatial
 * state track each cell as an independent navigation target (and what
 * makes cell-to-cell `h`/`j`/`k`/`l` navigation work).
 */
function renderCellContent(
  entity: Entity,
  col: DataTableColumn,
  isEditing: boolean,
  renderEditor: DataTableProps["renderEditor"],
  grid: UseGridReturn,
): React.ReactNode {
  if (isEditing && renderEditor) {
    const exit = () => grid.exitEdit();
    return renderEditor(entity, col.field, exit, exit);
  }
  return (
    <Field
      fieldDef={col.field}
      entityType={entity.entity_type}
      entityId={entity.id}
      mode="compact"
      editing={false}
    />
  );
}

function cellEditorNeedsPadding(col: DataTableColumn, isEditing: boolean) {
  return (
    isEditing &&
    col.field.editor !== "color-palette" &&
    col.field.editor !== "select" &&
    col.field.editor !== "multi-select"
  );
}

function DataTableCell({
  col,
  colIndex,
  entity,
  isCursor,
  isSel,
  isEditing,
  cursorRef,
  renderEditor,
  grid,
  onCellClick,
  dataRowIndex,
}: DataTableCellProps) {
  const cellMoniker = fieldMoniker(
    entity.entity_type,
    entity.id,
    col.field.name,
  );
  // Focus bar is painted globally by `[data-focused]::before` in
  // `index.css` — the enclosing `FocusScope`'s `useFocusDecoration`
  // writes the attribute onto the same `<td>`. `cell-focus` pulls the
  // bar inside the cell so it isn't clipped by the enclosing `<tr>`
  // and view container overflow; see the `.cell-focus[data-focused]`
  // override in `index.css`.
  const cellClasses = cn(
    "cell-focus px-3 py-1.5 align-middle max-w-[300px]",
    colIndex === 0 && "pl-4",
    isSel && !isCursor && "bg-primary/10",
    cellEditorNeedsPadding(col, isEditing) && "p-0",
  );

  return (
    <FocusScope
      key={col.field.id}
      moniker={cellMoniker}
      commands={[]}
      renderContainer={false}
    >
      <DataTableCellTd
        cellMoniker={cellMoniker}
        cursorRef={isCursor ? cursorRef : undefined}
        className={cellClasses}
        onClick={() => onCellClick(dataRowIndex, colIndex)}
        onDoubleClick={() => {
          onCellClick(dataRowIndex, colIndex);
          grid.enterEdit();
        }}
      >
        {renderCellContent(entity, col, isEditing, renderEditor, grid)}
      </DataTableCellTd>
    </FocusScope>
  );
}

interface DataTableCellTdProps {
  /** Cell moniker — used for `data-moniker` attribution on the `<td>`. */
  cellMoniker: string;
  /** When defined, the cursor cell attaches this ref so scroll-into-view works. */
  cursorRef: React.RefObject<HTMLTableCellElement | null> | undefined;
  className: string;
  onClick: () => void;
  onDoubleClick: () => void;
  children: React.ReactNode;
}

/**
 * Render the `<td>` for a data cell, wiring up the per-cell
 * `FocusScope` element ref.
 *
 * Reads `elementRef` from `FocusScopeElementRefContext` (populated by
 * the enclosing `FocusScope` with `renderContainer={false}`) and
 * assigns it — along with the optional grid-cursor ref — to the
 * `<td>` via a composite ref callback.
 */
function DataTableCellTd({
  cellMoniker,
  cursorRef,
  className,
  onClick,
  onDoubleClick,
  children,
}: DataTableCellTdProps) {
  const scopeElementRef = useFocusScopeElementRef();

  const refCallback = useCallback(
    (node: HTMLTableCellElement | null) => {
      if (scopeElementRef) scopeElementRef.current = node;
      if (cursorRef) cursorRef.current = node;
    },
    [scopeElementRef, cursorRef],
  );

  return (
    <TableCell
      ref={refCallback}
      data-moniker={cellMoniker}
      className={className}
      onClick={onClick}
      onDoubleClick={onDoubleClick}
    >
      {children}
    </TableCell>
  );
}

interface DataTableRowProps {
  row: Row<Entity>;
  dataRowIndex: number;
  columns: DataTableColumn[];
  grid: UseGridReturn;
  cursorRef: React.RefObject<HTMLTableCellElement | null>;
  showRowSelector: boolean;
  rowEntityCommands: DataTableProps["rowEntityCommands"];
  renderEditor: DataTableProps["renderEditor"];
  isSelected: (row: number, col: number) => boolean;
  handleCellClick: (row: number, col: number) => void;
}

/**
 * Render one data row: FocusScope wrapper, EntityRow, optional selector, and cells.
 *
 * The outer `FocusScope` uses `renderContainer={false}` so no extra DOM
 * element appears between `<tbody>` and `<tr>`, and `spatial={false}`
 * so the row's rect does not participate in the Rust beam-test graph.
 * Excluding the row is what lets cardinal-direction navigation from a
 * cell reach sibling cells directly — the cell rects would otherwise
 * be shadowed by the row's enclosing rect.
 *
 * The row selector and each field cell render their own per-cell
 * `FocusScope`s (see `RowSelector` and `DataTableCell`), which register
 * as individual spatial entries. Those entries are the only grid-level
 * navigation targets.
 */
function renderRowCells({
  columns,
  entity,
  dataRowIndex,
  grid,
  cursorRef,
  renderEditor,
  isSelected,
  handleCellClick,
}: {
  columns: DataTableColumn[];
  entity: Entity;
  dataRowIndex: number;
  grid: UseGridReturn;
  cursorRef: React.RefObject<HTMLTableCellElement | null>;
  renderEditor: DataTableProps["renderEditor"];
  isSelected: (row: number, col: number) => boolean;
  handleCellClick: (row: number, col: number) => void;
}) {
  return columns.map((col, ci) => {
    const isCursor = dataRowIndex === grid.cursor.row && ci === grid.cursor.col;
    return (
      <DataTableCell
        key={col.field.id}
        col={col}
        colIndex={ci}
        entity={entity}
        isCursor={isCursor}
        isSel={isSelected(dataRowIndex, ci)}
        isEditing={isCursor && grid.mode === "edit" && !!renderEditor}
        cursorRef={cursorRef}
        renderEditor={renderEditor}
        grid={grid}
        onCellClick={handleCellClick}
        dataRowIndex={dataRowIndex}
      />
    );
  });
}

function DataTableRow({
  row,
  dataRowIndex,
  columns,
  grid,
  cursorRef,
  showRowSelector,
  rowEntityCommands,
  renderEditor,
  isSelected,
  handleCellClick,
}: DataTableRowProps) {
  const entity = row.original;
  const entityMk = entity.moniker;
  const rowCommands = rowEntityCommands?.(entity) ?? [];
  const isCursorRow = dataRowIndex === grid.cursor.row;

  return (
    <FocusScope
      key={row.id}
      moniker={entityMk}
      commands={rowCommands}
      renderContainer={false}
      spatial={false}
    >
      <EntityRow
        entityMk={entityMk}
        isCursorRow={isCursorRow}
        isEditing={grid.mode === "edit"}
      >
        {showRowSelector && (
          <RowSelector
            entity={entity}
            di={dataRowIndex}
            isCursorRow={isCursorRow}
            onClick={() => handleCellClick(dataRowIndex, grid.cursor.col)}
          />
        )}
        {renderRowCells({
          columns,
          entity,
          dataRowIndex,
          grid,
          cursorRef,
          renderEditor,
          isSelected,
          handleCellClick,
        })}
      </EntityRow>
    </FocusScope>
  );
}

interface DataTableBodyProps {
  flatRows: Row<Entity>[];
  dataRowIndices: number[];
  columns: DataTableColumn[];
  grid: UseGridReturn;
  cursorRef: React.RefObject<HTMLTableCellElement | null>;
  showRowSelector: boolean;
  rowEntityCommands: DataTableProps["rowEntityCommands"];
  renderEditor: DataTableProps["renderEditor"];
  isSelected: (row: number, col: number) => boolean;
  handleCellClick: (row: number, col: number) => void;
}

/** Render the table body, dispatching each row to GroupHeaderRow or DataTableRow. */
function DataTableBody({
  flatRows,
  dataRowIndices,
  columns,
  grid,
  cursorRef,
  showRowSelector,
  rowEntityCommands,
  renderEditor,
  isSelected,
  handleCellClick,
}: DataTableBodyProps) {
  const groupColSpan = showRowSelector ? columns.length + 1 : columns.length;

  return (
    <TableBody>
      {flatRows.map((row, ri) => {
        if (row.getIsGrouped()) {
          return (
            <GroupHeaderRow key={row.id} row={row} colSpan={groupColSpan} />
          );
        }
        return (
          <DataTableRow
            key={row.id}
            row={row}
            dataRowIndex={dataRowIndices[ri]}
            columns={columns}
            grid={grid}
            cursorRef={cursorRef}
            showRowSelector={showRowSelector}
            rowEntityCommands={rowEntityCommands}
            renderEditor={renderEditor}
            isSelected={isSelected}
            handleCellClick={handleCellClick}
          />
        );
      })}
    </TableBody>
  );
}

// ---------------------------------------------------------------------------
// Main orchestrator
// ---------------------------------------------------------------------------

function useIsSelected(grid: UseGridReturn) {
  const selectedRange = grid.getSelectedRange();
  return useCallback(
    (r: number, c: number) => {
      if (!selectedRange) return false;
      return (
        r >= selectedRange.startRow &&
        r <= selectedRange.endRow &&
        c >= selectedRange.startCol &&
        c <= selectedRange.endCol
      );
    },
    [selectedRange],
  );
}

function useCursorScroll(
  cursorRef: React.RefObject<HTMLTableCellElement | null>,
  row: number,
  col: number,
) {
  useEffect(() => {
    cursorRef.current?.scrollIntoView({ block: "nearest", inline: "nearest" });
  }, [cursorRef, row, col]);
}

function useHandleCellClick(onCellClick: DataTableProps["onCellClick"]) {
  return useCallback(
    (r: number, c: number) => {
      onCellClick?.(r, c);
    },
    [onCellClick],
  );
}

function useVisibleRowCountEffect(
  visibleDataRowCount: number,
  onVisibleRowCount: DataTableProps["onVisibleRowCount"],
) {
  useEffect(() => {
    onVisibleRowCount?.(visibleDataRowCount);
  }, [visibleDataRowCount, onVisibleRowCount]);
}

function DataTableEmpty() {
  return (
    <div className="flex-1 flex items-center justify-center text-muted-foreground">
      No rows to display
    </div>
  );
}

/** TanStack react-table wrapper with sorting, grouping, and grid navigation integration. */
export function DataTable({
  columns,
  rows,
  grid,
  onCellClick,
  onRowContextMenu: _onRowContextMenu,
  renderEditor,
  grouping: groupingProp,
  onVisibleRowCount,
  showRowSelector = true,
  rowEntityCommands,
  perspectiveSort,
  perspectiveId,
}: DataTableProps) {
  const tableContainerRef = useRef<HTMLDivElement>(null);
  const cursorRef = useRef<HTMLTableCellElement>(null);
  const dispatchSortToggle = useDispatchCommand("perspective.sort.toggle");
  const perspectiveSortMap = usePerspectiveSortMap(perspectiveSort);
  const table = useDataTableConfig(columns, rows, groupingProp);
  const flatRows = table.getRowModel().rows;
  const { dataRowIndices, visibleDataRowCount } = useDataRowIndices(flatRows);
  const isSelected = useIsSelected(grid);
  const handleCellClick = useHandleCellClick(onCellClick);

  useVisibleRowCountEffect(visibleDataRowCount, onVisibleRowCount);
  useCursorScroll(cursorRef, grid.cursor.row, grid.cursor.col);

  if (flatRows.length === 0) {
    return <DataTableEmpty />;
  }

  return (
    <div ref={tableContainerRef} className="flex-1 overflow-auto min-h-0">
      <Table className="border-collapse text-sm">
        <DataTableHeader
          table={table}
          showRowSelector={showRowSelector}
          perspectiveSortMap={perspectiveSortMap}
          perspectiveId={perspectiveId}
          dispatchSortToggle={dispatchSortToggle}
        />
        <DataTableBody
          flatRows={flatRows}
          dataRowIndices={dataRowIndices}
          columns={columns}
          grid={grid}
          cursorRef={cursorRef}
          showRowSelector={showRowSelector}
          rowEntityCommands={rowEntityCommands}
          renderEditor={renderEditor}
          isSelected={isSelected}
          handleCellClick={handleCellClick}
        />
      </Table>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Row-level helpers (unchanged)
// ---------------------------------------------------------------------------

interface EntityRowProps {
  entityMk: string;
  isCursorRow: boolean;
  isEditing: boolean;
  children: React.ReactNode;
}

/**
 * Table row rendered inside a FocusScope(renderContainer=false).
 *
 * Mirrors FocusScopeInner behavior on a <tr>: click sets entity focus,
 * double-click dispatches ui.inspect, right-click opens context menu.
 * All hooks read from the per-row FocusScope that wraps this component.
 */
function EntityRow({
  entityMk,
  isCursorRow,
  isEditing,
  children,
}: EntityRowProps) {
  const contextMenuHandler = useContextMenu();
  const { setFocus } = useEntityFocus();

  return (
    <TableRow
      data-moniker={entityMk}
      className={cn(
        "border-b border-border/50 transition-colors",
        isCursorRow && !isEditing && "bg-accent/30",
      )}
      onContextMenu={(e) => {
        setFocus(entityMk);
        contextMenuHandler(e);
      }}
    >
      {children}
    </TableRow>
  );
}

interface RowSelectorProps {
  entity: Entity;
  di: number;
  isCursorRow: boolean;
  onClick: () => void;
}

/**
 * Row number selector cell.
 *
 * Wrapped in a per-row `FocusScope` so the selector column is a
 * first-class spatial entry — `h` from the leftmost data cell
 * navigates into this cell, and `l` from here navigates back into
 * the first data cell. The reserved field name `__rowselector` keeps
 * the moniker distinct from any schema field.
 *
 * The `onClick` prop is the grid-cursor side-effect from the parent row
 * (it advances the cursor row so the row's data cells keep their
 * `isCursorRow` highlight). This component composes that callback with
 * `setFocus(selectorMoniker)` so both mouse and keyboard navigation
 * converge on the selector as the focused target — without the
 * composition, clicking the selector would hand focus to whatever cell
 * the grid cursor happens to point at and the selector's focus ring
 * would never paint.
 */
function RowSelector({ entity, di, isCursorRow, onClick }: RowSelectorProps) {
  const { setFocus } = useEntityFocus();
  const selectorMoniker = fieldMoniker(
    entity.entity_type,
    entity.id,
    ROW_SELECTOR_FIELD,
  );
  const handleClick = useCallback(() => {
    onClick();
    setFocus(selectorMoniker);
  }, [onClick, setFocus, selectorMoniker]);
  return (
    <FocusScope moniker={selectorMoniker} commands={[]} renderContainer={false}>
      <RowSelectorTd
        selectorMoniker={selectorMoniker}
        di={di}
        isCursorRow={isCursorRow}
        onClick={handleClick}
      />
    </FocusScope>
  );
}

interface RowSelectorTdProps {
  selectorMoniker: string;
  di: number;
  isCursorRow: boolean;
  onClick: () => void;
}

/**
 * Render the `<td>` for the row selector, wiring the enclosing
 * `FocusScope`'s `elementRef` to the `<td>` so `ResizeObserver` can
 * measure its rect for spatial navigation.
 *
 * The focus bar comes from the enclosing `FocusScope` — it sets
 * `data-focused="true"` on this `<td>` when the selector's moniker is
 * claimed (see `useFocusDecoration` in `focus-scope.tsx`). The global
 * `[data-focused]::before` rule in `index.css` paints the bar, and
 * `cell-focus` pulls it inside the cell so the enclosing `<tr>` /
 * view container overflow doesn't clip it. The row selector is
 * intentionally excluded from the grid's `cellMonikerMap` (the grid
 * cursor is a 2-D coord over data columns only), but the scope-level
 * decoration runs independent of the cursor machinery.
 */
function RowSelectorTd({
  selectorMoniker,
  di,
  isCursorRow,
  onClick,
}: RowSelectorTdProps) {
  const scopeElementRef = useFocusScopeElementRef();
  const refCallback = useCallback(
    (node: HTMLTableCellElement | null) => {
      if (scopeElementRef) scopeElementRef.current = node;
    },
    [scopeElementRef],
  );

  return (
    <TableCell
      ref={refCallback}
      data-testid="row-selector"
      data-moniker={selectorMoniker}
      data-active={isCursorRow ? "true" : "false"}
      className={cn(
        "cell-focus w-10 px-0 py-1.5 text-center cursor-pointer select-none text-[10px] font-medium text-muted-foreground bg-muted/50 border-r border-border/50",
        isCursorRow && "bg-muted text-foreground",
      )}
      style={{ width: ROW_SELECTOR_WIDTH }}
      onClick={onClick}
    >
      {di + 1}
    </TableCell>
  );
}

// ---------------------------------------------------------------------------
// Sort utilities (unchanged)
// ---------------------------------------------------------------------------

/**
 * Resolve the effective sort strategy for a field.
 *
 * Uses the explicit `field.sort` when present (FieldDef.sort is typed as
 * `string | undefined` in kanban.ts), otherwise infers from
 * `field.type.kind` -- mirroring the Rust backend's `effective_sort()`.
 *
 * The kind-based fallback is a pragmatic convention: the backend resolves
 * the same fallback chain, so these two implementations must stay in sync.
 * When `field.sort` is populated by the schema loader, the kind checks
 * are bypassed entirely.
 */
function effectiveSort(field: FieldDef): string {
  if (field.sort) return field.sort;
  switch (field.type.kind) {
    case "date":
      return "datetime";
    case "number":
    case "integer":
      return "numeric";
    case "select":
    case "multi-select":
      return "option-order";
    default:
      return "lexical";
  }
}

/** Compare two field values for sorting, driven by `field.sort` metadata. */
function compareValues(a: unknown, b: unknown, field: FieldDef): number {
  if (a == null && b == null) return 0;
  if (a == null) return 1;
  if (b == null) return -1;

  const sort = effectiveSort(field);
  if (sort === "numeric") {
    return (Number(a) || 0) - (Number(b) || 0);
  }
  if (Array.isArray(a) && Array.isArray(b)) {
    return a.length - b.length;
  }
  return String(a).localeCompare(String(b));
}
