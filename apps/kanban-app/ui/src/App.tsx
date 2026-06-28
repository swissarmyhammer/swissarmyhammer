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
import { AiPanelContainer } from "@/components/ai-panel-container";
import { CommandBusyProvider } from "@/lib/command-scope";
import { FocusDebugProvider } from "@/lib/focus-debug-context";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { FocusLayer } from "@/components/focus-layer";
import { asSegment, fqRoot } from "@/types/spatial";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { DiagErrorBoundary } from "@/components/diag-error-boundary";

/**
 * Identity-stable `LayerName` for the window-root spatial-nav layer.
 *
 * Pulled to module scope so re-renders never mint a fresh value — the
 * `<FocusLayer>` push effect depends on `name`, and a fresh literal in JSX
 * would force an unnecessary tear-down / re-push of the window root layer.
 */
const WINDOW_LAYER_NAME = asSegment("window");

/**
 * Identity-stable parent FQM for the window-root `<FocusLayer>`, rooted at
 * THIS window's unique label (`/<label>`).
 *
 * Why a UNIQUE root per window: the kernel's `SpatialRegistry.layers` map is
 * keyed by FQM. If every window rooted its layer at the literal `/window`,
 * two windows on the same board would collide on that key — the second push
 * clobbers the first, and any window resolution that leans on the layer's
 * `window_label` side field then targets the wrong window (the cross-window
 * focus/nav contamination bug). Rooting at the window label makes the layer
 * FQM `/<label>/window`, the board a nested segment below it, and every
 * descendant scope window-unique by construction.
 *
 * Why MODULE SCOPE (computed once, never inline in JSX): the prior rooting
 * attempt computed `fqRoot(getCurrentWindow().label)` inline in the
 * `parentLayerFq` prop. That minted a fresh value on every render; the
 * `<FocusLayer>` `fq` memo (and therefore the push fq) churned, tearing the
 * layer down and re-pushing it under a shifting identity while focus commits
 * raced the gap — every commit dropped with "focus snapshot names an
 * unregistered layer". Computing the root ONCE at module load (the window
 * label is constant for the webview's lifetime) keeps the parent — and thus
 * the composed layer fq, the registry key, and the snapshot's `layer_fq` —
 * identity-stable and mutually equal. Mirrors `WINDOW_LAYER_NAME` above and
 * `window-container.tsx`'s module-scope `WINDOW_LABEL`.
 */
const WINDOW_ROOT_FQ = fqRoot(asSegment(getCurrentWindow().label));

/** Parse URL params once at module level. */
const URL_PARAMS = new URLSearchParams(window.location.search);

/** Detect if this window instance is the quick-capture popup. */
const IS_QUICK_CAPTURE = URL_PARAMS.get("window") === "quick-capture";

// Mark <html> so CSS can make the quick-capture window fully transparent.
if (IS_QUICK_CAPTURE) {
  document.documentElement.setAttribute("data-quick-capture", "");
}

/**
 * Main application shell — pure container composition, no state or logic.
 *
 * Authoritative container hierarchy (ARCHITECTURE.md references this file).
 * Ordering constraints, outermost → innermost:
 *
 * - `CommandBusyProvider` — owns the in-flight counter shared by
 *   `useDispatchCommand` (writer inside `WindowContainer` descendants) and
 *   `refreshEntities` (writer inside `RustEngineContainer`). Must sit above
 *   both writers; otherwise the nav-bar progress bar never lights up for
 *   refetches.
 * - `RustEngineContainer` — entity state, schema, undo, event bus. Owns the
 *   busy-setter writer for `refreshEntities` refetches, so it must sit
 *   inside `CommandBusyProvider`.
 * - `WindowContainer` — window identity, board switching, `AppShell`
 *   keybindings. Calls `useRustEngine()`.
 * - `AppModeContainer` — mode transitions (normal/command/search). Must wrap
 *   `NavBar` and content so keybindings and toolbar reflect active mode.
 * - `BoardContainer` — conditional render (loading/empty/active). Owns
 *   `FileDropProvider` and `DragSessionProvider`.
 * - `ViewsContainer` → `PerspectivesContainer` → `PerspectiveContainer` —
 *   tab bar, active perspective, filter/sort/group application.
 * - `ViewContainer` — innermost routing: BoardView or GridView.
 * - `InspectorsContainer` — sibling of the board layout, inside
 *   `BoardContainer` so it can consume `FileDropProvider` for attachments.
 */
function App() {
  return (
    <DiagErrorBoundary>
      <FocusDebugProvider enabled={false}>
        <SpatialFocusProvider>
          <FocusLayer name={WINDOW_LAYER_NAME} parentLayerFq={WINDOW_ROOT_FQ}>
            <CommandBusyProvider>
              <RustEngineContainer>
                <WindowContainer>
                  <AppModeContainer>
                    <BoardContainer>
                      <div className="h-screen bg-background text-foreground flex flex-col overflow-hidden">
                        <NavBar />
                        {/* The view area and the AI panel sit side by side on
                            the main (window) layer. `AiPanelContainer` is the
                            right dock — a sibling of `ViewsContainer`, inside
                            `WindowContainer`, and outside the inspector stack
                            (`InspectorsContainer` below). */}
                        <div className="flex-1 min-h-0 flex overflow-hidden">
                          <ViewsContainer>
                            <PerspectivesContainer>
                              <PerspectiveContainer>
                                <div className="flex-1 min-w-0 overflow-hidden flex flex-col">
                                  <ViewContainer />
                                </div>
                              </PerspectiveContainer>
                            </PerspectivesContainer>
                          </ViewsContainer>
                          <AiPanelContainer />
                        </div>
                        <ModeIndicator />
                      </div>
                      <InspectorsContainer />
                    </BoardContainer>
                  </AppModeContainer>
                </WindowContainer>
              </RustEngineContainer>
            </CommandBusyProvider>
          </FocusLayer>
        </SpatialFocusProvider>
      </FocusDebugProvider>
    </DiagErrorBoundary>
  );
}

/**
 * Quick-capture window — minimal provider tree wrapping the capture form.
 *
 * Uses RustEngineContainer for schema and entity state instead of
 * duplicating individual providers. Sets body/html to transparent so
 * the borderless window shows only the styled card.
 *
 * Wrapped in `<SpatialFocusProvider>` + `<FocusLayer name="window"
 * parentLayerFq={WINDOW_ROOT_FQ}>` to match the main `App` shell — every
 * Tauri webview's React root must mount its own window-root layer, rooted
 * at this window's unique label (`/<label>/window`) so its FQMs never
 * collide with another window's. The capture form does
 * not currently use spatial primitives directly, but the wrapping is
 * harmless when no descendants register and future-proofs the window
 * for spatial-aware children (e.g. arrow-key navigation between fields).
 */
function QuickCaptureApp() {
  // Mark document as transparent for the borderless capture window
  document.documentElement.style.background = "transparent";
  document.body.style.background = "transparent";

  return (
    <FocusDebugProvider enabled={false}>
      <SpatialFocusProvider>
        <FocusLayer name={WINDOW_LAYER_NAME} parentLayerFq={WINDOW_ROOT_FQ}>
          <RustEngineContainer>
            <QuickCapture />
          </RustEngineContainer>
        </FocusLayer>
      </SpatialFocusProvider>
    </FocusDebugProvider>
  );
}

export default IS_QUICK_CAPTURE ? QuickCaptureApp : App;
