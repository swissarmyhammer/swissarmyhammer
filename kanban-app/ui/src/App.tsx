// Field type registrations — must be imported before any Field renders
import "@/components/fields/registrations";

import { NavBar } from "@/components/nav-bar";
import { ModeIndicator } from "@/components/mode-indicator";
import { PerspectivesContainer } from "@/components/perspectives-container";
import { PerspectiveContainer } from "@/components/perspective-container";
import { QuickCapture } from "@/components/quick-capture";
import { RustEngineContainer } from "@/components/rust-engine-container";
import { WindowContainer } from "@/components/window-container";
import { BoardContainer } from "@/components/board-container";
import { AppModeContainer } from "@/components/app-mode-container";
import { InspectorsContainer } from "@/components/inspectors-container";
import { ViewsContainer } from "@/components/views-container";
import { ViewContainer } from "@/components/view-container";

/** Parse URL params once at module level. */
const URL_PARAMS = new URLSearchParams(window.location.search);

/** Detect if this window instance is the quick-capture popup. */
const IS_QUICK_CAPTURE = URL_PARAMS.get("window") === "quick-capture";

// Mark <html> so CSS can make the quick-capture window fully transparent.
if (IS_QUICK_CAPTURE) {
  document.documentElement.setAttribute("data-quick-capture", "");
}

/**
 * Main application shell — pure container composition with no state or logic.
 *
 * This is the authoritative container hierarchy. ARCHITECTURE.md references
 * this file rather than duplicating the tree. Each container comment below
 * explains WHY it sits at that level.
 */
function App() {
  return (
    // Outermost: provides entity state, schema, undo, and event bus
    // that every descendant needs. WindowContainer calls useRustEngine().
    <RustEngineContainer>
      {/* Needs useRustEngine() for refreshEntities; owns window identity,
          board switching, and AppShell keybindings. */}
      <WindowContainer>
        {/* Inside window: mode transitions (normal/command/search) dispatch
            commands that require window scope. Wraps NavBar and all content
            so keybindings and toolbar reflect the active mode. */}
        <AppModeContainer>
          {/* Conditional rendering (loading/empty/active board). Owns
              FileDropProvider and DragSessionProvider used by children. */}
          <BoardContainer>
            <div className="h-screen bg-background text-foreground flex flex-col">
              <NavBar />
              {/* Must wrap PerspectivesContainer because PerspectiveProvider
                  calls useViews(). Also owns LeftNav and view.switch:* commands. */}
              <ViewsContainer>
                {/* Owns the tab bar and PerspectiveProvider context that
                    PerspectiveContainer reads for the active perspective. */}
                <PerspectivesContainer>
                  {/* Applies filter/sort/group for the active perspective,
                      regardless of which view type renders below. */}
                  <PerspectiveContainer>
                    <div className="flex-1 min-w-0 overflow-hidden flex flex-col">
                      {/* Innermost content routing — picks BoardView or
                          GridView based on the active view definition. */}
                      <ViewContainer />
                    </div>
                  </PerspectiveContainer>
                </PerspectivesContainer>
              </ViewsContainer>
              <ModeIndicator />
            </div>
            {/* Inside BoardContainer so it has access to FileDropProvider —
                attachment fields in the inspector need drag-drop context.
                Renders as a fixed overlay above the board layout. */}
            <InspectorsContainer />
          </BoardContainer>
        </AppModeContainer>
      </WindowContainer>
    </RustEngineContainer>
  );
}

/**
 * Quick-capture window — minimal provider tree wrapping the capture form.
 *
 * Uses RustEngineContainer for schema and entity state instead of
 * duplicating individual providers. Sets body/html to transparent so
 * the borderless window shows only the styled card.
 */
function QuickCaptureApp() {
  // Mark document as transparent for the borderless capture window
  document.documentElement.style.background = "transparent";
  document.body.style.background = "transparent";

  return (
    <RustEngineContainer>
      <QuickCapture />
    </RustEngineContainer>
  );
}

export default IS_QUICK_CAPTURE ? QuickCaptureApp : App;
