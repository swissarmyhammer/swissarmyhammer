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
import { CellDispatch } from "@/components/cells";
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
  renderEditor?: (entity: Entity, field: FieldDef, onCommit: (value: unknown) => void, onCancel: () => void) => React.ReactNode;
  grouping?: string[];
  /** Called when the visible data row count changes (e.g. group collapse). */
  onVisibleRowCount?: (count: number) => void;
}

export function DataTable({ columns, rows, grid, onCellClick, onRowContextMenu, renderEditor, grouping: groupingProp, onVisibleRowCount }: DataTableProps) {
  const tableContainerRef = useRef<HTMLDivElement>(null);
  const cursorRef = useRef<HTMLTableCellElement>(null);
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
            <CellDispatch
              field={col.field}
              value={entity.fields[col.field.name]}
              entity={entity}
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
            <TableRow key={headerGroup.id} className="border-b border-border hover:bg-transparent">
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
                    style={header.column.getSize() ? { width: header.column.getSize() } : undefined}
                    onClick={header.column.getToggleSortingHandler()}
                    onContextMenu={(e) => {
                      e.preventDefault();
                      header.column.toggleGrouping();
                    }}
                  >
                    <span className="flex items-center gap-1">
                      {isGrouped && <ChevronRight className="h-3 w-3 text-primary" />}
                      {flexRender(header.column.columnDef.header, header.getContext())}
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
              return (
                <TableRow
                  key={row.id}
                  className="border-b border-border/50 bg-muted/20 hover:bg-muted/40 cursor-pointer"
                  onClick={() => row.toggleExpanded()}
                >
                  <TableCell colSpan={columns.length} className="px-4 py-1.5 font-medium text-sm">
                    <span className="flex items-center gap-1.5">
                      {row.getIsExpanded()
                        ? <ChevronDown className="h-3.5 w-3.5" />
                        : <ChevronRight className="h-3.5 w-3.5" />
                      }
                      {String(row.groupingValue ?? "—")}
                      <span className="text-muted-foreground text-xs">({row.subRows.length})</span>
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
                  di === grid.cursor.row && grid.mode !== "edit" && "bg-accent/30",
                )}
                onContextMenu={(e) => {
                  grid.setCursor(di, grid.cursor.col);
                  onRowContextMenu?.(entity, e);
                  contextMenuHandler(e);
                }}
              >
                {columns.map((col, ci) => {
                  const isCursor = di === grid.cursor.row && ci === grid.cursor.col;
                  const isSel = isSelected(di, ci);
                  const isEditing = isCursor && grid.mode === "edit" && renderEditor;
                  return (
                    <TableCell
                      key={col.field.id}
                      ref={isCursor ? cursorRef : undefined}
                      className={cn(
                        "px-3 py-1.5 align-middle max-w-[300px]",
                        ci === 0 && "pl-4",
                        isCursor && "ring-2 ring-primary ring-inset",
                        isSel && !isCursor && "bg-primary/10",
                        isEditing && "p-0",
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
                        <CellDispatch
                          field={col.field}
                          value={entity.fields[col.field.name]}
                          entity={entity}
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

/** Compare two field values for sorting. */
function compareValues(a: unknown, b: unknown, field: FieldDef): number {
  if (a == null && b == null) return 0;
  if (a == null) return 1;
  if (b == null) return -1;

  const kind = field.type.kind;
  if (kind === "number" || kind === "integer") {
    return (Number(a) || 0) - (Number(b) || 0);
  }
  if (kind === "date") {
    return String(a).localeCompare(String(b));
  }
  if (Array.isArray(a) && Array.isArray(b)) {
    return a.length - b.length;
  }
  return String(a).localeCompare(String(b));
}
