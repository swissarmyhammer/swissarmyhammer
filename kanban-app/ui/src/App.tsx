// Field type registrations — must be imported before any Field renders
import "@/components/fields/registrations";

import { useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { FieldUpdateProvider } from "@/lib/field-update-context";
import { UIStateProvider } from "@/lib/ui-state-context";
import { NavBar } from "@/components/nav-bar";
import { ModeIndicator } from "@/components/mode-indicator";
import { PerspectivesContainer } from "@/components/perspectives-container";
import { PerspectiveContainer } from "@/components/perspective-container";
import { QuickCapture } from "@/components/quick-capture";
import { StoreContainer } from "@/components/store-container";
import { RustEngineContainer } from "@/components/rust-engine-container";
import {
  WindowContainer,
  useActiveBoardPath,
} from "@/components/window-container";
import { BoardContainer } from "@/components/board-container";
import { AppModeContainer } from "@/components/app-mode-container";
import { InspectorContainer } from "@/components/inspector-container";
import { ViewsContainer } from "@/components/views-container";
import { ViewContainer } from "@/components/view-container";

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
  const activeBoardPath = useActiveBoardPath();

  return (
    <MaybeStoreScope path={activeBoardPath}>
      <BoardContainer>
        <div className="h-screen bg-background text-foreground flex flex-col">
          <NavBar />
          <PerspectivesContainer>
            <PerspectiveContainer>
              <div className="flex-1 flex min-h-0">
                <ViewsContainer>
                  <div className="flex-1 min-w-0 overflow-hidden flex flex-col">
                    <ViewContainer />
                  </div>
                </ViewsContainer>
              </div>
            </PerspectiveContainer>
          </PerspectivesContainer>
          <ModeIndicator />
        </div>
      </BoardContainer>
      <InspectorContainer />
    </MaybeStoreScope>
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
