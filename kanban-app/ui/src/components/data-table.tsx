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
import { FocusScope } from "@/components/focus-scope";
import { Field } from "@/components/fields/field";
import type { UseGridReturn } from "@/hooks/use-grid";
import { useEntityFocus } from "@/lib/entity-focus-context";
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

/**
 * Build TanStack column definitions and instantiate the table.
 *
 * Encapsulates column def construction, sorting, grouping, and all
 * TanStack row-model plugins so the main component stays declarative.
 */
function useDataTableConfig(
  columns: DataTableColumn[], rows: Entity[], groupingProp: string[] | undefined,
) {
  const [sorting, setSorting] = useState<SortingState>([]);
  const [grouping, setGrouping] = useState<GroupingState>(groupingProp ?? []);
  // Sync external grouping prop — reset to [] when cleared so the grid
  // reverts to a flat layout instead of retaining stale grouping state.
  useEffect(() => { setGrouping(groupingProp ?? []); }, [groupingProp]);

  const tanstackColumns = useMemo<ColumnDef<Entity>[]>(
    () => columns.map((col) => ({
      id: col.field.name,
      accessorFn: (row: Entity) => row.fields[col.field.name],
      header: col.field.name.replace(/_/g, " "),
      size: col.width,
      cell: ({ row: tanRow }) => {
        const entity = tanRow.original;
        return (
          <Field fieldDef={col.field} entityType={entity.entity_type}
            entityId={entity.id} mode="compact" editing={false} />
        );
      },
      sortingFn: (rowA: Row<Entity>, rowB: Row<Entity>, columnId: string) => {
        return compareValues(rowA.original.fields[columnId], rowB.original.fields[columnId], col.field);
      },
      aggregationFn: "count" as const,
    })),
    [columns],
  );

  return useReactTable({
    data: rows, columns: tanstackColumns,
    state: { sorting, grouping },
    onSortingChange: setSorting, onGroupingChange: setGrouping,
    getCoreRowModel: getCoreRowModel(), getSortedRowModel: getSortedRowModel(),
    getGroupedRowModel: getGroupedRowModel(), getExpandedRowModel: getExpandedRowModel(),
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
 * Supports both perspective-driven and TanStack-native sort toggling,
 * and right-click grouping toggle.
 */
function HeaderCell({
  header, colIndex, perspectiveSortMap, perspectiveId, dispatchSortToggle,
}: HeaderCellProps) {
  const columnId = header.column.id;
  const isGrouped = header.column.getIsGrouped();
  const pSort = perspectiveSortMap.get(columnId);
  const isSorted = pSort ? pSort.direction : header.column.getIsSorted();
  const sortPriority = pSort?.priority;
  const showPriority = sortPriority !== undefined && perspectiveSortMap.size > 1;

  const handleClick = perspectiveId
    ? () => {
        dispatchSortToggle({
          args: { field: columnId, perspective_id: perspectiveId },
        }).catch(console.error);
      }
    : header.column.getToggleSortingHandler();

  const handleContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    header.column.toggleGrouping();
  };

  return (
    <TableHead
      key={header.id}
      data-testid={`column-header-${columnId}`}
      className={cn(
        "text-left px-3 py-1.5 text-xs font-medium text-muted-foreground uppercase tracking-wide select-none cursor-pointer hover:bg-muted/60 transition-colors h-auto",
        colIndex === 0 && "pl-4",
      )}
      style={header.column.getSize() ? { width: header.column.getSize() } : undefined}
      onClick={handleClick}
      onContextMenu={handleContextMenu}
    >
      <SortIndicator
        columnId={columnId}
        isGrouped={isGrouped}
        isSorted={isSorted}
        sortPriority={sortPriority}
        showPriority={showPriority}
        headerDef={header.column.columnDef.header}
        headerContext={header.getContext()}
      />
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
  columnId, isGrouped, isSorted, sortPriority, showPriority, headerDef, headerContext,
}: SortIndicatorProps) {
  return (
    <span className="flex items-center gap-1">
      {isGrouped && <ChevronRight className="h-3 w-3 text-primary" />}
      {flexRender(headerDef, headerContext)}
      {isSorted === "asc" && (
        <ArrowUp className="h-3 w-3" data-testid={`sort-indicator-${columnId}`} />
      )}
      {isSorted === "desc" && (
        <ArrowDown className="h-3 w-3" data-testid={`sort-indicator-${columnId}`} />
      )}
      {showPriority && (
        <span className="text-[9px] text-muted-foreground/70" data-testid={`sort-priority-${columnId}`}>
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
  table, showRowSelector, perspectiveSortMap, perspectiveId, dispatchSortToggle,
}: DataTableHeaderProps) {
  return (
    <TableHeader className="sticky top-0 z-[1] bg-muted/80 backdrop-blur-sm">
      {table.getHeaderGroups().map((headerGroup) => (
        <TableRow key={headerGroup.id} className="border-b border-border hover:bg-transparent">
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
          {row.getIsExpanded()
            ? <ChevronDown className="h-3.5 w-3.5" />
            : <ChevronRight className="h-3.5 w-3.5" />}
          {String(row.groupingValue ?? "\u2014")}
          <span className="text-muted-foreground text-xs">({row.subRows.length})</span>
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

/** Render a single table cell, switching between display and editor mode. */
function DataTableCell({
  col, colIndex, entity, isCursor, isSel, isEditing, cursorRef,
  renderEditor, grid, onCellClick, dataRowIndex,
}: DataTableCellProps) {
  const cellContent = isEditing && renderEditor ? (
    renderEditor(entity, col.field, () => grid.exitEdit(), () => grid.exitEdit())
  ) : (
    <Field
      fieldDef={col.field} entityType={entity.entity_type}
      entityId={entity.id} mode="compact" editing={false}
    />
  );

  const cellClasses = cn(
    "px-3 py-1.5 align-middle max-w-[300px]",
    colIndex === 0 && "pl-4",
    isCursor && "ring-2 ring-primary ring-inset",
    isSel && !isCursor && "bg-primary/10",
    isEditing && col.field.editor !== "color-palette" &&
      col.field.editor !== "select" && col.field.editor !== "multi-select" && "p-0",
  );

  return (
    <TableCell
      key={col.field.id}
      ref={isCursor ? cursorRef : undefined}
      className={cellClasses}
      onClick={() => onCellClick(dataRowIndex, colIndex)}
      onDoubleClick={() => { onCellClick(dataRowIndex, colIndex); grid.enterEdit(); }}
    >
      {cellContent}
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
 * The FocusScope uses renderContainer={false} so no extra DOM element
 * appears between tbody and tr.
 */
function DataTableRow({
  row, dataRowIndex, columns, grid, cursorRef, showRowSelector,
  rowEntityCommands, renderEditor, isSelected, handleCellClick,
}: DataTableRowProps) {
  const entity = row.original;
  const entityMk = entity.moniker;
  const rowCommands = rowEntityCommands?.(entity) ?? [];
  const isCursorRow = dataRowIndex === grid.cursor.row;

  return (
    <FocusScope key={row.id} moniker={entityMk} commands={rowCommands} renderContainer={false}>
      <EntityRow entityMk={entityMk} isCursorRow={isCursorRow} isEditing={grid.mode === "edit"}>
        {showRowSelector && (
          <RowSelector di={dataRowIndex} isCursorRow={isCursorRow}
            onClick={() => handleCellClick(dataRowIndex, grid.cursor.col)} />
        )}
        {columns.map((col, ci) => (
          <DataTableCell
            key={col.field.id} col={col} colIndex={ci} entity={entity}
            isCursor={dataRowIndex === grid.cursor.row && ci === grid.cursor.col}
            isSel={isSelected(dataRowIndex, ci)}
            isEditing={dataRowIndex === grid.cursor.row && ci === grid.cursor.col && grid.mode === "edit" && !!renderEditor}
            cursorRef={cursorRef} renderEditor={renderEditor} grid={grid}
            onCellClick={handleCellClick} dataRowIndex={dataRowIndex}
          />
        ))}
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
  flatRows, dataRowIndices, columns, grid, cursorRef, showRowSelector,
  rowEntityCommands, renderEditor, isSelected, handleCellClick,
}: DataTableBodyProps) {
  const groupColSpan = showRowSelector ? columns.length + 1 : columns.length;

  return (
    <TableBody>
      {flatRows.map((row, ri) => {
        if (row.getIsGrouped()) {
          return <GroupHeaderRow key={row.id} row={row} colSpan={groupColSpan} />;
        }
        return (
          <DataTableRow
            key={row.id} row={row} dataRowIndex={dataRowIndices[ri]}
            columns={columns} grid={grid} cursorRef={cursorRef}
            showRowSelector={showRowSelector} rowEntityCommands={rowEntityCommands}
            renderEditor={renderEditor} isSelected={isSelected}
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

/** TanStack react-table wrapper with sorting, grouping, and grid navigation integration. */
export function DataTable({
  columns, rows, grid, onCellClick, onRowContextMenu: _onRowContextMenu,
  renderEditor, grouping: groupingProp, onVisibleRowCount,
  showRowSelector = true, rowEntityCommands, perspectiveSort, perspectiveId,
}: DataTableProps) {
  const tableContainerRef = useRef<HTMLDivElement>(null);
  const cursorRef = useRef<HTMLTableCellElement>(null);
  const dispatchSortToggle = useDispatchCommand("perspective.sort.toggle");
  const perspectiveSortMap = usePerspectiveSortMap(perspectiveSort);
  const table = useDataTableConfig(columns, rows, groupingProp);
  const flatRows = table.getRowModel().rows;
  const { dataRowIndices, visibleDataRowCount } = useDataRowIndices(flatRows);

  useEffect(() => { onVisibleRowCount?.(visibleDataRowCount); }, [visibleDataRowCount, onVisibleRowCount]);
  useEffect(() => { cursorRef.current?.scrollIntoView({ block: "nearest", inline: "nearest" }); }, [grid.cursor.row, grid.cursor.col]);

  const handleCellClick = useCallback((r: number, c: number) => { onCellClick?.(r, c); }, [onCellClick]);

  const selectedRange = grid.getSelectedRange();
  const isSelected = (r: number, c: number) => {
    if (!selectedRange) return false;
    return r >= selectedRange.startRow && r <= selectedRange.endRow &&
      c >= selectedRange.startCol && c <= selectedRange.endCol;
  };

  if (flatRows.length === 0) {
    return <div className="flex-1 flex items-center justify-center text-muted-foreground">No rows to display</div>;
  }

  return (
    <div ref={tableContainerRef} className="flex-1 overflow-auto min-h-0">
      <Table className="border-collapse text-sm">
        <DataTableHeader
          table={table} showRowSelector={showRowSelector}
          perspectiveSortMap={perspectiveSortMap} perspectiveId={perspectiveId}
          dispatchSortToggle={dispatchSortToggle}
        />
        <DataTableBody
          flatRows={flatRows} dataRowIndices={dataRowIndices} columns={columns}
          grid={grid} cursorRef={cursorRef} showRowSelector={showRowSelector}
          rowEntityCommands={rowEntityCommands} renderEditor={renderEditor}
          isSelected={isSelected} handleCellClick={handleCellClick}
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
  di: number;
  isCursorRow: boolean;
  onClick: () => void;
}

/** Row number selector cell. */
function RowSelector({ di, isCursorRow, onClick }: RowSelectorProps) {
  return (
    <TableCell
      data-testid="row-selector"
      data-active={isCursorRow ? "true" : "false"}
      className={cn(
        "w-10 px-0 py-1.5 text-center cursor-pointer select-none text-[10px] font-medium text-muted-foreground bg-muted/50 border-r border-border/50",
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
