import { useCallback, useContext, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useActiveBoardPath } from "@/lib/command-scope";
import { useGrid } from "@/hooks/use-grid";
import { useSchema } from "@/lib/schema-context";
import { useKeymap } from "@/lib/keymap-context";
import { useAppMode } from "@/lib/app-mode-context";
import { useFieldUpdate } from "@/lib/field-update-context";
import { useEntityStore } from "@/lib/entity-store-context";
import { useEntityFocus } from "@/lib/entity-focus-context";
import { useInspect } from "@/lib/inspect-context";
import { moniker, fieldMoniker } from "@/lib/moniker";
import { CommandScopeProvider, CommandScopeContext, type CommandDef, type CommandScope } from "@/lib/command-scope";
import { DataTable, type DataTableColumn } from "@/components/data-table";
import { CellEditor } from "@/components/cells/cell-editor";
import type { ViewDef, Entity, FieldDef } from "@/types/kanban";

interface GridViewProps {
  view: ViewDef;
}

export function GridView({ view }: GridViewProps) {
  const boardPath = useActiveBoardPath();
  const boardPathRef = useRef(boardPath);
  boardPathRef.current = boardPath;
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

  // Current entity and field from cursor position
  const currentEntity = grid.cursor.row >= 0 && grid.cursor.row < entities.length
    ? entities[grid.cursor.row] : null;
  const currentField = grid.cursor.col >= 0 && grid.cursor.col < columns.length
    ? columns[grid.cursor.col].field : null;
  const currentEntityMoniker = currentEntity ? moniker(entityType, currentEntity.id) : null;
  const currentFieldMoniker = currentEntity && currentField
    ? fieldMoniker(entityType, currentEntity.id, currentField.name) : null;

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

  // Grid-level commands (navigation, row creation — not entity-specific)
  const gridCommands = useMemo<CommandDef[]>(() => [
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
            ...(boardPathRef.current ? { boardPath: boardPathRef.current } : {}),
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
          ...(boardPathRef.current ? { boardPath: boardPathRef.current } : {}),
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
          ...(boardPathRef.current ? { boardPath: boardPathRef.current } : {}),
        }).catch((err) => console.error("Failed to add row:", err));
      },
    },
  ], [entities, entityType]);

  // Entity-level commands (depend on cursor row — registered via focus bridge)
  const entityCommands = useMemo<CommandDef[]>(() => {
    if (!currentEntity || !currentEntityMoniker) return [];
    return [
      {
        id: "entity.inspect",
        name: `Inspect ${entityType}`,
        target: currentEntityMoniker,
        contextMenu: true,
        execute: () => inspectEntity(currentEntityMoniker),
      },
      {
        id: `${entityType}.archive`,
        name: "Archive",
        target: currentEntityMoniker,
        contextMenu: true,
        execute: () => {
          invoke("dispatch_command", {
            cmd: `${entityType}.archive`,
            args: { id: currentEntity.id },
            ...(boardPathRef.current ? { boardPath: boardPathRef.current } : {}),
          }).catch((err) => console.error("Failed to archive:", err));
        },
      },
    ];
  }, [currentEntity, currentEntityMoniker, entityType, inspectEntity]);

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
          entity={entity}
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
      <GridFocusBridge
        entityCommands={entityCommands}
        entityMoniker={currentEntityMoniker}
        fieldMoniker={currentFieldMoniker}
      />
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

/**
 * Bridges the grid's command scope into the entity focus system.
 *
 * Sits inside the grid's CommandScopeProvider so it can access the grid scope
 * via context. Builds a two-level scope chain:
 *   grid scope → entity scope (inspect, archive) → field scope (future)
 *
 * Registers the deepest scope under the field-level moniker (or entity moniker
 * as fallback) in the EntityFocus registry, and sets focus so the command
 * palette and keybindings resolve through the full chain.
 */
function GridFocusBridge({
  entityCommands,
  entityMoniker,
  fieldMoniker: fMoniker,
}: {
  entityCommands: CommandDef[];
  entityMoniker: string | null;
  fieldMoniker: string | null;
}) {
  const gridScope = useContext(CommandScopeContext);
  const { setFocus, registerScope, unregisterScope } = useEntityFocus();
  const prevMonikerRef = useRef<string | null>(null);

  // Build entity scope that chains off the grid scope
  const entityScope = useMemo<CommandScope | null>(() => {
    if (!entityMoniker || entityCommands.length === 0) return null;
    const map = new Map<string, CommandDef>();
    for (const cmd of entityCommands) map.set(cmd.id, cmd);
    return { commands: map, parent: gridScope, moniker: entityMoniker };
  }, [entityCommands, gridScope, entityMoniker]);

  // The field scope chains off entity scope — placeholder for field-specific
  // commands in the future. For now it's an empty scope that just sets the
  // moniker so focus resolves at field granularity.
  const fieldScope = useMemo<CommandScope | null>(() => {
    if (!fMoniker || !entityScope) return null;
    return { commands: new Map(), parent: entityScope, moniker: fMoniker };
  }, [fMoniker, entityScope]);

  // Register the deepest available scope and set focus
  useEffect(() => {
    const scope = fieldScope ?? entityScope;
    const focusMoniker = fMoniker ?? entityMoniker;

    // Unregister previous moniker if it changed
    if (prevMonikerRef.current && prevMonikerRef.current !== focusMoniker) {
      unregisterScope(prevMonikerRef.current);
    }

    if (!focusMoniker || !scope) {
      prevMonikerRef.current = null;
      return;
    }

    registerScope(focusMoniker, scope);
    setFocus(focusMoniker);
    prevMonikerRef.current = focusMoniker;

    return () => {
      if (prevMonikerRef.current) {
        unregisterScope(prevMonikerRef.current);
        prevMonikerRef.current = null;
      }
    };
  }, [fieldScope, entityScope, fMoniker, entityMoniker, registerScope, unregisterScope, setFocus]);

  return null;
}
