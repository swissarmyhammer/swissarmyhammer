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
import { CommandScopeProvider, type CommandDef } from "@/lib/command-scope";
import { Field } from "@/components/fields/field";
import type { UseGridReturn } from "@/hooks/use-grid";
import type { Entity, FieldDef } from "@/types/kanban";

export interface DataTableColumn {
  field: FieldDef;
  width?: number;
}

interface DataTableProps {
  columns: DataTableColumn[];
  rows: Entity[];
  grid: UseGridReturn;
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
   *
   * When provided, each row's selector cell is wrapped in a per-row
   * CommandScopeProvider so that right-clicking row N always resolves
   * commands for row N's entity — regardless of the grid cursor position.
   *
   * This eliminates the race between `grid.setCursor()` (async state update)
   * and `contextMenuHandler(e)` (synchronous) when the user right-clicks a
   * row that isn't the current cursor row.
   */
  rowEntityCommands?: (entity: Entity) => CommandDef[];
}

export function DataTable({
  columns,
  rows,
  grid,
  onCellClick,
  onRowContextMenu,
  renderEditor,
  grouping: groupingProp,
  onVisibleRowCount,
  showRowSelector = true,
  rowEntityCommands,
}: DataTableProps) {
  const tableContainerRef = useRef<HTMLDivElement>(null);
  const cursorRef = useRef<HTMLTableCellElement>(null);
  // Grid-level context menu handler — used when rowEntityCommands is not set.
  const contextMenuHandler = useContextMenu();
  const [sorting, setSorting] = useState<SortingState>([]);
  const [grouping, setGrouping] = useState<GroupingState>(groupingProp ?? []);

  // Sync external grouping prop
  useEffect(() => {
    if (groupingProp) setGrouping(groupingProp);
  }, [groupingProp]);

  // Build TanStack column definitions from our field-based columns
  const tanstackColumns = useMemo<ColumnDef<Entity>[]>(
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

  // Map each flatRow index to a data-row index (-1 for group headers).
  // The grid cursor operates on data-row indices, not visual row indices.
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

  // Report visible data row count changes to parent
  useEffect(() => {
    onVisibleRowCount?.(visibleDataRowCount);
  }, [visibleDataRowCount, onVisibleRowCount]);

  // Scroll the active cell into view
  useEffect(() => {
    cursorRef.current?.scrollIntoView({ block: "nearest", inline: "nearest" });
  }, [grid.cursor.row, grid.cursor.col]);

  const handleCellClick = useCallback(
    (row: number, col: number) => {
      grid.setCursor(row, col);
      onCellClick?.(row, col);
    },
    [grid, onCellClick],
  );

  const selectedRange = grid.getSelectedRange();
  const isSelected = (row: number, col: number) => {
    if (!selectedRange) return false;
    return (
      row >= selectedRange.startRow &&
      row <= selectedRange.endRow &&
      col >= selectedRange.startCol &&
      col <= selectedRange.endCol
    );
  };

  if (flatRows.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center text-muted-foreground">
        No rows to display
      </div>
    );
  }

  return (
    <div ref={tableContainerRef} className="flex-1 overflow-auto min-h-0">
      <Table className="border-collapse text-sm">
        <TableHeader className="sticky top-0 z-[1] bg-muted/80 backdrop-blur-sm">
          {table.getHeaderGroups().map((headerGroup) => (
            <TableRow
              key={headerGroup.id}
              className="border-b border-border hover:bg-transparent"
            >
              {showRowSelector && (
                <TableHead
                  data-testid="row-selector-header"
                  className="w-10 px-0 py-1.5 bg-muted/80"
                  style={{ width: 40 }}
                />
              )}
              {headerGroup.headers.map((header, ci) => {
                const isSorted = header.column.getIsSorted();
                const isGrouped = header.column.getIsGrouped();
                return (
                  <TableHead
                    key={header.id}
                    className={cn(
                      "text-left px-3 py-1.5 text-xs font-medium text-muted-foreground uppercase tracking-wide select-none cursor-pointer hover:bg-muted/60 transition-colors h-auto",
                      ci === 0 && "pl-4",
                    )}
                    style={
                      header.column.getSize()
                        ? { width: header.column.getSize() }
                        : undefined
                    }
                    onClick={header.column.getToggleSortingHandler()}
                    onContextMenu={(e) => {
                      e.preventDefault();
                      header.column.toggleGrouping();
                    }}
                  >
                    <span className="flex items-center gap-1">
                      {isGrouped && (
                        <ChevronRight className="h-3 w-3 text-primary" />
                      )}
                      {flexRender(
                        header.column.columnDef.header,
                        header.getContext(),
                      )}
                      {isSorted === "asc" && <ArrowUp className="h-3 w-3" />}
                      {isSorted === "desc" && <ArrowDown className="h-3 w-3" />}
                    </span>
                  </TableHead>
                );
              })}
            </TableRow>
          ))}
        </TableHeader>
        <TableBody>
          {flatRows.map((row, ri) => {
            // Group header rows — not part of grid cursor navigation
            if (row.getIsGrouped()) {
              // colSpan covers all field columns plus the selector column when visible
              const groupColSpan = showRowSelector
                ? columns.length + 1
                : columns.length;
              return (
                <TableRow
                  key={row.id}
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

            // Data row — use data-row index (di) for grid cursor, not visual index (ri)
            const di = dataRowIndices[ri];
            const entity = row.original;
            return (
              <TableRow
                key={row.id}
                className={cn(
                  "border-b border-border/50 transition-colors",
                  di === grid.cursor.row &&
                    grid.mode !== "edit" &&
                    "bg-accent/30",
                )}
                onContextMenu={(e) => {
                  grid.setCursor(di, grid.cursor.col);
                  onRowContextMenu?.(entity, e);
                  contextMenuHandler(e);
                }}
              >
                {showRowSelector &&
                  (rowEntityCommands ? (
                    <RowSelectorWithScope
                      entity={entity}
                      di={di}
                      cursorRow={grid.cursor.row}
                      cursorCol={grid.cursor.col}
                      commands={rowEntityCommands(entity)}
                      onCellClick={handleCellClick}
                      onRowContextMenu={onRowContextMenu}
                    />
                  ) : (
                    <TableCell
                      data-testid="row-selector"
                      data-active={di === grid.cursor.row ? "true" : "false"}
                      className={cn(
                        "w-10 px-0 py-1.5 text-center cursor-pointer select-none text-[10px] font-medium text-muted-foreground bg-muted/50 border-r border-border/50",
                        di === grid.cursor.row && "bg-muted text-foreground",
                      )}
                      style={{ width: 40 }}
                      onClick={() => handleCellClick(di, grid.cursor.col)}
                      onContextMenu={(e) => {
                        grid.setCursor(di, grid.cursor.col);
                        onRowContextMenu?.(entity, e);
                        contextMenuHandler(e);
                      }}
                    >
                      {di + 1}
                    </TableCell>
                  ))}
                {columns.map((col, ci) => {
                  const isCursor =
                    di === grid.cursor.row && ci === grid.cursor.col;
                  const isSel = isSelected(di, ci);
                  const isEditing =
                    isCursor && grid.mode === "edit" && renderEditor;
                  return (
                    <TableCell
                      key={col.field.id}
                      ref={isCursor ? cursorRef : undefined}
                      className={cn(
                        "px-3 py-1.5 align-middle max-w-[300px]",
                        ci === 0 && "pl-4",
                        isCursor && "ring-2 ring-primary ring-inset",
                        isSel && !isCursor && "bg-primary/10",
                        isEditing &&
                          col.field.editor !== "color-palette" &&
                          col.field.editor !== "select" &&
                          col.field.editor !== "multi-select" &&
                          "p-0",
                      )}
                      onClick={() => handleCellClick(di, ci)}
                      onDoubleClick={() => {
                        grid.setCursor(di, ci);
                        grid.enterEdit();
                      }}
                    >
                      {isEditing ? (
                        renderEditor(
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
                      )}
                    </TableCell>
                  );
                })}
              </TableRow>
            );
          })}
        </TableBody>
      </Table>
    </div>
  );
}

interface RowSelectorWithScopeProps {
  entity: Entity;
  di: number;
  cursorRow: number;
  cursorCol: number;
  commands: CommandDef[];
  onCellClick: (row: number, col: number) => void;
  onRowContextMenu?: (entity: Entity, e: React.MouseEvent) => void;
}

/**
 * Renders a row selector cell wrapped in a per-row CommandScopeProvider.
 *
 * This component exists so that `useContextMenu()` is called from inside the
 * row-specific scope. When the user right-clicks this cell, the context menu
 * resolves commands from `commands` (built for this row's entity) rather than
 * from the grid-level scope — eliminating the race between `grid.setCursor()`
 * (async state update) and the synchronous context menu open.
 */
function RowSelectorWithScope({
  entity,
  di,
  cursorRow,
  cursorCol,
  commands,
  onCellClick,
  onRowContextMenu,
}: RowSelectorWithScopeProps) {
  return (
    <CommandScopeProvider commands={commands}>
      <RowSelectorCell
        entity={entity}
        di={di}
        cursorRow={cursorRow}
        cursorCol={cursorCol}
        onCellClick={onCellClick}
        onRowContextMenu={onRowContextMenu}
      />
    </CommandScopeProvider>
  );
}

interface RowSelectorCellProps {
  entity: Entity;
  di: number;
  cursorRow: number;
  cursorCol: number;
  onCellClick: (row: number, col: number) => void;
  onRowContextMenu?: (entity: Entity, e: React.MouseEvent) => void;
}

/**
 * The inner selector cell rendered inside the per-row CommandScopeProvider.
 *
 * Calls `useContextMenu()` here so the hook reads from the row-specific scope,
 * not the grid-level scope. The `data-row-entity-id` attribute on the wrapping
 * `<td>` enables tests to verify which entity's scope is active per row.
 */
function RowSelectorCell({
  entity,
  di,
  cursorRow,
  cursorCol,
  onCellClick,
  onRowContextMenu,
}: RowSelectorCellProps) {
  const contextMenuHandler = useContextMenu();

  const handleContextMenu = useCallback(
    (e: React.MouseEvent) => {
      onRowContextMenu?.(entity, e);
      contextMenuHandler(e);
    },
    [entity, onRowContextMenu, contextMenuHandler],
  );

  return (
    <TableCell
      data-testid="row-selector"
      data-active={di === cursorRow ? "true" : "false"}
      data-row-entity-id={entity.id}
      className={cn(
        "w-10 px-0 py-1.5 text-center cursor-pointer select-none text-[10px] font-medium text-muted-foreground bg-muted/50 border-r border-border/50",
        di === cursorRow && "bg-muted text-foreground",
      )}
      style={{ width: 40 }}
      onClick={() => onCellClick(di, cursorCol)}
      onContextMenu={handleContextMenu}
    >
      {di + 1}
    </TableCell>
  );
}

/**
 * Resolve the effective sort strategy for a field.
 *
 * Uses the explicit `field.sort` when present, otherwise infers from
 * `field.type.kind` — mirroring the Rust backend's `effective_sort()`.
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
