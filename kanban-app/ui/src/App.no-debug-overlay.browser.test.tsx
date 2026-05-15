/**
 * Regression test — full `<App/>` mount must NOT render any spatial-nav
 * debug-overlay decorators in production.
 *
 * Source of truth for kanban task `01KQYWNHX9NS63JAFRDQ6E0DCM`
 * ("Disable focus-debug overlays in production"). Both `App.tsx` mount
 * sites — the main app shell and the quick-capture window — must pass
 * `<FocusDebugProvider enabled={false}>` so the dashed border, colored
 * corner handle, and `(x,y)` tooltip the developer overlay paints on
 * every `<FocusLayer>` / `<FocusScope>` host stay off in shipped builds.
 *
 * The Jump-To overlay supersedes the need for these debug visualizations
 * (they were a developer aid for diagnosing rect-staleness during the
 * spatial-nav build-out). Flipping the prop is the one-line edit; this
 * test pins the new default so a future re-flip back to `enabled` is
 * caught immediately, before it reaches users.
 *
 * # What this test asserts
 *
 * After the App mounts and bootstrap settles, NO element anywhere in the
 * DOM carries `data-debug` (the stable selector
 * `<FocusDebugOverlay>` / `<FocusLayerOverlay>` paint on their outer
 * spans — see `kanban-app/ui/src/components/focus-debug-overlay.tsx`).
 * Zero matches proves both:
 *
 *   1. The provider is mounted with `enabled={false}` so consumers
 *      (`<FocusLayer>`, `<FocusScope>`) skip rendering the overlay.
 *   2. No production code path mounts a second `FocusDebugProvider`
 *      with `enabled` somewhere deeper in the tree that would re-enable
 *      overlays for a subset of the chrome.
 *
 * # Approach
 *
 * Mounts the production `<App/>` directly via the same Tauri-boundary
 * stub the spatial-nav-end-to-end test uses — `RustEngineContainer`,
 * `WindowContainer`, `BoardContainer`, `ViewsContainer`,
 * `PerspectivesContainer`, `PerspectiveContainer`, `ViewContainer`,
 * `InspectorsContainer` all run their real bootstrap. We do NOT stub
 * any provider — only the Tauri IPC surface — so a regression in the
 * `App.tsx` provider tree (a stray `<FocusDebugProvider enabled>`
 * deeper than the App root, or a flip back to `enabled` at the root)
 * is caught honestly.
 *
 * Pairs with the test-local debug-overlay coverage in
 * `kanban-app/ui/src/components/focus-debug-overlay.browser.test.tsx`,
 * which mounts its own `<FocusDebugProvider enabled>` to verify the
 * overlay still works when a developer flips it on locally.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, act } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Tauri API mocks — these MUST be in the test file (not just the helper)
// because vitest's `vi.mock` is file-scoped. Modules like
// `views-context.tsx` invoke `getCurrentWindow()` at MODULE EVALUATION
// TIME, so the mock must be in place at the moment the App's transitive
// imports resolve. To keep the helper as the single source of mock
// state, the factories here forward to the helper's exported spies.
// `vi.hoisted` resolves before the `import App` line so the helper's
// exports are available.
//
// Mirrors the mock setup in
// `spatial-nav-end-to-end.spatial.test.tsx`; keep them in sync if either
// file's mock surface grows.
// ---------------------------------------------------------------------------

const { mockInvoke, mockListen, listeners } = await vi.hoisted(async () => {
  const helper = await import("@/test/spatial-shadow-registry");
  return {
    mockInvoke: helper.mockInvoke,
    mockListen: helper.mockListen,
    listeners: helper.listeners,
  };
});

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...a: unknown[]) => mockInvoke(...(a as [string, unknown?])),
}));

vi.mock("@tauri-apps/api/event", () => ({
  emit: vi.fn(() => Promise.resolve()),
  listen: (...a: Parameters<typeof mockListen>) => mockListen(...a),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    label: "main",
    listen: vi.fn(() => Promise.resolve(() => {})),
    setSize: vi.fn(() => Promise.resolve()),
    setPosition: vi.fn(() => Promise.resolve()),
    setFocus: vi.fn(() => Promise.resolve()),
    show: vi.fn(() => Promise.resolve()),
    hide: vi.fn(() => Promise.resolve()),
  }),
  WebviewWindow: class {
    label: string;
    constructor(label: string) {
      this.label = label;
    }
    listen() {
      return Promise.resolve(() => {});
    }
    emit() {
      return Promise.resolve();
    }
    close() {
      return Promise.resolve();
    }
    setSize() {
      return Promise.resolve();
    }
    show() {
      return Promise.resolve();
    }
    hide() {
      return Promise.resolve();
    }
  },
  LogicalSize: class {
    constructor(
      public width: number,
      public height: number,
    ) {}
  },
  LogicalPosition: class {
    constructor(
      public x: number,
      public y: number,
    ) {}
  },
  PhysicalSize: class {
    constructor(
      public width: number,
      public height: number,
    ) {}
  },
  PhysicalPosition: class {
    constructor(
      public x: number,
      public y: number,
    ) {}
  },
  currentMonitor: vi.fn(() =>
    Promise.resolve({
      name: "test-monitor",
      size: { width: 1920, height: 1080 },
      position: { x: 0, y: 0 },
      scaleFactor: 1,
    }),
  ),
  availableMonitors: vi.fn(() => Promise.resolve([])),
}));

vi.mock("@tauri-apps/plugin-log", () => ({
  error: vi.fn(),
  warn: vi.fn(),
  info: vi.fn(),
  debug: vi.fn(),
  trace: vi.fn(),
  attachConsole: vi.fn(() => Promise.resolve()),
}));

// Reference `listeners` so eslint doesn't warn about an unused binding;
// the real consumer is the helper-internal shadow-navigator install path.
void listeners;

// ---------------------------------------------------------------------------
// Imports — after mocks
// ---------------------------------------------------------------------------

import { setupSpatialHarness } from "@/test/spatial-shadow-registry";
import {
  getBoardDataResponse,
  listEntitiesResponse,
  listOpenBoardsResponse,
  listViewsResponse,
  perspectiveListDispatchResponse,
  getUIStateResponse,
  getUndoStateResponse,
  listEntityTypesResponse,
  getEntitySchemaResponse,
} from "@/test/fixtures/end-to-end-board";

import App from "@/App";

// ---------------------------------------------------------------------------
// Bootstrap-invoke handler — covers every Tauri command the production
// provider stack fires on mount. Mirrors the handler in
// `spatial-nav-end-to-end.spatial.test.tsx`.
// ---------------------------------------------------------------------------

/**
 * Default Tauri-invoke handler for non-spatial commands.
 *
 * Walks every bootstrap call the production provider stack makes on
 * mount and returns the matching fixture response. Unknown commands
 * return `undefined` (the Tauri default for void-result commands), which
 * lets the App degrade gracefully instead of throwing.
 *
 * Layered UNDER the shadow-registry installer in `setupSpatialHarness`:
 * spatial commands are intercepted there, and everything else falls
 * through to this handler.
 */
async function bootstrapInvokeImpl(
  cmd: string,
  args?: unknown,
): Promise<unknown> {
  if (cmd === "list_entity_types") return listEntityTypesResponse();
  if (cmd === "get_entity_schema") {
    const a = (args ?? {}) as Record<string, unknown>;
    return getEntitySchemaResponse(String(a.entityType));
  }
  if (cmd === "get_board_data") return getBoardDataResponse();
  if (cmd === "list_entities") {
    const a = (args ?? {}) as Record<string, unknown>;
    return listEntitiesResponse(String(a.entityType));
  }
  if (cmd === "list_open_boards") return listOpenBoardsResponse();
  if (cmd === "list_views") return listViewsResponse();
  if (cmd === "get_ui_state") return getUIStateResponse();
  if (cmd === "get_undo_state") return getUndoStateResponse();
  if (cmd === "dispatch_command") {
    const a = (args ?? {}) as Record<string, unknown>;
    if (a.cmd === "perspective.list") return perspectiveListDispatchResponse();
    return { result: null, undoable: false };
  }
  if (cmd === "list_commands_for_scope") return [];
  return undefined;
}

// ---------------------------------------------------------------------------
// Layout substitute — same approach as the spatial-nav-end-to-end test
// ---------------------------------------------------------------------------

const TEST_VIEWPORT_WIDTH_PX = 1400;
const TEST_VIEWPORT_HEIGHT_PX = 900;

/**
 * CSS substitute for the production Tailwind output. The browser test
 * project does not load `@tailwindcss/vite` (the plugin runs only at
 * production build time), so utility classes resolve to no styles.
 * This stylesheet pins the small set of classes the App's layout chain
 * relies on so `<App/>` lays out plausibly during the test.
 *
 * The overlay-presence check does not actually need geometry, but the
 * App's bootstrap chain is more deterministic when its container divs
 * have non-zero dimensions — the rAF rect probe in
 * `<FocusDebugOverlay>` (if it ran) would otherwise read all-zero
 * rects and could mask a regression where overlays mount but produce
 * empty boxes.
 */
const TEST_LAYOUT_CSS = `
  .h-screen { height: 100vh; }
  .flex { display: flex; }
  .flex-col { flex-direction: column; }
  .flex-1 { flex: 1 1 0%; min-width: 0; min-height: 0; }
  .min-w-0 { min-width: 0; }
  .overflow-hidden { overflow: hidden; }
`;

/** Inject the layout substitute stylesheet exactly once per document. */
function ensureTestLayoutCss(): void {
  if (document.querySelector("style[data-test-no-debug-overlay-layout]"))
    return;
  const style = document.createElement("style");
  style.setAttribute("data-test-no-debug-overlay-layout", "");
  style.textContent = TEST_LAYOUT_CSS;
  document.head.appendChild(style);
}

/**
 * Mount the full production `<App/>` inside a viewport-sized wrapper.
 * Mirrors `renderApp()` in `spatial-nav-end-to-end.spatial.test.tsx`.
 */
function renderApp() {
  ensureTestLayoutCss();
  return render(
    <div
      style={{
        width: `${TEST_VIEWPORT_WIDTH_PX}px`,
        height: `${TEST_VIEWPORT_HEIGHT_PX}px`,
        display: "flex",
        flexDirection: "column",
      }}
    >
      <App />
    </div>,
  );
}

/**
 * Wait long enough for the App's bootstrap chain to complete.
 *
 * `<App/>` mount → `RustEngineContainer` registers entity listeners →
 * `WindowContainer` calls `applyRestoredWindowState` → board data sets
 * → `BoardContainer` un-loads and renders → `ViewsContainer` /
 * `PerspectivesContainer` mount → registrations propagate. Each step
 * takes one or more macrotask ticks. The 250 ms wait below matches the
 * cross-column-nav and end-to-end spatial tests.
 */
async function flushAppMount() {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 250));
  });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("<App/> — focus-debug overlays disabled in production", () => {
  beforeEach(() => {
    setupSpatialHarness({ defaultInvokeImpl: bootstrapInvokeImpl });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("renders no [data-debug] overlays anywhere in the App tree", async () => {
    const { container, unmount } = renderApp();
    await flushAppMount();

    // The full overlay surface — `<FocusDebugOverlay>` for zone/scope
    // and `<FocusLayerOverlay>` for layers — paints `data-debug={kind}`
    // on its outer span. Zero matches proves the
    // `<FocusDebugProvider enabled={false}>` mount in `App.tsx` reaches
    // every consumer (`<FocusLayer>`, `<FocusScope>`) and that no
    // production code path mounts a second provider with `enabled`
    // deeper in the tree.
    const overlays = container.querySelectorAll("[data-debug]");
    expect(
      overlays.length,
      "App must NOT render any focus-debug overlays in production",
    ).toBe(0);

    // Sanity: confirm the App actually mounted — a regression where the
    // provider tree threw before reaching `<FocusLayer>` would also
    // produce zero overlays, which would falsely look like a pass.
    // The window-root layer registers via `spatial_push_layer`, so its
    // presence pins "App really did mount and register at least one
    // layer." The overlay assertion above then proves the layer
    // registered without a paired debug overlay.
    const layerPushes = mockInvoke.mock.calls.filter(
      (c) => c[0] === "spatial_push_layer",
    );
    expect(
      layerPushes.length,
      "App must push at least one focus layer (sanity check that App mounted)",
    ).toBeGreaterThan(0);

    unmount();
  });
});
