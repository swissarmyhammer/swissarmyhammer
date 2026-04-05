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
import { InspectorContainer } from "@/components/inspector-container";
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
 * Container hierarchy (outermost to innermost):
 *   RustEngineContainer → WindowContainer → AppModeContainer →
 *   BoardContainer → ViewsContainer → ViewContainer →
 *   PerspectivesContainer → PerspectiveContainer
 *
 * InspectorContainer is a sibling overlay alongside the board layout.
 */
function App() {
  return (
    <RustEngineContainer>
      <WindowContainer>
        <AppModeContainer>
          <BoardContainer>
            <div className="h-screen bg-background text-foreground flex flex-col">
              <NavBar />
              <ViewsContainer>
                <PerspectivesContainer>
                  <PerspectiveContainer>
                    <div className="flex-1 min-w-0 overflow-hidden flex flex-col">
                      <ViewContainer />
                    </div>
                  </PerspectiveContainer>
                </PerspectivesContainer>
              </ViewsContainer>
              <ModeIndicator />
            </div>
          </BoardContainer>
          <InspectorContainer />
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
