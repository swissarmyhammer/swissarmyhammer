// Field type registrations — must be imported before any Field renders
import "@/components/fields/registrations";

import { useEffect, useMemo } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { FieldUpdateProvider } from "@/lib/field-update-context";
import { UIStateProvider } from "@/lib/ui-state-context";
import { NavBar } from "@/components/nav-bar";
import { LeftNav } from "@/components/left-nav";
import { ModeIndicator } from "@/components/mode-indicator";
import { BoardView } from "@/components/board-view";
import { GridView } from "@/components/grid-view";
import { ViewsProvider, useViews } from "@/lib/views-context";
import { PerspectiveProvider } from "@/lib/perspective-context";
import {
  CommandScopeProvider,
  backendDispatch,
  type CommandDef,
} from "@/lib/command-scope";
import type { BoardData, Entity } from "@/types/kanban";
import { QuickCapture } from "@/components/quick-capture";
import { StoreContainer } from "@/components/store-container";
import {
  RustEngineContainer,
  useEntitiesByType,
} from "@/components/rust-engine-container";
import {
  WindowContainer,
  useBoardData,
  useActiveBoardPath,
  useOpenBoards,
  useHandleSwitchBoard,
} from "@/components/window-container";
import { BoardContainer } from "@/components/board-container";
import { AppModeContainer } from "@/components/app-mode-container";
import { InspectorContainer } from "@/components/inspector-container";

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
 * Main application shell:
 *   RustEngineContainer > WindowContainer > AppModeContainer > AppContent.
 *
 * RustEngineContainer owns entity state, entity event listeners, and all
 * Rust-bridge providers (Schema, EntityStore, EntityFocus, FieldUpdate,
 * UIState, Undo).
 *
 * WindowContainer owns the window scope, board lifecycle, AppShell, and
 * board switching.
 *
 * AppModeContainer owns the application interaction mode (normal, command,
 * search) and provides a mode-aware command scope moniker.
 *
 * AppContent (below) renders the board layout and InspectorContainer overlay.
 */
function App() {
  return (
    <RustEngineContainer>
      <WindowContainer>
        <AppModeContainer>
          <AppContent />
        </AppModeContainer>
      </WindowContainer>
    </RustEngineContainer>
  );
}

/**
 * Main content area that reads board state from container contexts and
 * renders the board or loading/placeholder states via BoardContainer.
 *
 * Owns:
 * - MaybeStoreScope for the active board path
 * - BoardContainer for conditional board rendering
 * - InspectorContainer as a sibling overlay
 */
function AppContent() {
  const board = useBoardData();
  const activeBoardPath = useActiveBoardPath();
  const openBoards = useOpenBoards();
  const handleSwitchBoard = useHandleSwitchBoard();
  const entitiesByType = useEntitiesByType();

  return (
    <MaybeStoreScope path={activeBoardPath}>
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

                <ModeIndicator />
              </div>
            </ViewCommandScope>
          </PerspectiveProvider>
        </ViewsProvider>
      </BoardContainer>
      <InspectorContainer />
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
