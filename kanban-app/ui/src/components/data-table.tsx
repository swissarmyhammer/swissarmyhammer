import { memo, useRef, useEffect, useCallback, useMemo, useState } from "react";
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
} from "@tanstack/react-table";
import { useVirtualizer, type Virtualizer } from "@tanstack/react-virtual";
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
import { useDispatchCommand } from "@/lib/command-scope";
import { FocusScope } from "@/components/focus-scope";
import { Field } from "@/components/fields/field";
import type { UseGridReturn } from "@/hooks/use-grid";
import {
  useEntityFocus,
  type ClaimPredicate,
} from "@/lib/entity-focus-context";
import { COMPACT_ROW_HEIGHT_PX } from "@/components/fields/displays/compact-cell-wrapper";
import type { Entity, FieldDef, PerspectiveSortEntry } from "@/types/kanban";

/**
 * Body row height in pixels.
 *
 * Body cells use `px-3 py-1.5` (12px combined vertical padding) and render
 * `<Field mode="compact" />` whose displays wrap their output in
 * {@link file://./fields/displays/compact-cell-wrapper.tsx CompactCellWrapper}
 * (24px = `h-6`). 12 + 24 = 36 — the exact height every data row renders at
 * regardless of which fields are populated, empty, or contain CM6 mention
 * pills / avatars.
 *
 * Sourced from {@link COMPACT_ROW_HEIGHT_PX} so the wrapper class and the
 * virtualizer's fixed-height estimate stay in lock-step automatically;
 * changing one without the other used to silently reintroduce the
 * row-jitter the wrapper was added to fix.
 */
const ROW_HEIGHT = COMPACT_ROW_HEIGHT_PX;

/**
 * Overscan rows rendered above and below the visible window.
 *
 * Mirrors `column-view.tsx::VirtualColumn` so card-grid and table-grid
 * virtualization stay in lockstep. Five rows on each side keeps fast
 * keyboard / wheel scrolling visually seamless without paying for many
 * off-screen renders.
 */
const ROW_OVERSCAN = 5;

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
   * When provided together with claimPredicates, each cell is wrapped
   * in a FocusScope for pull-based navigation.
   */
  cellMonikers?: string[][];
  /**
   * 2D array of claim predicates: claimPredicates[row][col] = ClaimPredicate[].
   * Must match dimensions of cellMonikers.
   */
  claimPredicates?: ClaimPredicate[][][];
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
   * Active perspective sort entries. When provided, column headers show
   * sort indicators and dispatch `perspective.sort.toggle` on click.
   */
  perspectiveSort?: readonly PerspectiveSortEntry[];
  /** Active perspective ID — required for dispatching sort commands. */
  perspectiveId?: string;
  /**
   * Context-menu handler for the outer scroll container. Right-clicking
   * whitespace between rows or below the last row fires this handler so
   * view-scoped commands (e.g. `entity.add:{type}`) surface even when the
   * click doesn't land on a row. Per-row context menus stop propagation
   * via `useContextMenu`, so this handler only fires from non-row areas.
   */
  onContainerContextMenu?: (e: React.MouseEvent) => void;
}

export function DataTable(props: DataTableProps) {
  const cursorRef = useRef<HTMLTableCellElement>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const { table, perspectiveSortMap, dataRowIndices, handleCellClick } =
    useDataTableState(props, cursorRef);

  const flatRows = table.getRowModel().rows;
  const useClaimNav =
    props.cellMonikers !== undefined && props.claimPredicates !== undefined;

  // Virtualize body rows. Header is a CSS-sticky `<thead>` outside the
  // virtualized list so it continues to pin to the scroll container's
  // top edge during scroll without needing virtualization itself.
  const virtualizer = useVirtualizer({
    count: flatRows.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: ROW_OVERSCAN,
  });

  // Translate the cursor's data-row index into a flat-row index and
  // scroll the virtualizer to that row whenever the cursor moves
  // vertically. Without this, moving the cursor past the visible
  // window leaves the cursor row unmounted -- `cursorRef.scrollIntoView`
  // (the existing horizontal-scroll handler in `useDataTableState`)
  // can't scroll a node that does not exist.
  useCursorRowScroll(virtualizer, dataRowIndices, props.grid.cursor.row);

  const virtualItems = virtualizer.getVirtualItems();
  const totalSize = virtualizer.getTotalSize();
  const paddingTop = virtualItems.length > 0 ? virtualItems[0].start : 0;
  const paddingBottom =
    virtualItems.length > 0
      ? totalSize - virtualItems[virtualItems.length - 1].end
      : 0;

  return (
    <div
      ref={scrollRef}
      className="flex-1 overflow-auto min-h-0"
      onContextMenu={props.onContainerContextMenu}
    >
      <Table className="border-collapse text-sm">
        <TableHeader className="sticky top-0 z-[1] bg-muted/80 backdrop-blur-sm">
          {table.getHeaderGroups().map((headerGroup) => (
            <DataTableHeaderRow
              key={headerGroup.id}
              headerGroup={headerGroup}
              showRowSelector={props.showRowSelector ?? true}
              perspectiveSortMap={perspectiveSortMap}
              perspectiveId={props.perspectiveId}
            />
          ))}
        </TableHeader>
        <TableBody>
          {paddingTop > 0 && <PaddingRow height={paddingTop} />}
          {virtualItems.map((vi) => {
            const row = flatRows[vi.index];
            return (
              <DataTableRow
                key={row.id}
                row={row}
                ri={vi.index}
                columns={props.columns}
                grid={props.grid}
                dataRowIndices={dataRowIndices}
                showRowSelector={props.showRowSelector ?? true}
                useClaimNav={useClaimNav}
                cellMonikers={props.cellMonikers}
                claimPredicates={props.claimPredicates}
                handleCellClick={handleCellClick}
                renderEditor={props.renderEditor}
                onCellClick={props.onCellClick}
                cursorRef={cursorRef}
              />
            );
          })}
          {paddingBottom > 0 && <PaddingRow height={paddingBottom} />}
        </TableBody>
      </Table>
    </div>
  );
}

interface PaddingRowProps {
  height: number;
}

/**
 * Empty `<tr>` spacer that reserves vertical space above or below the
 * virtualized window of body rows.
 *
 * The padding-row pattern is the standard way to virtualize inside a
 * `<table>` element while keeping browser table layout intact: two
 * blank rows (top + bottom) absorb the scroll-extent that the unmounted
 * rows would otherwise occupy, so the scrollbar reflects the full row
 * count and `scrollToIndex` lands at the right pixel.
 *
 * `pointer-events: none` prevents the spacer (which can be hundreds of
 * pixels tall on long lists) from swallowing right-click events that
 * the outer scroll container's `onContainerContextMenu` handler relies
 * on for view-scoped commands like "New Task" on whitespace.
 */
function PaddingRow({ height }: PaddingRowProps) {
  return (
    <tr
      aria-hidden="true"
      data-virtual-padding="true"
      style={{ height, pointerEvents: "none" }}
    />
  );
}

/**
 * Build an inverse map from data-row index to flat-row index.
 *
 * `dataRowIndices` maps `flatRowIndex -> dataRowIndex` (with `-1` for
 * group-header rows). The virtualizer indexes into the flat row list,
 * but the grid cursor uses data-row indices -- so to call
 * `virtualizer.scrollToIndex(...)` for a cursor move we need the
 * inverse: `dataRowIndex -> flatRowIndex`.
 */
function useDataRowToFlatRow(dataRowIndices: number[]): number[] {
  return useMemo(() => {
    const inverse: number[] = [];
    for (let fi = 0; fi < dataRowIndices.length; fi++) {
      const di = dataRowIndices[fi];
      if (di >= 0) inverse[di] = fi;
    }
    return inverse;
  }, [dataRowIndices]);
}

/**
 * Scroll the virtualizer to the cursor row when the cursor moves
 * outside the currently mounted virtual window.
 *
 * Runs only when the data-row index changes -- horizontal cursor moves
 * (column changes) do not trigger a virtualizer scroll because they
 * cannot move the cursor outside the vertical viewport. The existing
 * `cursorRef.scrollIntoView` effect in `useDataTableState` handles
 * inline (horizontal) scrolling once the row remounts.
 *
 * Skips the initial mount: on first render the cursor's previous row
 * is undefined, so there is nothing to scroll *to* (the user has not
 * yet moved). This also prevents an unconditional `scrollToIndex` on
 * every component mount, which would fire a React state update inside
 * the virtualizer outside any pending `act()` scope and produce a
 * spurious warning in test runs.
 *
 * Bails out early when the cursor row is already within the mounted
 * virtual window -- intra-window cursor moves are handled entirely by
 * the existing `cursorRef.scrollIntoView` effect.
 */
function useCursorRowScroll(
  virtualizer: Virtualizer<HTMLDivElement, Element>,
  dataRowIndices: number[],
  cursorRow: number,
) {
  const dataRowToFlatRow = useDataRowToFlatRow(dataRowIndices);
  const previousCursorRow = useRef<number | null>(null);
  useEffect(() => {
    const previous = previousCursorRow.current;
    previousCursorRow.current = cursorRow;
    // Skip initial mount -- nothing to scroll to until the cursor moves.
    if (previous === null) return;
    if (previous === cursorRow) return;

    const flatIndex = dataRowToFlatRow[cursorRow];
    if (flatIndex === undefined) return;
    const visible = virtualizer.getVirtualItems();
    if (visible.length === 0) return;
    const firstVisible = visible[0].index;
    const lastVisible = visible[visible.length - 1].index;
    if (flatIndex >= firstVisible && flatIndex <= lastVisible) return;
    virtualizer.scrollToIndex(flatIndex, { align: "auto" });
  }, [virtualizer, dataRowToFlatRow, cursorRow]);
}

/** Bundle all the table-state hooks the renderer needs. */
function useDataTableState(
  props: DataTableProps,
  cursorRef: React.RefObject<HTMLTableCellElement | null>,
) {
  const {
    columns,
    rows,
    grid,
    grouping: groupingProp,
    onVisibleRowCount,
    perspectiveSort,
    onCellClick,
  } = props;
  const [sorting, setSorting] = useState<SortingState>([]);
  const [grouping, setGrouping] = useState<GroupingState>(groupingProp ?? []);

  // Sync external grouping prop — reset to [] when cleared so the grid
  // reverts to a flat layout instead of retaining stale grouping state.
  useEffect(() => {
    setGrouping(groupingProp ?? []);
  }, [groupingProp]);

  const perspectiveSortMap = usePerspectiveSortMap(perspectiveSort);
  const tanstackColumns = useTanstackColumns(columns);

  const table = useReactTable({
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

  const flatRows = table.getRowModel().rows;
  const dataRowIndices = useDataRowIndices(flatRows);
  const visibleDataRowCount = useMemo(
    () => dataRowIndices.filter((i) => i >= 0).length,
    [dataRowIndices],
  );
  useEffect(() => {
    onVisibleRowCount?.(visibleDataRowCount);
  }, [visibleDataRowCount, onVisibleRowCount]);

  useEffect(() => {
    cursorRef.current?.scrollIntoView({ block: "nearest", inline: "nearest" });
  }, [grid.cursor.row, grid.cursor.col, cursorRef]);

  const handleCellClick = useCallback(
    (row: number, col: number) => {
      onCellClick?.(row, col);
    },
    [onCellClick],
  );

  return { table, perspectiveSortMap, dataRowIndices, handleCellClick };
}

/** Build the per-field-name lookup of `{ direction, priority }`. */
function usePerspectiveSortMap(
  perspectiveSort: readonly PerspectiveSortEntry[] | undefined,
) {
  return useMemo(() => {
    const map = new Map<
      string,
      { direction: "asc" | "desc"; priority: number }
    >();
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

/** Build TanStack column definitions from our field-based columns. */
function useTanstackColumns(columns: DataTableColumn[]) {
  return useMemo<ColumnDef<Entity>[]>(
    () =>
      columns.map((col) => ({
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
          const a = rowA.original.fields[columnId];
          const b = rowB.original.fields[columnId];
          return compareValues(a, b, col.field);
        },
        aggregationFn: "count" as const,
      })),
    [columns],
  );
}

/** Map each flat-row index to a data-row index (-1 for group headers). */
function useDataRowIndices(flatRows: Row<Entity>[]) {
  return useMemo(() => {
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
}

interface DataTableHeaderRowProps {
  headerGroup: ReturnType<
    ReturnType<typeof useReactTable<Entity>>["getHeaderGroups"]
  >[number];
  showRowSelector: boolean;
  perspectiveSortMap: Map<
    string,
    { direction: "asc" | "desc"; priority: number }
  >;
  perspectiveId: string | undefined;
}

/** One header row containing the selector cell (optional) and all field headers. */
function DataTableHeaderRow({
  headerGroup,
  showRowSelector,
  perspectiveSortMap,
  perspectiveId,
}: DataTableHeaderRowProps) {
  return (
    <TableRow className="border-b border-border hover:bg-transparent">
      {showRowSelector && (
        <TableHead
          data-testid="row-selector-header"
          className="w-10 px-0 py-1.5 bg-muted/80"
          style={{ width: 40 }}
        />
      )}
      {headerGroup.headers.map((header, ci) => (
        <DataTableHeaderCell
          key={header.id}
          header={header}
          ci={ci}
          perspectiveSortMap={perspectiveSortMap}
          perspectiveId={perspectiveId}
        />
      ))}
    </TableRow>
  );
}

interface DataTableHeaderCellProps {
  header: ReturnType<
    ReturnType<typeof useReactTable<Entity>>["getHeaderGroups"]
  >[number]["headers"][number];
  ci: number;
  perspectiveSortMap: Map<
    string,
    { direction: "asc" | "desc"; priority: number }
  >;
  perspectiveId: string | undefined;
}

/**
 * One TableHead cell. Left-click dispatches a sort toggle (when a perspective
 * is active) or falls back to TanStack's built-in toggle; right-click toggles
 * column grouping and stops propagation so the outer container's view-scoped
 * context menu does not also fire.
 */
function DataTableHeaderCell({
  header,
  ci,
  perspectiveSortMap,
  perspectiveId,
}: DataTableHeaderCellProps) {
  const dispatchSortToggle = useDispatchCommand("perspective.sort.toggle");
  const columnId = header.column.id;
  const isGrouped = header.column.getIsGrouped();
  const pSort = perspectiveSortMap.get(columnId);
  const isSorted = pSort ? pSort.direction : header.column.getIsSorted();
  const sortPriority = pSort?.priority;
  const showPriority =
    sortPriority !== undefined && perspectiveSortMap.size > 1;

  const handleHeaderClick = perspectiveId
    ? () => {
        dispatchSortToggle({
          args: { field: columnId, perspective_id: perspectiveId },
        }).catch(console.error);
      }
    : header.column.getToggleSortingHandler();

  // Right-clicking a column header toggles grouping. Must stopPropagation so
  // the contextmenu event does NOT bubble to the outer scroll container's
  // `onContainerContextMenu` — otherwise the header toggle would fire
  // alongside the view-scoped native menu ("New <EntityType>" etc.).
  const handleHeaderContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    header.column.toggleGrouping();
  };

  return (
    <TableHead
      data-testid={`column-header-${columnId}`}
      className={cn(
        "text-left px-3 py-1.5 text-xs font-medium text-muted-foreground uppercase tracking-wide select-none cursor-pointer hover:bg-muted/60 transition-colors h-auto",
        ci === 0 && "pl-4",
      )}
      style={
        header.column.getSize() ? { width: header.column.getSize() } : undefined
      }
      onClick={handleHeaderClick}
      onContextMenu={handleHeaderContextMenu}
    >
      <span className="flex items-center gap-1">
        {isGrouped && <ChevronRight className="h-3 w-3 text-primary" />}
        {flexRender(header.column.columnDef.header, header.getContext())}
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
    </TableHead>
  );
}

interface DataTableRowProps {
  row: Row<Entity>;
  ri: number;
  columns: DataTableColumn[];
  grid: UseGridReturn;
  dataRowIndices: number[];
  showRowSelector: boolean;
  useClaimNav: boolean;
  cellMonikers: string[][] | undefined;
  claimPredicates: ClaimPredicate[][][] | undefined;
  handleCellClick: (row: number, col: number) => void;
  renderEditor: DataTableProps["renderEditor"];
  onCellClick: DataTableProps["onCellClick"];
  cursorRef: React.RefObject<HTMLTableCellElement | null>;
}

/**
 * Render one data or group-header row.
 *
 * `React.memo` so the ~2000 sibling rows don't re-render when GridView
 * re-renders with identical per-row state (same grid cursor, same mode,
 * same row data). Initial mount on a 2000-row grid used to fire 4x
 * (strict-mode + focus/cursor init) × 2000 = 8000 row renders; with memo
 * the three repeat renders skip the subtree when props are shallow-equal.
 */
const DataTableRow = memo(function DataTableRowImpl(props: DataTableRowProps) {
  const { row, ri, columns, grid, dataRowIndices, showRowSelector } = props;
  if (row.getIsGrouped()) {
    return (
      <GroupHeaderRow
        row={row}
        columns={columns}
        showRowSelector={showRowSelector}
      />
    );
  }
  const di = dataRowIndices[ri];
  const entity = row.original;
  const entityMk = entity.moniker;
  return (
    <FocusScope moniker={entityMk} renderContainer={false}>
      <EntityRow
        entityMk={entityMk}
        isCursorRow={di === grid.cursor.row}
        isEditing={grid.mode === "edit"}
      >
        {showRowSelector && (
          <RowSelector
            di={di}
            isCursorRow={di === grid.cursor.row}
            onClick={() => props.handleCellClick(di, grid.cursor.col)}
          />
        )}
        {columns.map((col, ci) => (
          <DataBodyCell
            key={col.field.id}
            di={di}
            ci={ci}
            col={col}
            entity={entity}
            grid={grid}
            useClaimNav={props.useClaimNav}
            cellMonikers={props.cellMonikers}
            claimPredicates={props.claimPredicates}
            handleCellClick={props.handleCellClick}
            renderEditor={props.renderEditor}
            onCellClick={props.onCellClick}
            cursorRef={props.cursorRef}
          />
        ))}
      </EntityRow>
    </FocusScope>
  );
});

interface GroupHeaderRowProps {
  row: Row<Entity>;
  columns: DataTableColumn[];
  showRowSelector: boolean;
}

/** Row rendering the "— (N)" collapsible header for a TanStack group. */
function GroupHeaderRow({
  row,
  columns,
  showRowSelector,
}: GroupHeaderRowProps) {
  const groupColSpan = showRowSelector ? columns.length + 1 : columns.length;
  return (
    <TableRow
      className="border-b border-border/50 bg-muted/20 hover:bg-muted/40 cursor-pointer"
      onClick={() => row.toggleExpanded()}
    >
      <TableCell
        colSpan={groupColSpan}
        className="px-4 py-1.5 font-medium text-sm"
      >
        <span className="flex items-center gap-1.5">
          {row.getIsExpanded() ? (
            <ChevronDown className="h-3.5 w-3.5" />
          ) : (
            <ChevronRight className="h-3.5 w-3.5" />
          )}
          {String(row.groupingValue ?? "—")}
          <span className="text-muted-foreground text-xs">
            ({row.subRows.length})
          </span>
        </span>
      </TableCell>
    </TableRow>
  );
}

interface DataBodyCellProps {
  di: number;
  ci: number;
  col: DataTableColumn;
  entity: Entity;
  grid: UseGridReturn;
  useClaimNav: boolean;
  cellMonikers: string[][] | undefined;
  claimPredicates: ClaimPredicate[][][] | undefined;
  handleCellClick: (row: number, col: number) => void;
  renderEditor: DataTableProps["renderEditor"];
  onCellClick: DataTableProps["onCellClick"];
  cursorRef: React.RefObject<HTMLTableCellElement | null>;
}

/**
 * One data cell.
 *
 * Memoized so that when GridView re-renders with identical per-cell state,
 * the ~12k sibling cells skip their render entirely. React.memo's default
 * shallow comparison treats stable useMemo/useCallback/ref props as equal,
 * so only cells whose props actually changed re-render.
 */
const DataBodyCell = memo(function DataBodyCellImpl(props: DataBodyCellProps) {
  const { di, ci, col, entity, grid, renderEditor, onCellClick } = props;
  const isCursor = di === grid.cursor.row && ci === grid.cursor.col;
  const isSel = isCellSelected(grid, di, ci);
  const isEditing = isCursor && grid.mode === "edit" && renderEditor;
  const cellContent = isEditing ? (
    renderEditor!(
      entity,
      col.field,
      () => grid.exitEdit(),
      () => grid.exitEdit(),
    )
  ) : (
    <Field
      fieldDef={col.field}
      entityType={entity.entity_type}
      entityId={entity.id}
      mode="compact"
      editing={false}
    />
  );
  const cellClasses = cn(
    "px-3 py-1.5 align-middle max-w-[300px]",
    ci === 0 && "pl-4",
    isCursor && "ring-2 ring-primary ring-inset",
    isSel && !isCursor && "bg-primary/10",
    // Strip cell padding during editing for editors that fill the entire cell.
    isEditing &&
      col.field.editor !== "color-palette" &&
      col.field.editor !== "select" &&
      col.field.editor !== "multi-select" &&
      "p-0",
  );

  if (props.useClaimNav) {
    const mk = props.cellMonikers?.[di]?.[ci];
    const preds = props.claimPredicates?.[di]?.[ci];
    if (mk && preds) {
      return (
        <GridCellScope
          moniker={mk}
          claimWhen={preds}
          isCursor={isCursor}
          cursorRef={isCursor ? props.cursorRef : undefined}
          className={cellClasses}
          onClick={() => props.handleCellClick(di, ci)}
          onDoubleClick={() => {
            onCellClick?.(di, ci);
            grid.enterEdit();
          }}
        >
          {cellContent}
        </GridCellScope>
      );
    }
  }
  return (
    <TableCell
      ref={isCursor ? props.cursorRef : undefined}
      className={cellClasses}
      onClick={() => props.handleCellClick(di, ci)}
      onDoubleClick={() => {
        onCellClick?.(di, ci);
        grid.enterEdit();
      }}
    >
      {cellContent}
    </TableCell>
  );
});

/** Is cell (row,col) inside the current grid selection range? */
function isCellSelected(
  grid: UseGridReturn,
  row: number,
  col: number,
): boolean {
  const r = grid.getSelectedRange();
  if (!r) return false;
  return (
    row >= r.startRow && row <= r.endRow && col >= r.startCol && col <= r.endCol
  );
}

interface GridCellScopeProps {
  moniker: string;
  claimWhen: ClaimPredicate[];
  isCursor: boolean;
  cursorRef?: React.Ref<HTMLTableCellElement>;
  className?: string;
  onClick: () => void;
  onDoubleClick: () => void;
  children: React.ReactNode;
}

/**
 * Wraps a grid cell in a FocusScope with claimWhen predicates.
 *
 * The FocusScope renders as a <td> element (via the underlying FocusHighlight).
 * This component bridges the FocusScope's div-based rendering with the table
 * structure by wrapping FocusScope inside a TableCell.
 *
 * @param moniker - Cell moniker (entityType:entityId.fieldName)
 * @param claimWhen - Predicates for pull-based navigation
 * @param isCursor - Whether this cell is the current cursor position
 * @param cursorRef - Ref to attach for scroll-into-view
 * @param className - CSS classes for the cell
 * @param onClick - Click handler
 * @param onDoubleClick - Double-click handler
 */
function GridCellScope({
  moniker,
  claimWhen,
  isCursor: _isCursor,
  cursorRef,
  className,
  onClick,
  onDoubleClick,
  children,
}: GridCellScopeProps) {
  return (
    <TableCell
      ref={cursorRef}
      className={className}
      onClick={onClick}
      onDoubleClick={onDoubleClick}
    >
      <FocusScope moniker={moniker} claimWhen={claimWhen} showFocusBar={false}>
        {children}
      </FocusScope>
    </TableCell>
  );
}

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
      style={{ width: 40 }}
      onClick={onClick}
    >
      {di + 1}
    </TableCell>
  );
}

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
