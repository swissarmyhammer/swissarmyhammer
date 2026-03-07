import { useRef, useEffect, useCallback, useState, useMemo } from "react";
import { ArrowUp, ArrowDown } from "lucide-react";
import { cn } from "@/lib/utils";
import { useContextMenu } from "@/lib/context-menu";
import { CellDispatch } from "@/components/cells";
import type { UseGridReturn } from "@/hooks/use-grid";
import type { Entity, FieldDef } from "@/types/kanban";

export interface DataTableColumn {
  field: FieldDef;
  width?: number;
}

export type SortDirection = "asc" | "desc";

export interface SortState {
  fieldName: string;
  direction: SortDirection;
}

interface DataTableProps {
  columns: DataTableColumn[];
  rows: Entity[];
  grid: UseGridReturn;
  onCellClick?: (row: number, col: number) => void;
  /** Right-click handler for a row's entity. */
  onRowContextMenu?: (entity: Entity, e: React.MouseEvent) => void;
  /** Inline cell editor renderer. Called when grid.mode === "edit" for the cursor cell. */
  renderEditor?: (entity: Entity, field: FieldDef, onCommit: (value: unknown) => void, onCancel: () => void) => React.ReactNode;
}

/** Compare two field values for sorting. */
function compareValues(a: unknown, b: unknown, field: FieldDef): number {
  // Nulls sort last
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

export function DataTable({ columns, rows, grid, onCellClick, onRowContextMenu, renderEditor }: DataTableProps) {
  const tableRef = useRef<HTMLDivElement>(null);
  const cursorRef = useRef<HTMLTableCellElement>(null);
  const [sort, setSort] = useState<SortState | null>(null);
  const contextMenuHandler = useContextMenu();

  // Sorted rows (ephemeral — doesn't mutate source)
  const sortedRows = useMemo(() => {
    if (!sort) return rows;
    const field = columns.find((c) => c.field.name === sort.fieldName)?.field;
    if (!field) return rows;
    const dir = sort.direction === "asc" ? 1 : -1;
    return [...rows].sort((a, b) =>
      dir * compareValues(a.fields[field.name], b.fields[field.name], field)
    );
  }, [rows, sort, columns]);

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

  const handleHeaderClick = useCallback((fieldName: string) => {
    setSort((prev) => {
      if (prev?.fieldName === fieldName) {
        if (prev.direction === "asc") return { fieldName, direction: "desc" };
        return null; // third click clears sort
      }
      return { fieldName, direction: "asc" };
    });
  }, []);

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

  if (sortedRows.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center text-muted-foreground">
        No rows to display
      </div>
    );
  }

  return (
    <div ref={tableRef} className="flex-1 overflow-auto min-h-0">
      <table className="w-full border-collapse text-sm">
        <thead className="sticky top-0 z-[1] bg-muted/80 backdrop-blur-sm">
          <tr>
            {columns.map((col, ci) => {
              const isSorted = sort?.fieldName === col.field.name;
              return (
                <th
                  key={col.field.id}
                  className={cn(
                    "text-left px-3 py-1.5 text-xs font-medium text-muted-foreground uppercase tracking-wide border-b border-border select-none cursor-pointer hover:bg-muted/60 transition-colors",
                    ci === 0 && "pl-4",
                  )}
                  style={col.width ? { width: col.width } : undefined}
                  onClick={() => handleHeaderClick(col.field.name)}
                >
                  <span className="flex items-center gap-1">
                    {col.field.name.replace(/_/g, " ")}
                    {isSorted && (
                      sort.direction === "asc"
                        ? <ArrowUp className="h-3 w-3" />
                        : <ArrowDown className="h-3 w-3" />
                    )}
                  </span>
                </th>
              );
            })}
          </tr>
        </thead>
        <tbody>
          {sortedRows.map((entity, ri) => (
            <tr
              key={entity.id}
              className={cn(
                "border-b border-border/50 transition-colors",
                ri === grid.cursor.row && grid.mode !== "edit" && "bg-accent/30",
              )}
              onContextMenu={(e) => {
                grid.setCursor(ri, grid.cursor.col);
                if (onRowContextMenu) {
                  onRowContextMenu(entity, e);
                }
                contextMenuHandler(e);
              }}
            >
              {columns.map((col, ci) => {
                const isCursor =
                  ri === grid.cursor.row && ci === grid.cursor.col;
                const isSel = isSelected(ri, ci);
                const isEditing = isCursor && grid.mode === "edit" && renderEditor;
                return (
                  <td
                    key={col.field.id}
                    ref={isCursor ? cursorRef : undefined}
                    className={cn(
                      "px-3 py-1.5 align-middle max-w-[300px]",
                      ci === 0 && "pl-4",
                      isCursor && "ring-2 ring-primary ring-inset",
                      isSel && !isCursor && "bg-primary/10",
                      isEditing && "p-0",
                    )}
                    onClick={() => handleCellClick(ri, ci)}
                    onDoubleClick={() => {
                      grid.setCursor(ri, ci);
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
                  </td>
                );
              })}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
