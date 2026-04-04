// Field type registrations — must be imported before any Field renders
import "@/components/fields/registrations";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useUIState } from "@/lib/ui-state-context";
import { useRestoreFocus } from "@/lib/entity-focus-context";
import { SchemaProvider, useSchema } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { FieldUpdateProvider } from "@/lib/field-update-context";
import { UIStateProvider } from "@/lib/ui-state-context";
import { NavBar } from "@/components/nav-bar";
import { LeftNav } from "@/components/left-nav";
import { ModeIndicator } from "@/components/mode-indicator";
import { ErrorBoundary } from "@/components/ui/error-boundary";
import { BoardView } from "@/components/board-view";
import { GridView } from "@/components/grid-view";
import { InspectorFocusBridge } from "@/components/inspector-focus-bridge";
import { SlidePanel } from "@/components/slide-panel";
import { ViewsProvider, useViews } from "@/lib/views-context";
import { PerspectiveProvider } from "@/lib/perspective-context";
import {
  CommandScopeProvider,
  useDispatchCommand,
  backendDispatch,
  type CommandDef,
} from "@/lib/command-scope";
import type { BoardData, Entity, EntityBag } from "@/types/kanban";
import { entityFromBag, getStr } from "@/types/kanban";
import { QuickCapture } from "@/components/quick-capture";
import { StoreContainer } from "@/components/store-container";
import {
  RustEngineContainer,
  useEntitiesByType,
} from "@/components/rust-engine-container";
import {
  WindowContainer,
  useBoardData,
  useWindowLoading,
  useActiveBoardPath,
  useOpenBoards,
  useHandleSwitchBoard,
} from "@/components/window-container";
import { BoardContainer } from "@/components/board-container";

/** Parse URL params once at module level. */
const URL_PARAMS = new URLSearchParams(window.location.search);

/** Detect if this window instance is the quick-capture popup. */
const IS_QUICK_CAPTURE = URL_PARAMS.get("window") === "quick-capture";

/** Window label for per-window state persistence. */
const WINDOW_LABEL = getCurrentWindow().label;

// Mark <html> so CSS can make the quick-capture window fully transparent.
if (IS_QUICK_CAPTURE) {
  document.documentElement.setAttribute("data-quick-capture", "");
}

const PANEL_WIDTH = 420;

/** A panel entry is just an entity reference — entity type + id. */
interface PanelEntry {
  entityType: string;
  entityId: string;
}

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

/**
 * Conditionally wraps children in a StoreContainer when a board path is
 * active. Injects a `store:{path}` moniker into the scope chain so the
 * backend can resolve the board handle from scope instead of an explicit
 * boardPath parameter.
 *
 * When path is undefined (no board loaded), renders children directly.
 */
function MaybeStoreScope({
  path,
  children,
}: {
  path: string | undefined;
  children: React.ReactNode;
}) {
  if (path) {
    return <StoreContainer path={path}>{children}</StoreContainer>;
  }
  return <>{children}</>;
}

/**
 * Main application shell: RustEngineContainer > WindowContainer > AppContent.
 *
 * RustEngineContainer owns entity state, entity event listeners, and all
 * Rust-bridge providers (Schema, EntityStore, EntityFocus, FieldUpdate,
 * UIState, AppMode, Undo).
 *
 * WindowContainer owns the window scope, board lifecycle, AppShell, and
 * board switching.
 *
 * AppContent (below) owns the inspector panel state and uses BoardContainer
 * for conditional board rendering.
 */
function App() {
  return (
    <RustEngineContainer>
      <WindowContainer>
        <AppContent />
      </WindowContainer>
    </RustEngineContainer>
  );
}

/**
 * Main content area that reads board state from container contexts and
 * renders the board or loading/placeholder states via BoardContainer.
 *
 * Owns:
 * - Inspector panel state (panelStack, InspectorSyncBridge)
 * - MaybeStoreScope for the active board path
 * - BoardContainer for conditional board rendering
 */
function AppContent() {
  const board = useBoardData();
  const loading = useWindowLoading();
  const activeBoardPath = useActiveBoardPath();
  const openBoards = useOpenBoards();
  const handleSwitchBoard = useHandleSwitchBoard();
  const entitiesByType = useEntitiesByType();

  const [panelStack, setPanelStack] = useState<PanelEntry[]>([]);

  const entityStore = useMemo(() => entitiesByType, [entitiesByType]);

  /** Close the topmost inspector panel via the command architecture.
   *  Fire-and-forget — InspectorSyncBridge updates panelStack from UIState. */
  const dispatchInspectorClose = useDispatchCommand("ui.inspector.close");
  const closeTopPanel = useCallback(() => {
    dispatchInspectorClose().catch((e) =>
      console.error("ui.inspector.close failed:", e),
    );
  }, [dispatchInspectorClose]);

  /** Close all inspector panels via the command architecture.
   *  Fire-and-forget — InspectorSyncBridge updates panelStack from UIState. */
  const dispatchInspectorCloseAll = useDispatchCommand(
    "ui.inspector.close_all",
  );
  const closeAll = useCallback(() => {
    dispatchInspectorCloseAll().catch((e) =>
      console.error("ui.inspector.close_all failed:", e),
    );
  }, [dispatchInspectorCloseAll]);

  return (
    <MaybeStoreScope path={activeBoardPath}>
      <InspectorSyncBridge setPanelStack={setPanelStack} />
      <BoardContainer>
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
                <div className="flex-1 flex min-h-0">
                  <LeftNav />
                  <div className="flex-1 min-w-0 overflow-hidden flex flex-col">
                    <ActiveViewRenderer
                      board={board!}
                      tasks={entitiesByType.task ?? []}
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
                  onClick={closeAll}
                />

                {/* Render inspector panels from the stack */}
                {panelStack.map((entry, index) => {
                  const rightOffset =
                    (panelStack.length - 1 - index) * PANEL_WIDTH;
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

                <ModeIndicator />
              </div>
            </ViewCommandScope>
          </PerspectiveProvider>
        </ViewsProvider>
      </BoardContainer>
    </MaybeStoreScope>
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
          {fetchError ? `Entity not found` : "Loading\u2026"}
        </p>
      </SlidePanel>
    );
  }

  return (
    <SlidePanel open={true} onClose={onClose} style={style}>
      <ErrorBoundary>
        <InspectorFocusBridge entity={resolved} />
      </ErrorBoundary>
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
