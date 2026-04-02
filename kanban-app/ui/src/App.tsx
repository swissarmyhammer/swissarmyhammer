// Field type registrations — must be imported before any Field renders
import "@/components/fields/registrations";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { UIStateProvider, useUIState } from "@/lib/ui-state-context";
import { AppModeProvider } from "@/lib/app-mode-context";
import { UndoProvider } from "@/lib/undo-context";
import {
  EntityFocusProvider,
  useRestoreFocus,
} from "@/lib/entity-focus-context";
import { SchemaProvider, useSchema } from "@/lib/schema-context";
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
import { Loader2 } from "lucide-react";
import { BoardView } from "@/components/board-view";
import { GridView } from "@/components/grid-view";
import { InspectorFocusBridge } from "@/components/inspector-focus-bridge";
import { SlidePanel } from "@/components/slide-panel";
import { ViewsProvider, useViews } from "@/lib/views-context";
import { PerspectiveProvider } from "@/lib/perspective-context";
import {
  CommandScopeProvider,
  ActiveBoardPathProvider,
  dispatchCommand,
  backendDispatch,
  type CommandDef,
} from "@/lib/command-scope";
import { DragSessionProvider } from "@/lib/drag-session-context";
import { FileDropProvider } from "@/lib/file-drop-context";
import type { BoardData, OpenBoard, Entity, EntityBag } from "@/types/kanban";
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

/**
 * Bridge component that syncs the backend UIState inspector_stack to the
 * local panelStack state. Must render inside UIStateProvider so useUIState()
 * returns real data.
 *
 * Context menu and palette dispatches go directly to the Rust backend (bypassing
 * the React command execute callbacks), so the only way to detect inspector_stack
 * changes is by reactively reading UIState.
 */
function InspectorSyncBridge({
  setPanelStack,
}: {
  setPanelStack: React.Dispatch<React.SetStateAction<PanelEntry[]>>;
}) {
  const uiState = useUIState();
  const winState = uiState.windows?.[WINDOW_LABEL];
  const inspectorStack = winState?.inspector_stack;

  useEffect(() => {
    if (!inspectorStack) return;
    const entries: PanelEntry[] = [];
    for (const m of inspectorStack) {
      const sep = m.indexOf(":");
      if (sep < 0) continue;
      entries.push({
        entityType: m.slice(0, sep),
        entityId: m.slice(sep + 1),
      });
    }
    setPanelStack(entries);
  }, [inspectorStack, setPanelStack]);

  return null;
}

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
  board_path?: string;
}

/** A panel entry is just an entity reference — entity type + id. */
interface PanelEntry {
  entityType: string;
  entityId: string;
}

function App() {
  const [board, setBoard] = useState<BoardData | null>(null);
  const [loading, setLoading] = useState(true);
  /** All list-type entities keyed by type (task, tag, actor, ...). */
  const [entitiesByType, setEntitiesByType] = useState<
    Record<string, Entity[]>
  >({});
  const setEntitiesFor = useCallback(
    (type: string, updater: (prev: Entity[]) => Entity[]) =>
      setEntitiesByType((prev) => ({
        ...prev,
        [type]: updater(prev[type] ?? []),
      })),
    [],
  );
  const [openBoards, setOpenBoards] = useState<OpenBoard[]>([]);
  /** Per-window active board path. Secondary windows get it from URL; main restores from backend. */
  const [activeBoardPath, setActiveBoardPath] = useState<string | undefined>(
    INITIAL_BOARD_PATH,
  );
  const activeBoardPathRef = useRef(activeBoardPath);
  activeBoardPathRef.current = activeBoardPath;

  const [panelStack, setPanelStack] = useState<PanelEntry[]>([]);
  const panelStackRef = useRef(panelStack);
  panelStackRef.current = panelStack;

  // Scope chain for this window — used by direct backendDispatch calls
  // so the backend knows which window's inspector stack to modify.
  const windowScopeChain = useMemo(() => [`window:${WINDOW_LABEL}`], []);

  /** Open an inspector for any entity via the command architecture.
   *  Fire-and-forget — InspectorSyncBridge updates panelStack from UIState. */
  const inspectEntity = useCallback(
    (entityType: string, entityId: string) => {
      backendDispatch({
        cmd: "ui.inspect",
        target: `${entityType}:${entityId}`,
        scopeChain: windowScopeChain,
      }).catch((e) => console.error("ui.inspect failed:", e));
    },
    [windowScopeChain],
  );

  /** Close the topmost inspector panel via the command architecture.
   *  Fire-and-forget — InspectorSyncBridge updates panelStack from UIState. */
  const closeTopPanel = useCallback(() => {
    backendDispatch({
      cmd: "ui.inspector.close",
      scopeChain: windowScopeChain,
    }).catch((e) => console.error("ui.inspector.close failed:", e));
  }, [windowScopeChain]);

  /** Close the topmost panel. Returns true if a panel was actually closed.
   *  Fire-and-forget — InspectorSyncBridge updates panelStack from UIState. */
  const dismissTopPanel = useCallback((): boolean => {
    if (panelStackRef.current.length === 0) return false;
    backendDispatch({
      cmd: "ui.inspector.close",
      scopeChain: windowScopeChain,
    }).catch((e) => console.error("ui.inspector.close failed:", e));
    return true;
  }, [windowScopeChain]);

  /** Close all inspector panels via the command architecture.
   *  Fire-and-forget — InspectorSyncBridge updates panelStack from UIState. */
  const closeAll = useCallback(() => {
    backendDispatch({
      cmd: "ui.inspector.close_all",
      scopeChain: windowScopeChain,
    }).catch((e) => console.error("ui.inspector.close_all failed:", e));
  }, []);

  // Intentional empty deps: reads activeBoardPathRef to avoid stale closure.
  // The ref is kept in sync with state inside the callback.
  const refresh = useCallback(async () => {
    setLoading(true);
    const result = await refreshBoards(activeBoardPathRef.current);
    // Open boards always update — even if board data failed.
    setOpenBoards(result.openBoards);

    // Pick or fall back to a valid active board path. Handles both initial
    // mount (no path yet) and board-closed (path no longer in open list).
    const currentPath = activeBoardPathRef.current;
    const pathStillOpen =
      currentPath && result.openBoards.some((b) => b.path === currentPath);
    if ((!currentPath || !pathStillOpen) && result.openBoards.length > 0) {
      const active =
        result.openBoards.find((b) => b.is_active) ?? result.openBoards[0];
      setActiveBoardPath(active.path);
      activeBoardPathRef.current = active.path;
      // Persist the fallback selection so it survives hot reload
      backendDispatch({
        cmd: "file.switchBoard",
        args: { windowLabel: WINDOW_LABEL, path: active.path },
        scopeChain: windowScopeChain,
      }).catch(() => {});
    }

    if (result.openBoards.length === 0) {
      // All boards closed — clear stale state so the placeholder shows.
      setBoard(null);
      setEntitiesByType({});
      setActiveBoardPath(undefined);
      setLoading(false);
      return;
    }
    // Update board data and entities atomically. If board data arrives
    // but entities fail, clear entities rather than leaving stale data
    // from a previous board.
    setBoard(result.boardData);
    setEntitiesByType(result.entitiesByType ?? {});
    setLoading(false);
  }, []);

  // Restore window state from backend on mount.
  // For main window: reads board_path + inspector_stack from config.
  // For secondary windows: board comes from URL param, this restores inspector.
  useEffect(() => {
    let cancelled = false;
    (async () => {
      // Restore window state from UIState (board path + inspector stack).
      // UIState.windows[windowLabel] holds all per-window state; no need for a
      // separate get_ui_context command.
      try {
        const uiState = await invoke<{
          windows: Record<
            string,
            {
              board_path?: string;
              inspector_stack?: string[];
              active_view_id?: string;
            }
          >;
        }>("get_ui_state");
        if (cancelled) return;
        const winState = uiState.windows?.[WINDOW_LABEL];

        // Restore board path from backend config (main window only — secondary gets it from URL)
        if (!INITIAL_BOARD_PATH && winState?.board_path) {
          // Open board idempotently and persist window→board mapping
          await backendDispatch({
            cmd: "file.switchBoard",
            args: { windowLabel: WINDOW_LABEL, path: winState.board_path },
            scopeChain: windowScopeChain,
          });
          if (cancelled) return;
          setActiveBoardPath(winState.board_path);
          activeBoardPathRef.current = winState.board_path;
        }

        // Inspector stack restore is handled by InspectorSyncBridge
        // reading UIState after UIStateProvider mounts.
      } catch {
        // No saved state — will fall through to refresh below
      }
      if (cancelled) return;
      await refresh();
      if (cancelled) return;

      // Secondary windows are restored by the Rust backend in setup() —
      // no frontend invoke needed. Each restore goes through the same
      // create_window_impl path as window.new.
    })();
    return () => {
      cancelled = true;
    };
  }, [refresh]);

  // ---------------------------------------------------------------------------
  // Granular entity event listeners — patch local state surgically instead
  // of doing a full refresh.
  // ---------------------------------------------------------------------------

  useEffect(() => {
    const unlisteners = [
      listen<EntityCreatedEvent>("entity-created", (event) => {
        const { entity_type, id, board_path } = event.payload;
        console.warn(
          `[entity-created] received: ${entity_type}/${id} board_path=${board_path ?? "none"}`,
        );
        if (
          board_path &&
          activeBoardPathRef.current &&
          board_path !== activeBoardPathRef.current
        ) {
          console.warn(
            `[entity-created] SKIPPED: board_path mismatch (active=${activeBoardPathRef.current})`,
          );
          return;
        }
        if (entity_type === "column" || entity_type === "swimlane") {
          console.warn(`[entity-created] structural type → full refresh`);
          refresh();
          return;
        }
        console.warn(
          `[entity-created] fetching ${entity_type}/${id} via get_entity`,
        );
        invoke<EntityBag>("get_entity", {
          entityType: entity_type,
          id,
          ...(activeBoardPathRef.current
            ? { boardPath: activeBoardPathRef.current }
            : {}),
        })
          .then((bag) => {
            const entity = entityFromBag(bag);
            console.warn(
              `[entity-created] fetched ${entity_type}/${id}, fields: ${Object.keys(entity.fields).join(",")}`,
            );
            setEntitiesFor(entity_type, (prev) => {
              if (prev.some((e) => e.id === id)) {
                return prev.map((e) => (e.id === id ? entity : e));
              }
              return [...prev, entity];
            });
          })
          .catch((err) => {
            console.error(
              `[entity-created] Failed to fetch ${entity_type}/${id}:`,
              err,
            );
          });
      }),
      listen<EntityRemovedEvent>("entity-removed", (event) => {
        const { entity_type, id, board_path } = event.payload;
        console.warn(
          `[entity-removed] received: ${entity_type}/${id} board_path=${board_path ?? "none"}`,
        );
        if (
          board_path &&
          activeBoardPathRef.current &&
          board_path !== activeBoardPathRef.current
        ) {
          console.warn(`[entity-removed] SKIPPED: board_path mismatch`);
          return;
        }
        if (entity_type === "column" || entity_type === "swimlane") {
          refresh();
        } else {
          setEntitiesFor(entity_type, (prev) =>
            prev.filter((e) => e.id !== id),
          );
        }
      }),
      listen<EntityFieldChangedEvent>("entity-field-changed", (event) => {
        const { entity_type, id, board_path } = event.payload;
        console.warn(
          `[entity-field-changed] received: ${entity_type}/${id} board_path=${board_path ?? "none"}`,
        );
        if (
          board_path &&
          activeBoardPathRef.current &&
          board_path !== activeBoardPathRef.current
        ) {
          console.warn(
            `[entity-field-changed] SKIPPED: board_path mismatch (active=${activeBoardPathRef.current})`,
          );
          return;
        }

        // Events are signals to re-fetch, not data carriers. Always fetch
        // fresh state from the backend so both command-path and watcher-path
        // events behave identically.
        console.warn(
          `[entity-field-changed] fetching ${entity_type}/${id} via get_entity`,
        );
        invoke<EntityBag>("get_entity", {
          entityType: entity_type,
          id,
          ...(activeBoardPathRef.current
            ? { boardPath: activeBoardPathRef.current }
            : {}),
        })
          .then((bag) => {
            const entity = entityFromBag(bag);
            console.warn(
              `[entity-field-changed] fetched ${entity_type}/${id}, fields: ${Object.keys(entity.fields).join(",")}`,
            );
            const replaceById = (entities: Entity[]) =>
              entities.map((e) => (e.id === id ? entity : e));

            setEntitiesFor(entity_type, replaceById);

            if (entity_type === "board") {
              setBoard((prev) => (prev ? { ...prev, board: entity } : prev));
            } else if (entity_type === "column") {
              setBoard((prev) =>
                prev ? { ...prev, columns: replaceById(prev.columns) } : prev,
              );
            } else if (entity_type === "swimlane") {
              setBoard((prev) =>
                prev
                  ? { ...prev, swimlanes: replaceById(prev.swimlanes) }
                  : prev,
              );
            }
          })
          .catch((err) => {
            console.error(
              `[entity-field-changed] Failed to fetch ${entity_type}/${id}:`,
              err,
            );
          });
      }),
      // board-opened: emitted only to the window that initiated the open (via emit_to).
      // Use window-scoped listen for defense-in-depth.
      getCurrentWindow().listen<{ path: string }>(
        "board-opened",
        async (event: { payload: { path: string } }) => {
          const newPath = event.payload.path;
          // Persist window→board mapping so it survives hot reload / restart
          backendDispatch({
            cmd: "file.switchBoard",
            args: { windowLabel: WINDOW_LABEL, path: newPath },
            scopeChain: windowScopeChain,
          }).catch(() => {});
          setActiveBoardPath(newPath);
          activeBoardPathRef.current = newPath;
          setLoading(true);
          const result = await refreshBoards(newPath);
          setOpenBoards(result.openBoards);
          setBoard(result.boardData);
          setEntitiesByType(result.entitiesByType ?? {});
          setLoading(false);
        },
      ),
      // board-changed: structural change (open/close/switch). All windows
      // refresh their open boards list. If this window's board was closed,
      // fall back to another open board. Also checks UIState for board
      // assignment changes (e.g. Open Board dialog switched this window's
      // board but board-opened was not received).
      listen("board-changed", async () => {
        let boards: OpenBoard[] = [];
        try {
          boards = await invoke<OpenBoard[]>("list_open_boards");
        } catch {
          /* ignore */
        }
        setOpenBoards(boards);

        if (boards.length === 0) {
          setBoard(null);
          setEntitiesByType({});
          setActiveBoardPath(undefined);
          setLoading(false);
          return;
        }

        // Check if UIState says this window should show a different board
        // (e.g. file.switchBoard ran but board-opened was not delivered).
        let assignedPath: string | undefined;
        try {
          const uiState = await invoke<{
            windows: Record<string, { board_path?: string }>;
          }>("get_ui_state");
          assignedPath = uiState.windows?.[WINDOW_LABEL]?.board_path;
        } catch {
          /* ignore */
        }

        const currentPath = activeBoardPathRef.current;

        // If the backend assigned a different board to this window, switch.
        if (
          assignedPath &&
          assignedPath !== currentPath &&
          boards.some((b) => b.path === assignedPath)
        ) {
          setActiveBoardPath(assignedPath);
          activeBoardPathRef.current = assignedPath;
          setLoading(true);
          const result = await refreshBoards(assignedPath);
          setOpenBoards(result.openBoards);
          setBoard(result.boardData);
          setEntitiesByType(result.entitiesByType ?? {});
          setLoading(false);
          return;
        }

        // If this window's board is still open, keep it and refresh data
        const stillOpen =
          currentPath && boards.some((b) => b.path === currentPath);
        if (stillOpen) {
          setLoading(true);
          const result = await refreshBoards(currentPath);
          setBoard(result.boardData);
          setEntitiesByType(result.entitiesByType ?? {});
          setLoading(false);
          return;
        }

        // Board was closed — fall back to another open board and persist
        const fallback = boards.find((b) => b.is_active) ?? boards[0];
        setActiveBoardPath(fallback.path);
        activeBoardPathRef.current = fallback.path;
        backendDispatch({
          cmd: "file.switchBoard",
          args: { windowLabel: WINDOW_LABEL, path: fallback.path },
          scopeChain: windowScopeChain,
        }).catch(() => {});
        setLoading(true);
        const result = await refreshBoards(fallback.path);
        setBoard(result.boardData);
        setEntitiesByType(result.entitiesByType ?? {});
        setLoading(false);
      }),
    ];
    return () => {
      for (const p of unlisteners) {
        p.then((fn: () => void) => fn());
      }
    };
  }, [refresh]);

  /** Switch this window's active board. Persists via backend file.switchBoard command. */
  const handleSwitchBoard = useCallback(
    async (path: string) => {
      setActiveBoardPath(path);
      activeBoardPathRef.current = path;
      try {
        await backendDispatch({
          cmd: "file.switchBoard",
          args: { windowLabel: WINDOW_LABEL, path },
          scopeChain: windowScopeChain,
        });
      } catch {
        /* ignore */
      }
      refresh();
    },
    [refresh],
  );

  const entityStore = useMemo(() => entitiesByType, [entitiesByType]);

  return (
    <CommandScopeProvider commands={[]} moniker={`window:${WINDOW_LABEL}`}>
      <TooltipProvider delayDuration={400}>
        <Toaster position="bottom-right" richColors />
        <InitProgressListener />
        <ActiveBoardPathProvider value={activeBoardPath}>
          <SchemaProvider>
            <EntityStoreProvider entities={entityStore}>
              <EntityFocusProvider>
                <FieldUpdateProvider>
                  <UIStateProvider>
                    <InspectorSyncBridge setPanelStack={setPanelStack} />
                    <AppModeProvider>
                      <UndoProvider>
                        <InspectProvider
                          onInspect={inspectEntity}
                          onDismiss={dismissTopPanel}
                        >
                          <FileDropProvider>
                            <AppShell
                              openBoards={openBoards}
                              onSwitchBoard={handleSwitchBoard}
                            >
                              <DragSessionProvider>
                                <ViewsProvider>
                                  <PerspectiveProvider>
                                    <ViewCommandScope>
                                      <div className="h-screen bg-background text-foreground flex flex-col">
                                        <NavBar
                                          board={board}
                                          openBoards={openBoards}
                                          activeBoardPath={activeBoardPath}
                                          onSwitchBoard={handleSwitchBoard}
                                        />
                                        {board && activeBoardPath ? (
                                          <>
                                            <div className="flex-1 flex min-h-0">
                                              <LeftNav />
                                              <div className="flex-1 min-w-0 overflow-hidden flex flex-col">
                                                <ActiveViewRenderer
                                                  board={board}
                                                  tasks={
                                                    entitiesByType.task ?? []
                                                  }
                                                  boardPath={activeBoardPath}
                                                />
                                              </div>
                                            </div>

                                            {/* Backdrop — visible when any panel is open */}
                                            <div
                                              className={`fixed inset-0 z-20 bg-black/20 transition-opacity duration-200 ${
                                                panelStack.length > 0
                                                  ? "opacity-100"
                                                  : "opacity-0 pointer-events-none"
                                              }`}
                                              onClick={() => {
                                                dispatchCommand(
                                                  {
                                                    id: "ui.inspector.close_all",
                                                    name: "Close All Inspectors",
                                                    execute: closeAll,
                                                  },
                                                  undefined,
                                                  [],
                                                );
                                              }}
                                            />

                                            {/* Render inspector panels from the stack */}
                                            {panelStack.map((entry, index) => {
                                              const rightOffset =
                                                (panelStack.length -
                                                  1 -
                                                  index) *
                                                PANEL_WIDTH;
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
                                        ) : loading ? (
                                          <main className="flex-1 flex items-center justify-center">
                                            <Loader2 className="h-8 w-8 text-muted-foreground/50 animate-spin [animation-delay:200ms] [animation-fill-mode:backwards]" />
                                          </main>
                                        ) : (
                                          <main className="flex-1 flex items-center justify-center">
                                            <div className="text-center space-y-3">
                                              <p className="text-muted-foreground text-lg">
                                                No board loaded
                                              </p>
                                              <div className="text-sm text-muted-foreground/70 space-y-1">
                                                <p>
                                                  <kbd className="px-1.5 py-0.5 rounded bg-muted text-xs font-mono">
                                                    Cmd+N
                                                  </kbd>{" "}
                                                  New Board
                                                </p>
                                                <p>
                                                  <kbd className="px-1.5 py-0.5 rounded bg-muted text-xs font-mono">
                                                    Cmd+O
                                                  </kbd>{" "}
                                                  Open Board
                                                </p>
                                              </div>
                                            </div>
                                          </main>
                                        )}
                                        <ModeIndicator />
                                      </div>
                                    </ViewCommandScope>
                                  </PerspectiveProvider>
                                </ViewsProvider>
                              </DragSessionProvider>
                            </AppShell>
                          </FileDropProvider>
                        </InspectProvider>
                      </UndoProvider>
                    </AppModeProvider>
                  </UIStateProvider>
                </FieldUpdateProvider>
              </EntityFocusProvider>
            </EntityStoreProvider>
          </SchemaProvider>
        </ActiveBoardPathProvider>
      </TooltipProvider>
    </CommandScopeProvider>
  );
}

/**
 * Provides view.switch commands generated from the views registry.
 * Each view gets a `view.switch:<id>` command that dispatches through
 * the backend command system (which redirects to `ui.view.set`).
 */
function ViewCommandScope({ children }: { children: React.ReactNode }) {
  const { views } = useViews();

  const viewCommands: CommandDef[] = useMemo(() => {
    return views.map((view) => ({
      id: `view.switch:${view.id}`,
      name: `View: ${view.name}`,
      execute: () => {
        backendDispatch({
          cmd: `view.switch:${view.id}`,
          scopeChain: [`window:${WINDOW_LABEL}`],
        }).catch(console.error);
      },
    }));
  }, [views]);

  return (
    <CommandScopeProvider commands={viewCommands}>
      {children}
    </CommandScopeProvider>
  );
}

/** Props for the ActiveViewRenderer component. */
interface ViewRouterProps {
  board: BoardData;
  tasks: Entity[];
  boardPath?: string;
}

/**
 * Renders the currently active view based on its kind.
 * For "board" kind, renders the BoardView. Other kinds show a placeholder.
 */
function ActiveViewRenderer({ board, tasks, boardPath }: ViewRouterProps) {
  const { activeView } = useViews();

  if (!activeView || activeView.kind === "board") {
    return <BoardView board={board} tasks={tasks} boardPath={boardPath} />;
  }

  if (activeView.kind === "grid") {
    return <GridView view={activeView} />;
  }

  return (
    <main className="flex-1 flex items-center justify-center">
      <p className="text-muted-foreground">
        {activeView.name} view ({activeView.kind}) is not yet implemented.
      </p>
    </main>
  );
}

/** Props for the InspectorPanel component. */
interface InspectorPanelProps {
  entry: PanelEntry;
  entityStore: Record<string, Entity[]>;
  board: BoardData | null;
  onClose: () => void;
  style?: React.CSSProperties;
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
}: InspectorPanelProps) {
  // Save focus on mount, restore on unmount (guarded against stale monikers)
  useRestoreFocus();

  const { getSchema } = useSchema();
  const [fetchedEntity, setFetchedEntity] = useState<Entity | null>(null);
  const [fetchError, setFetchError] = useState<string | null>(null);
  const fetchedRef = useRef<string | null>(null);

  // Try local store first — match by ID, then by search_display_field from schema
  const entities = entityStore[entry.entityType];
  let localEntity = entities?.find((e) => e.id === entry.entityId);
  if (!localEntity) {
    const displayField = getSchema(entry.entityType)?.entity
      .search_display_field;
    if (displayField) {
      localEntity = entities?.find(
        (e) => getStr(e, displayField) === entry.entityId,
      );
    }
  }
  // Board entity is special
  const resolved =
    localEntity ??
    (entry.entityType === "board" ? board?.board : undefined) ??
    fetchedEntity;

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
        console.error(
          `[InspectorPanel] Failed to fetch entity: ${fetchKey}`,
          err,
        );
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
      <InspectorFocusBridge entity={resolved} />
    </SlidePanel>
  );
}

/**
 * Quick-capture window renders a minimal provider tree wrapping the capture
 * form.
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
          <UIStateProvider>
            <QuickCapture />
          </UIStateProvider>
        </FieldUpdateProvider>
      </EntityStoreProvider>
    </SchemaProvider>
  );
}

export default IS_QUICK_CAPTURE ? QuickCaptureApp : App;
