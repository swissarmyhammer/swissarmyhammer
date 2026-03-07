import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useGrid } from "@/hooks/use-grid";
import { useSchema } from "@/lib/schema-context";
import { useKeymap } from "@/lib/keymap-context";
import { useAppMode } from "@/lib/app-mode-context";
import { useFieldUpdate } from "@/lib/field-update-context";
import { useEntityStore } from "@/lib/entity-store-context";
import { useInspect } from "@/lib/inspect-context";
import { moniker } from "@/lib/moniker";
import { CommandScopeProvider, type CommandDef } from "@/lib/command-scope";
import { DataTable, type DataTableColumn } from "@/components/data-table";
import { CellEditor } from "@/components/cells/cell-editor";
import type { ViewDef, Entity, FieldDef } from "@/types/kanban";

interface GridViewProps {
  view: ViewDef;
}

export function GridView({ view }: GridViewProps) {
  const { getEntities } = useEntityStore();
  const entityType = view.entity_type ?? "task";
  const entities = getEntities(entityType);
  const { getSchema } = useSchema();
  const schema = getSchema(entityType);
  const fields = schema?.fields ?? [];

  // Build columns from view's card_fields (or all visible fields)
  const columns = useMemo<DataTableColumn[]>(() => {
    const fieldNames = view.card_fields ?? [];
    if (fieldNames.length === 0) {
      return fields
        .filter((f) => f.section !== "hidden")
        .map((f) => ({ field: f }));
    }
    const fieldMap = new Map<string, FieldDef>();
    for (const f of fields) fieldMap.set(f.name, f);
    return fieldNames
      .map((name) => fieldMap.get(name))
      .filter((f): f is FieldDef => f !== undefined)
      .map((f) => ({ field: f }));
  }, [view.card_fields, fields]);

  // Visible row count may differ from entities.length when groups are collapsed
  const [visibleRowCount, setVisibleRowCount] = useState(entities.length);
  useEffect(() => { setVisibleRowCount(entities.length); }, [entities.length]);

  const grid = useGrid({ rowCount: visibleRowCount, colCount: columns.length });
  const gridRef = useRef(grid);
  gridRef.current = grid;

  const { mode: keymapMode } = useKeymap();
  const { mode: appMode } = useAppMode();
  const { updateField } = useFieldUpdate();
  const inspectEntity = useInspect();

  // Keyboard handler
  useEffect(() => {
    if (appMode !== "normal") return;

    const handler = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement | null;
      // Skip if inside an input/editor
      if (
        target?.tagName === "INPUT" ||
        target?.tagName === "TEXTAREA" ||
        target?.tagName === "SELECT" ||
        target?.closest?.(".cm-editor") ||
        target?.closest?.("[contenteditable]")
      ) {
        return;
      }

      const g = gridRef.current;

      if (g.mode === "edit") {
        if (e.key === "Escape") {
          e.preventDefault();
          g.exitEdit();
        }
        return;
      }

      // Normal mode navigation
      let handled = true;
      switch (e.key) {
        // Vim + arrow navigation
        case "j":
        case "ArrowDown":
          g.moveDown();
          break;
        case "k":
        case "ArrowUp":
          g.moveUp();
          break;
        case "h":
        case "ArrowLeft":
          g.moveLeft();
          break;
        case "l":
        case "ArrowRight":
          g.moveRight();
          break;
        case "Home":
        case "0":
          g.moveToRowStart();
          break;
        case "End":
        case "$":
          g.moveToRowEnd();
          break;
        // Edit mode entry
        case "i":
          if (keymapMode === "vim") g.enterEdit();
          break;
        case "Enter":
          g.enterEdit();
          break;
        case "Escape":
          if (g.mode === "visual") g.exitVisual();
          break;
        // Visual mode
        case "v":
          if (keymapMode === "vim") {
            if (g.mode === "visual") g.exitVisual();
            else g.enterVisual();
          }
          break;
        default:
          handled = false;
      }

      if (handled) {
        e.preventDefault();
        e.stopPropagation();
      }
    };

    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [keymapMode, appMode]);

  // Grid commands for the command scope
  const gridCommands = useMemo<CommandDef[]>(() => {
    const commands: CommandDef[] = [
      {
        id: "grid.moveUp",
        name: "Move Up",
        keys: { vim: "k", cua: "ArrowUp" },
        execute: () => gridRef.current.moveUp(),
      },
      {
        id: "grid.moveDown",
        name: "Move Down",
        keys: { vim: "j", cua: "ArrowDown" },
        execute: () => gridRef.current.moveDown(),
      },
      {
        id: "grid.moveLeft",
        name: "Move Left",
        keys: { vim: "h", cua: "ArrowLeft" },
        execute: () => gridRef.current.moveLeft(),
      },
      {
        id: "grid.moveRight",
        name: "Move Right",
        keys: { vim: "l", cua: "ArrowRight" },
        execute: () => gridRef.current.moveRight(),
      },
      {
        id: "grid.edit",
        name: "Edit Cell",
        keys: { vim: "i", cua: "Enter" },
        execute: () => gridRef.current.enterEdit(),
      },
      {
        id: "grid.escape",
        name: "Exit Edit",
        keys: { vim: "Escape", cua: "Escape" },
        execute: () => {
          if (gridRef.current.mode === "edit") gridRef.current.exitEdit();
          else if (gridRef.current.mode === "visual") gridRef.current.exitVisual();
        },
      },
      {
        id: "grid.deleteRow",
        name: "Delete Row",
        execute: () => {
          const row = gridRef.current.cursor.row;
          if (row >= 0 && row < entities.length) {
            const entity = entities[row];
            invoke("dispatch_command", {
              cmd: `${entityType}.archive`,
              args: { id: entity.id },
            }).catch((err) => console.error("Failed to delete row:", err));
          }
        },
      },
      {
        id: "grid.newBelow",
        name: "New Row Below",
        keys: { vim: "o", cua: "Mod+Enter" },
        execute: () => {
          invoke("dispatch_command", {
            cmd: `${entityType}.add`,
            args: { title: `New ${entityType}` },
          }).catch((err) => console.error("Failed to add row:", err));
        },
      },
      {
        id: "grid.newAbove",
        name: "New Row Above",
        keys: { vim: "O", cua: "Mod+Shift+Enter" },
        execute: () => {
          invoke("dispatch_command", {
            cmd: `${entityType}.add`,
            args: { title: `New ${entityType}` },
          }).catch((err) => console.error("Failed to add row:", err));
        },
      },
      {
        id: "entity.inspect",
        name: "Inspect",
        contextMenu: true,
        execute: () => {
          const row = gridRef.current.cursor.row;
          if (row >= 0 && row < entities.length) {
            const entity = entities[row];
            inspectEntity(moniker(entity.entity_type, entity.id));
          }
        },
      },
      {
        id: `${entityType}.archive`,
        name: "Archive",
        contextMenu: true,
        execute: () => {
          const row = gridRef.current.cursor.row;
          if (row >= 0 && row < entities.length) {
            const entity = entities[row];
            invoke("dispatch_command", {
              cmd: `${entityType}.archive`,
              args: { id: entity.id },
            }).catch((err) => console.error("Failed to archive:", err));
          }
        },
      },
    ];
    return commands;
  }, [entities, entityType, inspectEntity]);

  const handleCellClick = useCallback((row: number, col: number) => {
    grid.setCursor(row, col);
  }, [grid]);

  const renderEditor = useCallback(
    (entity: Entity, field: FieldDef, onCommit: (value: unknown) => void, onCancel: () => void) => {
      const handleCommit = (value: unknown) => {
        updateField(entity.entity_type, entity.id, field.name, value).catch(() => {});
        onCommit(value);
      };
      return (
        <CellEditor
          field={field}
          value={entity.fields[field.name]}
          onCommit={handleCommit}
          onCancel={onCancel}
        />
      );
    },
    [updateField],
  );

  return (
    <CommandScopeProvider commands={gridCommands}>
      <main className="flex-1 flex flex-col min-h-0">
        <div className="flex items-center px-4 py-1.5 border-b border-border bg-muted/30 text-xs text-muted-foreground gap-3">
          <span>{entities.length} rows</span>
          <span className="text-muted-foreground/50">|</span>
          <span>
            {grid.mode === "edit" ? "EDIT" : grid.mode === "visual" ? "VISUAL" : "NORMAL"}
          </span>
          {entities.length > 0 && (
            <>
              <span className="text-muted-foreground/50">|</span>
              <span>
                R{grid.cursor.row + 1}:C{grid.cursor.col + 1}
              </span>
            </>
          )}
        </div>
        <DataTable
          columns={columns}
          rows={entities}
          grid={grid}
          onCellClick={handleCellClick}
          renderEditor={renderEditor}
          onVisibleRowCount={setVisibleRowCount}
        />
      </main>
    </CommandScopeProvider>
  );
}
