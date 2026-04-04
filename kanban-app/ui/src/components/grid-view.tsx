import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { useDispatchCommand } from "@/lib/command-scope";
import { useGrid } from "@/hooks/use-grid";
import { useSchema } from "@/lib/schema-context";
import { useEntityStore } from "@/lib/entity-store-context";
import { fieldMoniker } from "@/lib/moniker";
import {
  useEntityFocus,
  type ClaimPredicate,
} from "@/lib/entity-focus-context";
import {
  useEntityCommands,
  buildEntityCommandDefs,
} from "@/lib/entity-commands";
import { CommandScopeProvider, type CommandDef } from "@/lib/command-scope";
import { PerspectiveTabBar } from "@/components/perspective-tab-bar";
import { usePerspectives } from "@/lib/perspective-context";
import { evaluateFilter, evaluateSort } from "@/lib/perspective-eval";
import { DataTable, type DataTableColumn } from "@/components/data-table";
import { Field } from "@/components/fields/field";
import type { ViewDef, Entity, FieldDef } from "@/types/kanban";

interface GridViewProps {
  view: ViewDef;
}

export function GridView({ view }: GridViewProps) {
  const dispatch = useDispatchCommand();
  const { getEntities } = useEntityStore();

  // All hooks must be called unconditionally (React rules of hooks).
  // Use empty-string fallback so hooks always run; we guard before JSX below.
  const entityType = view.entity_type ?? "";
  const rawEntities = getEntities(entityType);
  const { getSchema, getEntityCommands } = useSchema();
  const schema = getSchema(entityType);
  const fields = schema?.fields ?? [];
  // Schema-driven entity commands for per-row context menus
  const schemaCommands = getEntityCommands(entityType);

  // Filter and sort entities through the active perspective's expressions.
  const { activePerspective } = usePerspectives();
  const entities = useMemo(() => {
    const filtered = evaluateFilter(activePerspective?.filter, rawEntities);
    return evaluateSort(activePerspective?.sort ?? [], filtered);
  }, [activePerspective?.filter, activePerspective?.sort, rawEntities]);

  // Derive DataTable grouping from the active perspective's group field.
  const grouping = useMemo<string[] | undefined>(
    () => (activePerspective?.group ? [activePerspective.group] : undefined),
    [activePerspective?.group],
  );

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
  useEffect(() => {
    setVisibleRowCount(entities.length);
  }, [entities.length]);

  // --- Pull-based navigation via claimWhen ---
  const { focusedMoniker, setFocus, broadcastNavCommand } = useEntityFocus();
  const broadcastRef = useRef(broadcastNavCommand);
  broadcastRef.current = broadcastNavCommand;

  /**
   * Build a moniker→{row,col} lookup from the current grid layout.
   * Used to derive the cursor position from the focused moniker.
   */
  const cellMonikerMap = useMemo(() => {
    const map = new Map<string, { row: number; col: number }>();
    for (let r = 0; r < entities.length; r++) {
      for (let c = 0; c < columns.length; c++) {
        const mk = fieldMoniker(
          entityType,
          entities[r].id,
          columns[c].field.name,
        );
        map.set(mk, { row: r, col: c });
      }
    }
    return map;
  }, [entities, columns, entityType]);

  /**
   * Build the flat list of cell monikers in row-major order.
   * cellMonikers[row][col] = moniker string.
   */
  const cellMonikers = useMemo(() => {
    return entities.map((entity) =>
      columns.map((col) => fieldMoniker(entityType, entity.id, col.field.name)),
    );
  }, [entities, columns, entityType]);

  /**
   * Derive the grid cursor position from the currently focused moniker.
   * Returns {row: -1, col: -1} if nothing in this grid is focused.
   */
  const derivedCursor = useMemo(() => {
    if (!focusedMoniker) return null;
    return cellMonikerMap.get(focusedMoniker) ?? null;
  }, [focusedMoniker, cellMonikerMap]);

  const grid = useGrid({
    rowCount: visibleRowCount,
    colCount: columns.length,
    cursor: derivedCursor ?? undefined,
  });
  const gridRef = useRef(grid);
  gridRef.current = grid;

  // Focus the first cell on mount if no grid cell is focused
  const firstCellMoniker = cellMonikers[0]?.[0] ?? null;
  useEffect(() => {
    if (!firstCellMoniker) return;
    // Only focus if nothing in this grid is currently focused
    if (!derivedCursor) {
      setFocus(firstCellMoniker);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [firstCellMoniker, setFocus]);

  /**
   * Build claimWhen predicates for each cell in the grid.
   * Returns a 2D array: claimPredicates[row][col] = ClaimPredicate[].
   */
  const claimPredicates = useMemo(() => {
    /** Check if a moniker is any cell in this grid. */
    const isGridCell = (mk: string | null): boolean => {
      if (!mk) return false;
      return cellMonikerMap.has(mk);
    };

    const rowCount = cellMonikers.length;
    const colCount = columns.length;

    return cellMonikers.map((row, ri) =>
      row.map((_, ci) => {
        const predicates: ClaimPredicate[] = [];

        // nav.down: claim if the cell above me is focused
        if (ri > 0) {
          const above = cellMonikers[ri - 1][ci];
          predicates.push({
            command: "nav.down",
            when: (f) => f === above,
          });
        }

        // nav.up: claim if the cell below me is focused
        if (ri < rowCount - 1) {
          const below = cellMonikers[ri + 1][ci];
          predicates.push({
            command: "nav.up",
            when: (f) => f === below,
          });
        }

        // nav.right: claim if the cell to my left is focused
        if (ci > 0) {
          const left = cellMonikers[ri][ci - 1];
          predicates.push({
            command: "nav.right",
            when: (f) => f === left,
          });
        }

        // nav.left: claim if the cell to my right is focused
        if (ci < colCount - 1) {
          const right = cellMonikers[ri][ci + 1];
          predicates.push({
            command: "nav.left",
            when: (f) => f === right,
          });
        }

        // nav.rowStart: claim if I'm column 0 and any cell in my row is focused
        if (ci === 0) {
          predicates.push({
            command: "nav.rowStart",
            when: (f) => {
              if (!f) return false;
              const pos = cellMonikerMap.get(f);
              return pos !== undefined && pos.row === ri && pos.col !== 0;
            },
          });
        }

        // nav.rowEnd: claim if I'm the last column and any cell in my row is focused
        if (ci === colCount - 1) {
          predicates.push({
            command: "nav.rowEnd",
            when: (f) => {
              if (!f) return false;
              const pos = cellMonikerMap.get(f);
              return (
                pos !== undefined && pos.row === ri && pos.col !== colCount - 1
              );
            },
          });
        }

        // nav.first: claim if I'm cell (0,0) and any other grid cell is focused
        if (ri === 0 && ci === 0) {
          predicates.push({
            command: "nav.first",
            when: (f) => isGridCell(f) && f !== cellMonikers[0][0],
          });
        }

        // nav.last: claim if I'm the last cell and any other grid cell is focused
        if (ri === rowCount - 1 && ci === colCount - 1) {
          predicates.push({
            command: "nav.last",
            when: (f) =>
              isGridCell(f) && f !== cellMonikers[rowCount - 1][colCount - 1],
          });
        }

        return predicates;
      }),
    );
  }, [cellMonikers, cellMonikerMap, columns.length]);

  // Current entity and field from cursor position
  const currentEntity =
    grid.cursor.row >= 0 && grid.cursor.row < entities.length
      ? entities[grid.cursor.row]
      : null;
  // currentField and monikers removed — not currently needed.
  // Re-derive from grid.cursor.col + columns if needed in the future.

  // Grid-level commands: navigation broadcasts nav commands via claimWhen.
  // Non-navigation commands (edit, visual, delete, new row) remain push-based.
  const gridCommands = useMemo<CommandDef[]>(
    () => [
      {
        id: "grid.moveUp",
        name: "Move Up",
        keys: { vim: "k", cua: "ArrowUp" },
        execute: () => {
          broadcastRef.current("nav.up");
        },
      },
      {
        id: "grid.moveDown",
        name: "Move Down",
        keys: { vim: "j", cua: "ArrowDown" },
        execute: () => {
          broadcastRef.current("nav.down");
        },
      },
      {
        id: "grid.moveLeft",
        name: "Move Left",
        keys: { vim: "h", cua: "ArrowLeft" },
        execute: () => {
          broadcastRef.current("nav.left");
        },
      },
      {
        id: "grid.moveRight",
        name: "Move Right",
        keys: { vim: "l", cua: "ArrowRight" },
        execute: () => {
          broadcastRef.current("nav.right");
        },
      },
      {
        id: "grid.moveToRowStart",
        name: "Row Start",
        keys: { vim: "0", cua: "Home" },
        execute: () => {
          broadcastRef.current("nav.rowStart");
        },
      },
      {
        id: "grid.moveToRowEnd",
        name: "Row End",
        keys: { vim: "$", cua: "End" },
        execute: () => {
          broadcastRef.current("nav.rowEnd");
        },
      },
      {
        id: "grid.firstCell",
        name: "First Cell",
        keys: { cua: "Mod+Home" },
        execute: () => {
          broadcastRef.current("nav.first");
        },
      },
      {
        id: "grid.lastCell",
        name: "Last Cell",
        keys: { vim: "Shift+G", cua: "Mod+End" },
        execute: () => {
          broadcastRef.current("nav.last");
        },
      },
      // nav.first/nav.last -- generic commands from sequence table (gg) and
      // global scope. Grid scope registers these so they resolve here.
      {
        id: "nav.first",
        name: "First Cell",
        execute: () => {
          broadcastRef.current("nav.first");
        },
      },
      {
        id: "nav.last",
        name: "Last Cell",
        execute: () => {
          broadcastRef.current("nav.last");
        },
      },
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
        // No keys -- field editors handle Escape via onCancel.
        // Escape falls through to app.dismiss.
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
            const entity = entities[row];
            dispatch(`${entityType}.archive`, {
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
          dispatch(`${entityType}.add`, {
            args: { title: `New ${entityType}` },
          }).catch((err) => console.error("Failed to add row:", err));
        },
      },
      {
        id: "grid.newAbove",
        name: "New Row Above",
        keys: { vim: "O", cua: "Mod+Shift+Enter" },
        execute: () => {
          dispatch(`${entityType}.add`, {
            args: { title: `New ${entityType}` },
          }).catch((err) => console.error("Failed to add row:", err));
        },
      },
    ],
    [entities, entityType],
  );

  // Entity-level commands (depend on cursor row)
  // Schema-driven: reads entity commands from YAML schema via useEntityCommands.
  const entityCommands = useEntityCommands(
    entityType,
    currentEntity?.id ?? "",
    currentEntity
      ? {
          entity_type: entityType,
          id: currentEntity.id,
          fields: currentEntity.fields ?? {},
        }
      : undefined,
  );

  const handleCellClick = useCallback(
    (row: number, col: number) => {
      // On click, set focus to the clicked cell's moniker
      const mk = cellMonikers[row]?.[col];
      if (mk) setFocus(mk);
    },
    [cellMonikers, setFocus],
  );

  /**
   * Factory that builds entity-specific context menu commands for a given row.
   *
   * Used by DataTable to wrap each row's selector cell in its own
   * CommandScopeProvider so right-clicking row N always resolves commands for
   * row N's entity -- regardless of the grid cursor position at the time of
   * the right-click.
   *
   * Uses buildEntityCommandDefs (non-hook) because this factory is called
   * inside a callback, not in the React render cycle.
   */
  const buildRowEntityCommands = useCallback(
    (entity: Entity): CommandDef[] => {
      return buildEntityCommandDefs(
        schemaCommands,
        entityType,
        entity.id,
        entity,
      );
    },
    [schemaCommands, entityType],
  );

  const renderEditor = useCallback(
    (
      entity: Entity,
      field: FieldDef,
      onCommit: (value: unknown) => void,
      onCancel: () => void,
    ) => {
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
    },
    [],
  );

  // Guard: views must declare entity_type. Log a warning so misconfigured
  // views are visible in the unified log, and render an empty state.
  if (!view.entity_type) {
    console.warn(
      `[GridView] view "${view.name ?? view.id}" has no entity_type — cannot render grid without one`,
    );
    return (
      <main className="flex-1 flex items-center justify-center text-muted-foreground text-sm">
        View is missing an entity_type definition.
      </main>
    );
  }

  return (
    <CommandScopeProvider commands={gridCommands}>
      {/* Entity commands for the cursor row — wraps DataTable so
          ui.inspect and other entity commands resolve with the correct
          target moniker when dispatched via keybindings or palette. */}
      <CommandScopeProvider commands={entityCommands}>
        <PerspectiveTabBar />
        <main className="flex-1 flex flex-col min-h-0">
          <div className="flex items-center px-4 py-1.5 border-b border-border bg-muted/30 text-xs text-muted-foreground gap-3">
            <span>{entities.length} rows</span>
            <span className="text-muted-foreground/50">|</span>
            <span>
              {grid.mode === "edit"
                ? "EDIT"
                : grid.mode === "visual"
                  ? "VISUAL"
                  : "NORMAL"}
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
            cellMonikers={cellMonikers}
            claimPredicates={claimPredicates}
            onCellClick={handleCellClick}
            renderEditor={renderEditor}
            grouping={grouping}
            onVisibleRowCount={setVisibleRowCount}
            rowEntityCommands={buildRowEntityCommands}
            perspectiveSort={activePerspective?.sort}
            perspectiveId={activePerspective?.id}
          />
        </main>
      </CommandScopeProvider>
    </CommandScopeProvider>
  );
}
