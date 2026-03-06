import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { KeymapProvider } from "@/lib/keymap-context";
import { AppModeProvider } from "@/lib/app-mode-context";
import { UndoStackProvider } from "@/lib/undo-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { SchemaProvider } from "@/lib/schema-context";
import { FieldUpdateProvider } from "@/lib/field-update-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { InspectProvider } from "@/lib/inspect-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import { AppShell } from "@/components/app-shell";
import { NavBar } from "@/components/nav-bar";
import { LeftNav } from "@/components/left-nav";
import { ModeIndicator } from "@/components/mode-indicator";
import { BoardView } from "@/components/board-view";
import { GridView } from "@/components/grid-view";
import { EntityInspector } from "@/components/entity-inspector";
import { SlidePanel } from "@/components/slide-panel";
import { ViewsProvider, useViews } from "@/lib/views-context";
import { CommandScopeProvider, type CommandDef } from "@/lib/command-scope";
import type {
  BoardData, OpenBoard, Entity, EntityBag,
  BoardDataResponse, EntityListResponse,
} from "@/types/kanban";
import { entityFromBag, parseBoardData, getStr } from "@/types/kanban";

const PANEL_WIDTH = 420;

/** Payload for entity-created Tauri event. */
interface EntityCreatedEvent {
  kind: "entity-created";
  entity_type: string;
  id: string;
  fields: Record<string, unknown>;
}

/** Payload for entity-removed Tauri event. */
interface EntityRemovedEvent {
  kind: "entity-removed";
  entity_type: string;
  id: string;
}

/** Payload for entity-field-changed Tauri event. */
interface EntityFieldChangedEvent {
  kind: "entity-field-changed";
  entity_type: string;
  id: string;
  changes: Array<{ field: string; value: unknown }>;
  /** Full entity state including computed fields. Present for own-command
   *  events; absent for external file-watcher events. */
  fields?: Record<string, unknown>;
}

/** A panel entry is just an entity reference — entity type + id. */
interface PanelEntry {
  entityType: string;
  entityId: string;
}

function App() {
  const [board, setBoard] = useState<BoardData | null>(null);
  const [taskEntities, setTaskEntities] = useState<Entity[]>([]);
  const [tagEntities, setTagEntities] = useState<Entity[]>([]);
  const [openBoards, setOpenBoards] = useState<OpenBoard[]>([]);
  const [panelStack, setPanelStack] = useState<PanelEntry[]>([]);
  const panelStackRef = useRef(panelStack);
  panelStackRef.current = panelStack;

  /** Open an inspector for any entity. Replaces the stack for primary entities, pushes for secondary. */
  const inspectEntity = useCallback((entityType: string, entityId: string) => {
    setPanelStack((prev) => {
      // Primary entity (task, column, board) replaces the stack
      if (entityType === "task" || entityType === "column" || entityType === "board") {
        return [{ entityType, entityId }];
      }
      // Secondary entities (tag) push onto the stack, replacing any existing entry of same type
      const filtered = prev.filter((e) => e.entityType !== entityType);
      return [...filtered, { entityType, entityId }];
    });
  }, []);

  const closeTopPanel = useCallback(() => {
    setPanelStack((prev) => prev.slice(0, -1));
  }, []);

  /** Close the topmost panel. Returns true if a panel was actually closed. */
  const dismissTopPanel = useCallback((): boolean => {
    if (panelStackRef.current.length === 0) return false;
    setPanelStack((prev) => prev.slice(0, -1));
    return true;
  }, []);

  const closeAll = useCallback(() => {
    setPanelStack([]);
  }, []);

  const refresh = useCallback(async () => {
    try {
      const [boardData, openData, taskData] = await Promise.all([
        invoke<BoardDataResponse>("get_board_data"),
        invoke<OpenBoard[]>("list_open_boards"),
        invoke<EntityListResponse>("list_entities", { entityType: "task" }),
      ]);
      setBoard(parseBoardData(boardData));
      setOpenBoards(openData);
      setTaskEntities(taskData.entities.map(entityFromBag));
      setTagEntities(boardData.tags.map(entityFromBag));
    } catch (error) {
      console.error("Failed to load board data:", error);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  // ---------------------------------------------------------------------------
  // Granular entity event listeners — patch local state surgically instead
  // of doing a full refresh.
  // ---------------------------------------------------------------------------

  useEffect(() => {
    const unlisteners = [
      listen<EntityCreatedEvent>("entity-created", (event) => {
        const { entity_type, id } = event.payload;
        if (entity_type === "column" || entity_type === "swimlane") {
          // Structural changes — full refresh to get correct counts/ordering
          refresh();
          return;
        }
        // Re-fetch the full entity so computed fields are included
        invoke<EntityBag>("get_entity", { entityType: entity_type, id })
          .then((bag) => {
            const entity = entityFromBag(bag);
            if (entity_type === "task") {
              setTaskEntities((prev) => {
                // Guard against duplicates (cache may lag behind entity store)
                if (prev.some((e) => e.id === id)) {
                  return prev.map((e) => (e.id === id ? entity : e));
                }
                return [...prev, entity];
              });
            } else if (entity_type === "tag") {
              setTagEntities((prev) => {
                if (prev.some((e) => e.id === id)) {
                  return prev.map((e) => (e.id === id ? entity : e));
                }
                return [...prev, entity];
              });
            }
          })
          .catch((err) => {
            console.error(`[entity-created] Failed to fetch ${entity_type}/${id}:`, err);
          });
      }),
      listen<EntityRemovedEvent>("entity-removed", (event) => {
        const { entity_type, id } = event.payload;
        if (entity_type === "task") {
          setTaskEntities((prev) => prev.filter((e) => e.id !== id));
        } else if (entity_type === "tag") {
          setTagEntities((prev) => prev.filter((e) => e.id !== id));
        } else if (entity_type === "column" || entity_type === "swimlane") {
          refresh();
        }
      }),
      listen<EntityFieldChangedEvent>("entity-field-changed", (event) => {
        const { entity_type, id, fields: fullFields } = event.payload;

        // Use enriched fields from the event when available (includes
        // computed fields like tags, progress). Fall back to fetching
        // via get_entity for external watcher events.
        const applyEntity = (entity: Entity) => {
          const replaceById = (entities: Entity[]) =>
            entities.map((e) => (e.id === id ? entity : e));

          if (entity_type === "task") {
            setTaskEntities(replaceById);
          } else if (entity_type === "tag") {
            setTagEntities(replaceById);
          } else if (entity_type === "board") {
            setBoard((prev) => (prev ? { ...prev, board: entity } : prev));
          } else if (entity_type === "column") {
            setBoard((prev) => (prev ? { ...prev, columns: replaceById(prev.columns) } : prev));
          } else if (entity_type === "swimlane") {
            setBoard((prev) => (prev ? { ...prev, swimlanes: replaceById(prev.swimlanes) } : prev));
          }
        };

        if (fullFields) {
          applyEntity({ entity_type, id, fields: fullFields });
        } else {
          invoke<EntityBag>("get_entity", { entityType: entity_type, id })
            .then((bag) => applyEntity(entityFromBag(bag)))
            .catch((err) => {
              console.error(`[entity-field-changed] Failed to fetch ${entity_type}/${id}:`, err);
            });
        }
      }),
      // Keep board-changed for structural operations (open/switch board)
      listen("board-changed", () => {
        refresh();
      }),
    ];
    return () => {
      for (const p of unlisteners) {
        p.then((fn) => fn());
      }
    };
  }, [refresh]);

  const entityStore = useMemo(() => ({
    task: taskEntities,
    tag: tagEntities,
    column: board?.columns ?? [],
    swimlane: board?.swimlanes ?? [],
  }), [taskEntities, tagEntities, board]);

  return (
    <TooltipProvider delayDuration={400}>
    <SchemaProvider>
    <EntityStoreProvider entities={entityStore}>
    <EntityFocusProvider>
    <FieldUpdateProvider>
    <KeymapProvider>
    <AppModeProvider>
    <UndoStackProvider>
    <InspectProvider onInspect={inspectEntity} onDismiss={dismissTopPanel}>
    <AppShell>
    <ViewsProvider>
    <ViewCommandScope>
    <div className="h-screen bg-background text-foreground flex flex-col">
      <NavBar
        board={board}
        openBoards={openBoards}
        onBoardSwitched={refresh}
        onBoardInspect={() => inspectEntity("board", "board")}
      />
      {board ? (
        <>
          <div className="flex-1 flex min-h-0">
            <LeftNav />
            <ActiveViewRenderer
              board={board}
              tasks={taskEntities}
            />
          </div>

          {/* Backdrop — visible when any panel is open */}
          <div
            className={`fixed inset-0 bg-black/20 transition-opacity duration-200 ${
              panelStack.length > 0 ? "opacity-100" : "opacity-0 pointer-events-none"
            }`}
            onClick={closeAll}
          />

          {/* Render inspector panels from the stack */}
          {panelStack.map((entry, index) => {
            const rightOffset = (panelStack.length - 1 - index) * PANEL_WIDTH;
            return (
              <InspectorPanel
                key={`${entry.entityType}-${entry.entityId}`}
                entry={entry}
                entityStore={entityStore}
                board={board}
                onClose={closeTopPanel}
                style={{ right: rightOffset }}
              />
            );
          })}
        </>
      ) : (
        <main className="flex-1 flex items-center justify-center">
          <p className="text-muted-foreground">
            No board loaded. Open a board to get started.
          </p>
        </main>
      )}
      <ModeIndicator />
    </div>
    </ViewCommandScope>
    </ViewsProvider>
    </AppShell>
    </InspectProvider>
    </UndoStackProvider>
    </AppModeProvider>
    </KeymapProvider>
    </FieldUpdateProvider>
    </EntityFocusProvider>
    </EntityStoreProvider>
    </SchemaProvider>
    </TooltipProvider>
  );
}

/**
 * Provides nav.view commands generated from the views registry.
 * Each view gets a `nav.view.<id>` command that switches to it,
 * plus a generic `nav.view` command that takes args.
 */
function ViewCommandScope({ children }: { children: React.ReactNode }) {
  const { views, setActiveViewId } = useViews();

  const viewCommands: CommandDef[] = useMemo(() => {
    return views.map((view) => ({
      id: `nav.view.${view.id}`,
      name: `View: ${view.name}`,
      execute: () => setActiveViewId(view.id),
    }));
  }, [views, setActiveViewId]);

  return (
    <CommandScopeProvider commands={viewCommands}>
      {children}
    </CommandScopeProvider>
  );
}

/**
 * Renders the currently active view based on its kind.
 * For "board" kind, renders the BoardView. Other kinds show a placeholder.
 */
function ActiveViewRenderer({
  board,
  tasks,
}: {
  board: BoardData;
  tasks: Entity[];
}) {
  const { activeView } = useViews();

  if (!activeView || activeView.kind === "board") {
    return (
      <BoardView
        board={board}
        tasks={tasks}
      />
    );
  }

  if (activeView.kind === "grid") {
    return (
      <GridView
        view={activeView}
        tasks={tasks}
      />
    );
  }

  return (
    <main className="flex-1 flex items-center justify-center">
      <p className="text-muted-foreground">
        {activeView.name} view ({activeView.kind}) is not yet implemented.
      </p>
    </main>
  );
}

/**
 * Resolves an entity for the inspector panel. Tries the local entity store
 * first, then falls back to fetching from the backend via get_entity.
 */
function InspectorPanel({
  entry,
  entityStore,
  board,
  onClose,
  style,
}: {
  entry: PanelEntry;
  entityStore: Record<string, Entity[]>;
  board: BoardData | null;
  onClose: () => void;
  style?: React.CSSProperties;
}) {
  const [fetchedEntity, setFetchedEntity] = useState<Entity | null>(null);
  const [fetchError, setFetchError] = useState<string | null>(null);
  const fetchedRef = useRef<string | null>(null);

  // Try local store first
  const entities = entityStore[entry.entityType];
  let localEntity = entities?.find((e) => e.id === entry.entityId);
  if (!localEntity && entry.entityType === "tag") {
    localEntity = entities?.find((e) => getStr(e, "tag_name") === entry.entityId);
  }
  // Board entity is special
  const resolved = localEntity ?? (
    entry.entityType === "board" ? board?.board : undefined
  ) ?? fetchedEntity;

  // Fetch from backend if not found locally
  const fetchKey = `${entry.entityType}:${entry.entityId}`;

  // Reset fetch dedup ref when the target entity changes so a new
  // fetch can be attempted (e.g. after a failed fetch for a different entity).
  useEffect(() => {
    fetchedRef.current = null;
  }, [fetchKey]);

  useEffect(() => {
    if (resolved || fetchedRef.current === fetchKey) return;
    fetchedRef.current = fetchKey;
    setFetchError(null);
    invoke<Record<string, unknown>>("get_entity", {
      entityType: entry.entityType,
      id: entry.entityId,
    })
      .then((bag) => {
        setFetchedEntity(entityFromBag(bag as EntityBag));
      })
      .catch((err) => {
        const msg = String(err);
        console.error(`[InspectorPanel] Failed to fetch entity: ${fetchKey}`, err);
        setFetchError(msg);
      });
  }, [resolved, fetchKey, entry.entityType, entry.entityId]);

  if (!resolved) {
    return (
      <SlidePanel open={true} onClose={onClose} style={style}>
        <p className="text-sm text-muted-foreground">
          {fetchError ? `Entity not found` : "Loading…"}
        </p>
      </SlidePanel>
    );
  }

  return (
    <SlidePanel open={true} onClose={onClose} style={style}>
      <EntityInspector entity={resolved} />
    </SlidePanel>
  );
}

export default App;
