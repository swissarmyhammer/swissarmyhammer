import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { KeymapProvider } from "@/lib/keymap-context";
import { AppModeProvider } from "@/lib/app-mode-context";
import { UndoStackProvider } from "@/lib/undo-context";
import { EntityFocusProvider, useRestoreFocus } from "@/lib/entity-focus-context";
import { SchemaProvider } from "@/lib/schema-context";
import { FieldUpdateProvider } from "@/lib/field-update-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { InspectProvider } from "@/lib/inspect-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import { Toaster } from "sonner";
import { InitProgressListener } from "@/components/init-progress-listener";
import { AppShell } from "@/components/app-shell";
import { NavBar } from "@/components/nav-bar";
import { LeftNav } from "@/components/left-nav";
import { ModeIndicator } from "@/components/mode-indicator";
import { BoardView } from "@/components/board-view";
import { GridView } from "@/components/grid-view";
import { EntityInspector } from "@/components/entity-inspector";
import { SlidePanel } from "@/components/slide-panel";
import { ViewsProvider, useViews } from "@/lib/views-context";
import { CommandScopeProvider, ActiveBoardPathProvider, type CommandDef } from "@/lib/command-scope";
import { DragSessionProvider } from "@/lib/drag-session-context";
import type {
  BoardData, OpenBoard, Entity, EntityBag,
} from "@/types/kanban";
import { entityFromBag, getStr } from "@/types/kanban";
import { refreshBoards } from "@/lib/refresh";
import { QuickCapture } from "@/components/quick-capture";

/** Parse URL params once at module level. */
const URL_PARAMS = new URLSearchParams(window.location.search);

/** Detect if this window instance is the quick-capture popup. */
const IS_QUICK_CAPTURE = URL_PARAMS.get("window") === "quick-capture";

/** Initial board path from URL (set when opening a new window for a specific board). */
const INITIAL_BOARD_PATH = URL_PARAMS.get("board") ?? undefined;

/** Window label for per-window state persistence. */
const WINDOW_LABEL = getCurrentWindow().label;

// Mark <html> so CSS can make the quick-capture window fully transparent.
if (IS_QUICK_CAPTURE) {
  document.documentElement.setAttribute("data-quick-capture", "");
}

const PANEL_WIDTH = 420;

/** Payload for entity-created Tauri event. */
interface EntityCreatedEvent {
  kind: "entity-created";
  entity_type: string;
  id: string;
  fields: Record<string, unknown>;
  board_path?: string;
}

/** Payload for entity-removed Tauri event. */
interface EntityRemovedEvent {
  kind: "entity-removed";
  entity_type: string;
  id: string;
  board_path?: string;
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
  board_path?: string;
}

/** A panel entry is just an entity reference — entity type + id. */
interface PanelEntry {
  entityType: string;
  entityId: string;
}

function App() {
  const [board, setBoard] = useState<BoardData | null>(null);
  /** All list-type entities keyed by type (task, tag, actor, ...). */
  const [entitiesByType, setEntitiesByType] = useState<Record<string, Entity[]>>({});
  const setEntitiesFor = useCallback(
    (type: string, updater: (prev: Entity[]) => Entity[]) =>
      setEntitiesByType((prev) => ({ ...prev, [type]: updater(prev[type] ?? []) })),
    [],
  );
  const [openBoards, setOpenBoards] = useState<OpenBoard[]>([]);
  /** Per-window active board path. Secondary windows get it from URL; main restores from backend. */
  const [activeBoardPath, setActiveBoardPath] = useState<string | undefined>(INITIAL_BOARD_PATH);
  const activeBoardPathRef = useRef(activeBoardPath);
  activeBoardPathRef.current = activeBoardPath;

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

  // Intentional empty deps: reads activeBoardPathRef to avoid stale closure.
  // The ref is kept in sync with state inside the callback.
  const refresh = useCallback(async () => {
    const result = await refreshBoards(activeBoardPathRef.current);
    // Open boards always update — even if board data failed.
    setOpenBoards(result.openBoards);

    // Pick or fall back to a valid active board path. Handles both initial
    // mount (no path yet) and board-closed (path no longer in open list).
    const currentPath = activeBoardPathRef.current;
    const pathStillOpen = currentPath && result.openBoards.some((b) => b.path === currentPath);
    if ((!currentPath || !pathStillOpen) && result.openBoards.length > 0) {
      const active = result.openBoards.find((b) => b.is_active) ?? result.openBoards[0];
      setActiveBoardPath(active.path);
      activeBoardPathRef.current = active.path;
      // Persist the fallback selection so it survives hot reload
      invoke("switch_board", { windowLabel: WINDOW_LABEL, path: active.path }).catch(() => {});
    }

    if (result.openBoards.length === 0) {
      // All boards closed — clear stale state so the placeholder shows.
      setBoard(null);
      setEntitiesByType({});
      setActiveBoardPath(undefined);
      return;
    }
    // Update board data and entities atomically. If board data arrives
    // but entities fail, clear entities rather than leaving stale data
    // from a previous board.
    setBoard(result.boardData);
    setEntitiesByType(result.entitiesByType ?? {});
  }, []);

  // Restore window state from backend on mount.
  // For main window: reads board_path + inspector_stack from config.
  // For secondary windows: board comes from URL param, this restores inspector.
  useEffect(() => {
    let cancelled = false;
    (async () => {
      // Restore board path from backend config (main window only — secondary gets it from URL)
      if (!INITIAL_BOARD_PATH) {
        try {
          const ctx = await invoke<{ board_path: string | null; inspector_stack: string[] }>(
            "get_ui_context",
            { windowLabel: WINDOW_LABEL },
          );
          if (cancelled) return;
          if (ctx.board_path) {
            // Open board idempotently and persist window→board mapping
            await invoke("switch_board", { windowLabel: WINDOW_LABEL, path: ctx.board_path });
            if (cancelled) return;
            setActiveBoardPath(ctx.board_path);
            activeBoardPathRef.current = ctx.board_path;
          }
          // Restore inspector stack
          if (ctx.inspector_stack?.length > 0) {
            const validated: PanelEntry[] = [];
            for (const moniker of ctx.inspector_stack) {
              const sep = moniker.indexOf(":");
              if (sep < 0) continue;
              validated.push({ entityType: moniker.slice(0, sep), entityId: moniker.slice(sep + 1) });
            }
            if (validated.length > 0) setPanelStack(validated);
          }
        } catch {
          // No saved state — will fall through to refresh below
        }
      }
      if (cancelled) return;
      await refresh();
      if (cancelled) return;

      // Main window restores secondary windows once boards are loaded
      if (!INITIAL_BOARD_PATH) {
        invoke("restore_windows").catch(() => {});
      }
    })();
    return () => { cancelled = true; };
  }, [refresh]);

  // Persist inspector stack whenever it changes (after initial mount)
  const inspectorInitialized = useRef(false);
  useEffect(() => {
    if (!inspectorInitialized.current) {
      inspectorInitialized.current = true;
      return;
    }
    const monikers = panelStack.map((e) => `${e.entityType}:${e.entityId}`);
    invoke("set_inspector_stack", { stack: monikers, windowLabel: WINDOW_LABEL }).catch(() => {});
  }, [panelStack]);

  // ---------------------------------------------------------------------------
  // Granular entity event listeners — patch local state surgically instead
  // of doing a full refresh.
  // ---------------------------------------------------------------------------

  useEffect(() => {
    const unlisteners = [
      listen<EntityCreatedEvent>("entity-created", (event) => {
        const { entity_type, id, board_path } = event.payload;
        if (board_path && activeBoardPathRef.current && board_path !== activeBoardPathRef.current) return;
        if (entity_type === "column" || entity_type === "swimlane") {
          refresh();
          return;
        }
        invoke<EntityBag>("get_entity", {
          entityType: entity_type,
          id,
          ...(activeBoardPathRef.current ? { boardPath: activeBoardPathRef.current } : {}),
        })
          .then((bag) => {
            const entity = entityFromBag(bag);
            setEntitiesFor(entity_type, (prev) => {
              if (prev.some((e) => e.id === id)) {
                return prev.map((e) => (e.id === id ? entity : e));
              }
              return [...prev, entity];
            });
          })
          .catch((err) => {
            console.error(`[entity-created] Failed to fetch ${entity_type}/${id}:`, err);
          });
      }),
      listen<EntityRemovedEvent>("entity-removed", (event) => {
        const { entity_type, id, board_path } = event.payload;
        if (board_path && activeBoardPathRef.current && board_path !== activeBoardPathRef.current) return;
        if (entity_type === "column" || entity_type === "swimlane") {
          refresh();
        } else {
          setEntitiesFor(entity_type, (prev) => prev.filter((e) => e.id !== id));
        }
      }),
      listen<EntityFieldChangedEvent>("entity-field-changed", (event) => {
        const { entity_type, id, fields: fullFields, board_path } = event.payload;
        if (board_path && activeBoardPathRef.current && board_path !== activeBoardPathRef.current) return;

        const applyEntity = (entity: Entity) => {
          const replaceById = (entities: Entity[]) =>
            entities.map((e) => (e.id === id ? entity : e));

          if (entity_type === "board") {
            setBoard((prev) => (prev ? { ...prev, board: entity } : prev));
          } else if (entity_type === "column") {
            setBoard((prev) => (prev ? { ...prev, columns: replaceById(prev.columns) } : prev));
          } else if (entity_type === "swimlane") {
            setBoard((prev) => (prev ? { ...prev, swimlanes: replaceById(prev.swimlanes) } : prev));
          } else {
            setEntitiesFor(entity_type, replaceById);
          }
        };

        if (fullFields) {
          applyEntity({ entity_type, id, fields: fullFields });
        } else {
          invoke<EntityBag>("get_entity", {
            entityType: entity_type,
            id,
            ...(activeBoardPathRef.current ? { boardPath: activeBoardPathRef.current } : {}),
          })
            .then((bag) => applyEntity(entityFromBag(bag)))
            .catch((err) => {
              console.error(`[entity-field-changed] Failed to fetch ${entity_type}/${id}:`, err);
            });
        }
      }),
      // board-opened: emitted only to the window that initiated the open (via emit_to).
      // Use window-scoped listen for defense-in-depth.
      getCurrentWindow().listen<{ path: string }>("board-opened", async (event: { payload: { path: string } }) => {
        const newPath = event.payload.path;
        // Persist window→board mapping so it survives hot reload / restart
        invoke("switch_board", { windowLabel: WINDOW_LABEL, path: newPath }).catch(() => {});
        setActiveBoardPath(newPath);
        activeBoardPathRef.current = newPath;
        const result = await refreshBoards(newPath);
        setOpenBoards(result.openBoards);
        setBoard(result.boardData);
        setEntitiesByType(result.entitiesByType ?? {});
      }),
      // board-changed: structural change (open/close/switch). All windows
      // refresh their open boards list. If this window's board was closed,
      // fall back to another open board.
      listen("board-changed", async () => {
        let boards: OpenBoard[] = [];
        try {
          boards = await invoke<OpenBoard[]>("list_open_boards");
        } catch { /* ignore */ }
        setOpenBoards(boards);

        if (boards.length === 0) {
          setBoard(null);
          setEntitiesByType({});
          setActiveBoardPath(undefined);
          return;
        }

        // If this window's board is still open, keep it and refresh data
        const currentPath = activeBoardPathRef.current;
        const stillOpen = currentPath && boards.some((b) => b.path === currentPath);
        if (stillOpen) {
          const result = await refreshBoards(currentPath);
          setBoard(result.boardData);
          setEntitiesByType(result.entitiesByType ?? {});
          return;
        }

        // Board was closed — fall back to another open board and persist
        const fallback = boards.find((b) => b.is_active) ?? boards[0];
        setActiveBoardPath(fallback.path);
        activeBoardPathRef.current = fallback.path;
        invoke("switch_board", { windowLabel: WINDOW_LABEL, path: fallback.path }).catch(() => {});
        const result = await refreshBoards(fallback.path);
        setBoard(result.boardData);
        setEntitiesByType(result.entitiesByType ?? {});
      }),
    ];
    return () => {
      for (const p of unlisteners) {
        p.then((fn: () => void) => fn());
      }
    };
  }, [refresh]);

  /** Switch this window's active board. Persists via backend switch_board command. */
  const handleSwitchBoard = useCallback(async (path: string) => {
    setActiveBoardPath(path);
    activeBoardPathRef.current = path;
    try {
      await invoke("switch_board", { windowLabel: WINDOW_LABEL, path });
    } catch { /* ignore */ }
    refresh();
  }, [refresh]);

  const entityStore = useMemo(() => ({
    ...entitiesByType,
    column: board?.columns ?? [],
    swimlane: board?.swimlanes ?? [],
  }), [entitiesByType, board]);

  return (
    <TooltipProvider delayDuration={400}>
    <Toaster position="bottom-right" richColors />
    <InitProgressListener />
    <ActiveBoardPathProvider value={activeBoardPath}>
    <SchemaProvider>
    <EntityStoreProvider entities={entityStore}>
    <EntityFocusProvider>
    <FieldUpdateProvider>
    <KeymapProvider>
    <AppModeProvider>
    <UndoStackProvider>
    <InspectProvider onInspect={inspectEntity} onDismiss={dismissTopPanel}>
    <AppShell openBoards={openBoards} onSwitchBoard={handleSwitchBoard}>
    <DragSessionProvider>
    <ViewsProvider>
    <ViewCommandScope>
    <div className="h-screen bg-background text-foreground flex flex-col">
      <NavBar
        board={board}
        openBoards={openBoards}
        activeBoardPath={activeBoardPath}
        onSwitchBoard={handleSwitchBoard}
        onBoardInspect={() => inspectEntity("board", "board")}
      />
      {board ? (
        <>
          <div className="flex-1 flex min-h-0">
            <LeftNav />
            <ActiveViewRenderer
              board={board}
              tasks={entitiesByType.task ?? []}
            />
          </div>

          {/* Backdrop — visible when any panel is open */}
          <div
            className={`fixed inset-0 z-20 bg-black/20 transition-opacity duration-200 ${
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
          <div className="text-center space-y-3">
            <p className="text-muted-foreground text-lg">No board loaded</p>
            <div className="text-sm text-muted-foreground/70 space-y-1">
              <p><kbd className="px-1.5 py-0.5 rounded bg-muted text-xs font-mono">Cmd+N</kbd> New Board</p>
              <p><kbd className="px-1.5 py-0.5 rounded bg-muted text-xs font-mono">Cmd+O</kbd> Open Board</p>
            </div>
          </div>
        </main>
      )}
      <ModeIndicator />
    </div>
    </ViewCommandScope>
    </ViewsProvider>
    </DragSessionProvider>
    </AppShell>
    </InspectProvider>
    </UndoStackProvider>
    </AppModeProvider>
    </KeymapProvider>
    </FieldUpdateProvider>
    </EntityFocusProvider>
    </EntityStoreProvider>
    </SchemaProvider>
    </ActiveBoardPathProvider>
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
  // Save focus on mount, restore on unmount (guarded against stale monikers)
  useRestoreFocus();

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

/**
 * Quick-capture window renders a minimal provider tree — just keymap context
 * (for vim/CUA mode awareness) wrapping the capture form.
 *
 * Sets body/html to transparent so the borderless window shows only the
 * styled card with rounded corners and shadow.
 */
function QuickCaptureApp() {
  useEffect(() => {
    document.documentElement.style.background = "transparent";
    document.body.style.background = "transparent";
  }, []);

  return (
    <SchemaProvider>
    <EntityStoreProvider entities={{}}>
    <FieldUpdateProvider>
    <KeymapProvider>
      <QuickCapture />
    </KeymapProvider>
    </FieldUpdateProvider>
    </EntityStoreProvider>
    </SchemaProvider>
  );
}

export default IS_QUICK_CAPTURE ? QuickCaptureApp : App;
