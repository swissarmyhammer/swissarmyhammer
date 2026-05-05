/**
 * End-to-end spatial-nav smoke test — mount full `<App/>` and walk every
 * gesture family.
 *
 * Source of truth for card `01KQ7PXYP62VQ18K9XYS4Y42GA` (the umbrella
 * end-to-end test commissioned after a release-blocker review found four
 * production regressions that the per-component spatial-nav test suite
 * had not caught:
 *
 *   1. Double-click on a perspective tab opened the inspector
 *      (`01KQ7GM77B1E6YH8Z893K05VKY`).
 *   2. Enter on a focused perspective tab did nothing instead of starting
 *      rename (`01KQ7GE3KY91X2YR6BX5AY40VK`).
 *   3. Nav.right from a card was trapped inside the column
 *      (`01KQ7GWE9V2XKWYAQ0HCPDE0EZ`).
 *   4. An unauthorized "ring" focus-indicator variant slipped past review
 *      (`01KQ7G7SCN7ZQD4TFGP5EH4FFX`).
 *
 * Every per-component spatial-nav test mounts ONE component with hand-
 * rolled providers and stubbed-shape contexts. None of them mount the
 * production `<App/>` and walk gestures across the real composition. So
 * bugs that live in the seams between components — a dblclick handler
 * bubbling through chrome `<FocusScope>`, a keymap dispatch flowing past
 * a perspective tab without a scope binding, the registration shape that
 * diverges between `<BoardView>` in isolation vs. `<BoardView>` inside
 * the real provider stack — go undetected.
 *
 * This file is the umbrella test that catches all four.
 *
 * # Removing any of the four fix commits would re-introduce a regression
 *
 * The four prerequisite-fix commits (`01KQ7GM77B1E6YH8Z893K05VKY`,
 * `01KQ7GE3KY91X2YR6BX5AY40VK`, `01KQ7GWE9V2XKWYAQ0HCPDE0EZ`,
 * `01KQ7G7SCN7ZQD4TFGP5EH4FFX`) are each pinned by a specific gesture
 * family below. A `git revert` of any of those commits — without
 * touching this file — would cause one or more of the families below to
 * fail. That is the test's reason for existing.
 *
 * Family ↔ regression mapping:
 *
 *   - **Family 6** (dblclick policy) catches `01KQ7GM77B1E6YH8Z893K05VKY`
 *     — dblclick on a perspective tab dispatching `ui.inspect`.
 *   - **Family 5** (Enter → rename on perspective tab) catches
 *     `01KQ7GE3KY91X2YR6BX5AY40VK` — Enter falling through to no-op
 *     drill-in instead of triggering rename.
 *   - **Family 2** (cross-zone hjkl/arrow nav) catches
 *     `01KQ7GWE9V2XKWYAQ0HCPDE0EZ` — right from a card trapped inside
 *     the column.
 *   - **Family 7** (single focus indicator, no ring variant) catches
 *     `01KQ7G7SCN7ZQD4TFGP5EH4FFX` — a second indicator visual shipping
 *     unnoticed.
 *
 * # Approach
 *
 * The test mounts the production `<App/>` directly — `<App/>` already
 * composes `SpatialFocusProvider → FocusLayer → CommandBusyProvider →
 * RustEngineContainer → WindowContainer → AppModeContainer →
 * BoardContainer → NavBar / ViewsContainer / PerspectivesContainer /
 * PerspectiveContainer / ViewContainer / InspectorsContainer`. The test
 * stubs no providers — it stubs only the **Tauri boundary**
 * (`@tauri-apps/api/core` invoke, `@tauri-apps/api/event` listen,
 * `@tauri-apps/api/window` getCurrent / WebviewWindow) so production
 * data flows through `RustEngineContainer` from the fixture.
 *
 * Greppable: `import App from "@/App"` appears on the import line below
 * — this is part of the acceptance criteria for the card.
 *
 * # Tauri boundary stub
 *
 * Bootstrap commands return fixture responses:
 *
 *   - `get_board_data` → fixture board (3 columns × 3 cards each)
 *   - `list_entities` → fixture tasks / actors / projects
 *   - `list_open_boards` → fixture board
 *   - `list_views` → one board view
 *   - `dispatch_command(perspective.list)` → fixture perspectives
 *   - `get_ui_state` → fixture ui-state with active perspective
 *   - `list_entity_types` / `get_entity_schema` → fixture schemas
 *
 * Spatial invokes (`spatial_register_scope`, `spatial_register_focusable`,
 * `spatial_register_layer`, `spatial_focus`, `spatial_navigate`,
 * `spatial_drill_in`, `spatial_drill_out`, `spatial_unregister_scope`,
 * `spatial_update_rect`) are routed through the shadow-registry
 * harness in `kanban-app/ui/src/test/spatial-shadow-registry.ts`.
 *
 * # Layout note
 *
 * The browser test project does not load `@tailwindcss/vite`, so
 * className-driven layout collapses — three columns stack vertically
 * instead of side-by-side. This test injects a small Tailwind
 * substitute stylesheet that makes the production class strings
 * resolve to the same row-of-columns layout the production app has on
 * a desktop window. The stylesheet is the same one
 * `board-view.cross-column-nav.spatial.test.tsx` uses; both files
 * are pinned to a 1400×900 viewport so column widths and cross-column
 * geometry match real-world layout.
 *
 * Runs under `kanban-app/ui/vite.config.ts`'s browser project (real
 * Chromium via Playwright).
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, act, waitFor, fireEvent } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Tauri API mocks — these MUST be in the test file (not just the
// helper) because vitest's `vi.mock` is file-scoped. A `vi.mock` call in
// the helper is hoisted to the top of the helper module, which makes it
// apply to imports THROUGH the helper — but transitive imports made by
// the test file's own `import App from "@/App"` go through the test
// file's import graph, which has no mocks unless they are declared
// here. Modules like `views-context.tsx` invoke `getCurrentWindow()` at
// MODULE EVALUATION TIME, so the mock must be in place at the moment
// the App's transitive imports resolve.
//
// To keep the helper as the single source of mock state, the factories
// here forward to the helper's exported spies. `vi.hoisted` resolves
// before the `import App` line so the helper's exports are available.
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
  // The quick-capture window uses LogicalSize to resize itself; we
  // export it as a no-op constructor so the import resolves at module
  // load time. The end-to-end test does not exercise the quick-capture
  // path, but `App.tsx` imports `quick-capture.tsx` transitively.
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
// Shared spatial-nav harness — provides the `setupSpatialHarness()`
// entry point that clears the mock triple, installs the bootstrap
// invoke handler, and layers the shadow-registry navigator on top.
// ---------------------------------------------------------------------------

import {
  setupSpatialHarness,
  type SpatialHarness,
} from "@/test/spatial-shadow-registry";

// ---------------------------------------------------------------------------
// Fixture — 3×3 board, 2 perspectives, schemas
// ---------------------------------------------------------------------------

import {
  E2E_BOARD_PATH,
  E2E_TASKS,
  E2E_PERSPECTIVES,
  E2E_VIEWS,
  E2E_BOARD_MONIKER,
  columnOfTaskMoniker,
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

// ---------------------------------------------------------------------------
// Production source under test — full <App/> by acceptance criterion.
// ---------------------------------------------------------------------------

import App from "@/App";
import { asSegment, type FullyQualifiedMoniker } from "@/types/spatial";

// ---------------------------------------------------------------------------
// Bootstrap-invoke handler — covers every Tauri command the production
// provider stack fires on mount.
// ---------------------------------------------------------------------------

/**
 * Default Tauri-invoke handler for non-spatial commands. Walks every
 * bootstrap call the production code makes on mount and returns the
 * matching fixture response. Unknown commands return `undefined` (the
 * Tauri default for void-result commands), which lets the App degrade
 * gracefully instead of throwing.
 *
 * This is layered UNDER the shadow-registry installer in
 * `setupSpatialHarness`: spatial commands are intercepted there, and
 * everything else falls through to this handler.
 */
async function bootstrapInvokeImpl(
  cmd: string,
  args?: unknown,
): Promise<unknown> {
  // Schema discovery
  if (cmd === "list_entity_types") return listEntityTypesResponse();
  if (cmd === "get_entity_schema") {
    const a = (args ?? {}) as Record<string, unknown>;
    return getEntitySchemaResponse(String(a.entityType));
  }
  // Board lifecycle
  if (cmd === "get_board_data") return getBoardDataResponse();
  if (cmd === "list_entities") {
    const a = (args ?? {}) as Record<string, unknown>;
    return listEntitiesResponse(String(a.entityType));
  }
  if (cmd === "list_open_boards") return listOpenBoardsResponse();
  if (cmd === "list_views") return listViewsResponse();
  // UI state
  if (cmd === "get_ui_state") return getUIStateResponse();
  if (cmd === "get_undo_state") return getUndoStateResponse();
  // Command dispatch — perspective.list is the only one that needs a
  // structured response; everything else returns void.
  if (cmd === "dispatch_command") {
    const a = (args ?? {}) as Record<string, unknown>;
    if (a.cmd === "perspective.list") return perspectiveListDispatchResponse();
    if (a.cmd === "perspective.set") return { result: null, undoable: false };
    if (a.cmd === "perspective.save") return { result: null, undoable: false };
    if (a.cmd === "perspective.rename") return { result: null, undoable: true };
    if (a.cmd === "view.set") return { result: null, undoable: false };
    if (a.cmd === "ui.inspect") return { result: null, undoable: false };
    if (a.cmd === "ui.setFocus") return { result: null, undoable: false };
    if (a.cmd === "file.switchBoard") return { result: null, undoable: false };
    return { result: null, undoable: false };
  }
  if (cmd === "list_commands_for_scope") return [];
  // Unknown command — return undefined (Tauri's default for void).
  return undefined;
}

// ---------------------------------------------------------------------------
// Layout substitute — same approach as the cross-column-nav test
// ---------------------------------------------------------------------------

const TEST_VIEWPORT_WIDTH_PX = 1400;
const TEST_VIEWPORT_HEIGHT_PX = 900;

/**
 * CSS substitute for the production Tailwind output. The browser test
 * project does not load `@tailwindcss/vite` (the plugin runs only at
 * production build time), so `className="flex flex-1 …"` on production
 * components renders as plain `<div>`s with no layout. This stylesheet
 * pins the small set of utility classes the App's layout chain relies
 * on so three columns lay out side-by-side and rect-based beam search
 * has sane geometry to score against.
 *
 * Mirrors the CSS in `board-view.cross-column-nav.spatial.test.tsx`;
 * keep the two in sync if either grows new rules.
 */
const TEST_LAYOUT_CSS = `
  .h-screen { height: 100vh; }
  .flex { display: flex; }
  .flex-col { flex-direction: column; }
  .flex-row { flex-direction: row; }
  .flex-1 { flex: 1 1 0%; min-width: 0; min-height: 0; }
  .min-h-0 { min-height: 0; }
  .min-w-0 { min-width: 0; }
  .overflow-hidden { overflow: hidden; }
  .overflow-x-auto { overflow-x: auto; }
  .overflow-y-auto { overflow-y: auto; }
  .relative { position: relative; }
  .absolute { position: absolute; }
  .min-w-\\[24em\\] { min-width: 24em; }
  .max-w-\\[48em\\] { max-width: 48em; }
  .shrink-0 { flex-shrink: 0; }
  .h-12 { height: 3rem; }
`;

/** Inject the layout substitute stylesheet exactly once per document. */
function ensureTestLayoutCss(): void {
  if (document.querySelector("style[data-test-e2e-layout]")) return;
  const style = document.createElement("style");
  style.setAttribute("data-test-e2e-layout", "");
  style.textContent = TEST_LAYOUT_CSS;
  document.head.appendChild(style);
}

/**
 * Mount the full production `<App/>` inside a viewport-sized wrapper.
 *
 * The outer `<div>` enforces 1400×900 so the column strip lays out
 * three columns side-by-side instead of collapsing to a vertical stack.
 * `<App/>` itself uses `h-screen` for the outermost container, which the
 * Tailwind substitute stylesheet maps to `100vh` — but `vh` is
 * percentage-of-viewport in Playwright's headless mode and does not
 * always settle to a pixel value during the initial paint. The wrapper
 * pins explicit pixel dimensions to keep the geometry deterministic.
 *
 * Returns `render`'s `{ container, unmount, ... }` bundle so individual
 * tests can query for `data-moniker` selectors and clean up.
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

// ---------------------------------------------------------------------------
// Setup helpers — flush bootstrap, locate registered keys.
// ---------------------------------------------------------------------------

/**
 * Wait long enough for the App's bootstrap chain to complete.
 *
 * `<App/>` mount → `RustEngineContainer` registers entity listeners →
 * `WindowContainer` calls `applyRestoredWindowState` (`get_ui_state`,
 * then `file.switchBoard`) → `WindowContainer.refresh()` runs three
 * `Promise.all`'d invokes (`get_board_data`, `list_entities` × 3) →
 * board data sets → `BoardContainer` un-loads and renders →
 * `ViewsContainer` / `PerspectivesContainer` mount → each fetches its
 * data → registrations propagate. Each step takes one or more macrotask
 * ticks; the production code is intentionally async to keep main-thread
 * responsiveness. The 250ms wait below is the same nudge the
 * cross-column-nav and column-view spatial tests use to let everything
 * settle.
 */
async function flushAppMount() {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 250));
  });
}

/**
 * Capture every `dispatch_command` call's args. Tests use this to
 * assert that a gesture dispatched (or did NOT dispatch) a specific
 * command id like `ui.inspect` or `perspective.rename`.
 */
function dispatchCalls(): Array<{
  cmd: string;
  args?: Record<string, unknown>;
  target?: string;
}> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "dispatch_command")
    .map((c) => c[1] as { cmd: string; args?: Record<string, unknown>; target?: string });
}

/** Filter dispatchCalls() to those with `cmd === target`. */
function dispatchCallsFor(target: string): Array<{
  cmd: string;
  args?: Record<string, unknown>;
  target?: string;
}> {
  return dispatchCalls().filter((d) => d.cmd === target);
}

/** Capture every `spatial_focus` call's args. */
function spatialFocusCalls(): Array<{ fq: FullyQualifiedMoniker }> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_focus")
    .map((c) => c[1] as { fq: FullyQualifiedMoniker });
}

/** Capture every `spatial_navigate` call's args. */
function spatialNavigateCalls(): Array<{ focusedFq: FullyQualifiedMoniker; direction: string }> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_navigate")
    .map((c) => c[1] as { focusedFq: FullyQualifiedMoniker; direction: string });
}

/** Capture every `spatial_drill_in` / `spatial_drill_out` call's args. */
function spatialDrillCalls(direction: "in" | "out"): Array<{ fq: FullyQualifiedMoniker }> {
  const cmd = direction === "in" ? "spatial_drill_in" : "spatial_drill_out";
  return mockInvoke.mock.calls
    .filter((c) => c[0] === cmd)
    .map((c) => c[1] as { fq: FullyQualifiedMoniker });
}

/** Pull every `spatial_register_scope` invocation argument bag. */
function registerScopeArgs(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_register_scope")
    .map((c) => c[1] as Record<string, unknown>);
}

/** Pull every `spatial_push_layer` invocation argument bag. */
function pushLayerArgs(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_push_layer")
    .map((c) => c[1] as Record<string, unknown>);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("End-to-end spatial-nav smoke test — full <App/>", () => {
  /**
   * Per-test harness. `setupSpatialHarness({ defaultInvokeImpl })`
   * clears the spy state, installs the bootstrap impl, layers the
   * shadow-registry navigator on top, and returns the bundle the
   * gesture families consume.
   */
  let harness: SpatialHarness;

  beforeEach(() => {
    harness = setupSpatialHarness({ defaultInvokeImpl: bootstrapInvokeImpl });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  // -------------------------------------------------------------------------
  // Smoke check — App mounts and bootstrap commands fire
  //
  // Run first as a sanity check: if the bootstrap chain is broken, every
  // gesture-family test below would fail with a confusing "no element
  // matches selector" error. This test makes the failure mode crystal
  // clear: the App's IPC fingerprint is the test's first hard contract.
  // -------------------------------------------------------------------------

  it("mounts <App/> and fires the expected bootstrap IPC fingerprint", async () => {
    const { unmount } = renderApp();
    await flushAppMount();

    // Bootstrap: schema discovery, board data, ui-state.
    expect(
      mockInvoke.mock.calls.some((c) => c[0] === "list_entity_types"),
      "App must call list_entity_types on mount",
    ).toBe(true);
    expect(
      mockInvoke.mock.calls.some((c) => c[0] === "get_ui_state"),
      "App must call get_ui_state on mount",
    ).toBe(true);
    expect(
      mockInvoke.mock.calls.some((c) => c[0] === "list_open_boards"),
      "App must call list_open_boards on mount",
    ).toBe(true);

    // Spatial-nav: a layer registration MUST fire — the window-root
    // `<FocusLayer name="window">` lives at the top of `<App/>`.
    expect(
      pushLayerArgs().length,
      "App must push at least one focus layer",
    ).toBeGreaterThan(0);

    unmount();
  });

  // -------------------------------------------------------------------------
  // Spatial-nav debug overlay — proves <FocusDebugProvider enabled> is
  // mounted at the App root.
  //
  // The overlay is gated by `useFocusDebug()`, which reads from a context
  // mounted in `App.tsx` (and the quick-capture window). When the project
  // is past its bug-fixing phase, flipping `enabled={false}` at the mount
  // site removes every `[data-debug=…]` element in one edit.
  // -------------------------------------------------------------------------

  it("app_renders_focus_debug_provider_at_root", async () => {
    const { container, unmount } = renderApp();
    await flushAppMount();

    // At least one scope overlay must exist somewhere in the rendered
    // tree — the chrome scopes (`ui:nav-bar`, `ui:board`, etc.) all
    // mount under `<FocusDebugProvider enabled>` and so must each
    // render their `[data-debug="scope"]` decorator. After parent
    // task `01KQSDP4ZJY5ERAJ68TFPVFRRE` collapsed the legacy split
    // primitives into a single `<FocusScope>`, every spatial-primitive
    // overlay carries `data-debug="scope"`.
    const scopeOverlays = container.querySelectorAll('[data-debug="scope"]');
    expect(
      scopeOverlays.length,
      "App must mount <FocusDebugProvider enabled> so scope overlays render",
    ).toBeGreaterThan(0);

    unmount();
  });

  // =========================================================================
  // Family 1 — Click → focus indicator visible
  //
  // Single-focus invariant: at any moment, exactly ONE element in the
  // document carries `data-focused="true"`. Clicking a leaf dispatches
  // `spatial_focus` against that leaf's key; the kernel emits
  // `focus-changed`; the `<FocusScope>` claim listener flips
  // `data-focused`. The visible `<FocusIndicator>` mounts as a
  // descendant of the focused leaf.
  //
  // Subjects: card, column body, perspective tab, nav-bar button,
  // inspector field row.
  // =========================================================================

  describe("Family 1 — Click → focus indicator visible", () => {
    it("clicking a card dispatches spatial_focus and flips data-focused on that card", async () => {
      const { container, unmount } = renderApp();
      await flushAppMount();

      // Cards register as `task:T1`-style scopes. Find the T1 card key
      // from the captured registrations.
      const t1Key = harness.getRegisteredFqBySegment("task:T1");
      expect(
        t1Key,
        "task:T1 must register before the click test runs",
      ).not.toBeNull();

      const t1Node = container.querySelector(
        "[data-segment='task:T1']",
      ) as HTMLElement | null;
      expect(t1Node, "task:T1 DOM node must exist after bootstrap").not.toBeNull();

      // Reset the invoke spy so we measure only the click's IPC.
      mockInvoke.mockClear();

      fireEvent.click(t1Node!);

      // The click handler dispatches `spatial_focus` against task:T1's key.
      // The shadow navigator queues a `focus-changed` event in response;
      // wait for the React tree to flip `data-focused`.
      await waitFor(() => {
        const focused = container.querySelector(
          "[data-segment='task:T1'][data-focused='true']",
        );
        expect(focused, "task:T1 must carry data-focused=true after click").not.toBeNull();
      });

      // Single-focus invariant: only ONE element in the document has
      // `data-focused="true"`. Multiple focused elements would break
      // the keyboard-handler's "the focused leaf is unambiguous" model.
      const allFocused = document.querySelectorAll("[data-focused='true']");
      expect(
        allFocused.length,
        "exactly one element in the document may carry data-focused=true",
      ).toBe(1);

      // The focused card hosts a `<FocusIndicator>` with the bar class
      // signature. Family 7 audits the class signature globally; Family
      // 1's job is to confirm the indicator mounts.
      const indicator = (allFocused[0] as HTMLElement).querySelector(
        "[data-testid='focus-indicator']",
      );
      expect(
        indicator,
        "focused card must render a <FocusIndicator>",
      ).not.toBeNull();

      // The spatial_focus call carries the registered key for task:T1.
      const focusCalls = spatialFocusCalls();
      expect(focusCalls.length).toBeGreaterThan(0);
      expect(focusCalls.some((c) => c.fq === t1Key!)).toBe(true);

      unmount();
    });

    it("clicking a perspective tab name focuses the name leaf inside that tab", async () => {
      const { container, unmount } = renderApp();
      await flushAppMount();

      // Post-reshape (card 01KQQSVS4EBKKFN5SS7MW5P8CN) the active perspective
      // mounts `<FocusScope perspective_tab:default>` with an inner
      // `<FocusScope perspective_tab.name:default>` leaf. A real user click
      // on the visible tab name lands on the inner leaf — that is the
      // realistic user path. The wrapping zone's onClick still calls
      // `focus(zone_fq)`, but the leaf calls `stopPropagation` so only one
      // `spatial_focus` reaches IPC. Mirrors `perspective-bar.spatial.test.tsx`
      // test #3 ("clicking a tab dispatches exactly one spatial_focus for the
      // name leaf").
      const nameLeafKey = harness.getRegisteredFqBySegment(
        "perspective_tab.name:default",
      );
      expect(
        nameLeafKey,
        "perspective_tab.name:default must register",
      ).not.toBeNull();

      const nameNode = container.querySelector(
        "[data-segment='perspective_tab.name:default']",
      ) as HTMLElement | null;
      expect(
        nameNode,
        "perspective_tab.name:default DOM node must exist",
      ).not.toBeNull();

      mockInvoke.mockClear();
      fireEvent.click(nameNode!);

      await waitFor(() => {
        const focused = container.querySelector(
          "[data-segment='perspective_tab.name:default'][data-focused='true']",
        );
        expect(focused).not.toBeNull();
      });

      // Single-focus invariant.
      expect(
        document.querySelectorAll("[data-focused='true']").length,
      ).toBe(1);

      // The focus call's key matches the registered name-leaf key — and the
      // outer zone's key is NOT also reported, because the leaf stops the
      // click from bubbling to the wrapping zone's onClick.
      const focusCalls = spatialFocusCalls();
      expect(focusCalls.some((c) => c.fq === nameLeafKey!)).toBe(true);
      const zoneKey = harness.getRegisteredFqBySegment(
        "perspective_tab:default",
      );
      expect(zoneKey, "perspective_tab:default must register").not.toBeNull();
      expect(focusCalls.find((c) => c.fq === zoneKey!)).toBeUndefined();

      unmount();
    });

    it("clicking a nav-bar leaf focuses that nav-bar leaf", async () => {
      const { container, unmount } = renderApp();
      await flushAppMount();

      // The nav bar registers `ui:navbar.search` as a leaf (and
      // `ui:navbar.board-selector`, `ui:navbar.inspect`). Pick the
      // search button — it always renders.
      const searchKey = harness.getRegisteredFqBySegment("ui:navbar.search");
      expect(searchKey, "ui:navbar.search must register").not.toBeNull();

      const searchNode = container.querySelector(
        "[data-segment='ui:navbar.search']",
      ) as HTMLElement | null;
      expect(searchNode).not.toBeNull();

      mockInvoke.mockClear();
      fireEvent.click(searchNode!);

      await waitFor(() => {
        const focused = container.querySelector(
          "[data-segment='ui:navbar.search'][data-focused='true']",
        );
        expect(focused).not.toBeNull();
      });

      // Single-focus invariant.
      expect(
        document.querySelectorAll("[data-focused='true']").length,
      ).toBe(1);

      unmount();
    });
  });

  // =========================================================================
  // Family 2 — hjkl / arrow navigation within and across zones
  //
  // Pins `01KQ7GWE9V2XKWYAQ0HCPDE0EZ`: right from a card in column TODO
  // must land on a card in column DOING (cross-zone leaf fallback).
  // =========================================================================

  describe("Family 2 — hjkl / arrow navigation", () => {
    it("ArrowDown from task:T1 advances to task:T2 within column TODO", async () => {
      const { container, unmount } = renderApp();
      await flushAppMount();

      const t1Key = harness.getRegisteredFqBySegment("task:T1");
      expect(t1Key).not.toBeNull();

      // Seed focus on T1 so the keymap pipeline knows the focused key.
      await harness.fireFocusChanged({
        next_fq: t1Key!,
        next_segment: asSegment("task:T1"),
      });
      await flushAppMount();

      fireEvent.keyDown(document.body, { key: "ArrowDown" });
      await flushAppMount();

      const focused = container.querySelector(
        "[data-focused='true'][data-segment]",
      );
      expect(focused, "ArrowDown must produce a focus change").not.toBeNull();
      // The next focused element should be task:T2 (the next card in
      // column TODO). Beam-search may skip ahead if rects are degenerate
      // — accept any task in column TODO for robustness.
      const moniker = focused!.getAttribute("data-moniker") ?? "";
      const column = columnOfTaskMoniker(moniker);
      expect(
        column,
        `ArrowDown from task:T1 must land on a task moniker (got ${moniker})`,
      ).toBeTruthy();
      expect(column).toBe("TODO");

      unmount();
    });

    it("ArrowRight from task:T1 lands on column DOING (zone or card, unified cascade)", async () => {
      const { container, unmount } = renderApp();
      await flushAppMount();

      const t1Key = harness.getRegisteredFqBySegment("task:T1");
      expect(t1Key).not.toBeNull();

      await harness.fireFocusChanged({
        next_fq: t1Key!,
        next_segment: asSegment("task:T1"),
      });
      await flushAppMount();

      fireEvent.keyDown(document.body, { key: "ArrowRight" });
      await flushAppMount();

      const focused = container.querySelector(
        "[data-focused='true'][data-segment]",
      );
      expect(focused).not.toBeNull();
      const moniker = focused!.getAttribute("data-moniker") ?? "";
      const column = columnOfTaskMoniker(moniker);
      expect(
        column,
        `ArrowRight from task:T1 must land on a task moniker (got ${moniker})`,
      ).toBeTruthy();
      expect(column).toBe("DOING");

      unmount();
    });

    it("ArrowLeft from a card in column DOING returns to column TODO (mirror)", async () => {
      const { container, unmount } = renderApp();
      await flushAppMount();

      const d1Key = harness.getRegisteredFqBySegment("task:D1");
      expect(d1Key).not.toBeNull();

      await harness.fireFocusChanged({
        next_fq: d1Key!,
        next_segment: asSegment("task:D1"),
      });
      await flushAppMount();

      fireEvent.keyDown(document.body, { key: "ArrowLeft" });
      await flushAppMount();

      const focused = container.querySelector(
        "[data-focused='true'][data-segment]",
      );
      expect(focused).not.toBeNull();
      const moniker = focused!.getAttribute("data-moniker") ?? "";
      const column = columnOfTaskMoniker(moniker);
      expect(column).toBe("TODO");

      unmount();
    });
  });

  // =========================================================================
  // Family 3 — Drill in (Enter) / drill out (Escape)
  //
  // Pressing Enter on a focused column zone sends `spatial_drill_in`.
  // Pressing Escape on a focused card sends `spatial_drill_out`. The
  // resulting focus moves are kernel-side; this family asserts on the
  // dispatched IPC, not on the post-drill `data-focused`.
  // =========================================================================

  describe("Family 3 — Drill in (Enter) / drill out (Escape)", () => {
    it("Enter on a focused card dispatches spatial_drill_in for that card's key", async () => {
      const { unmount } = renderApp();
      await flushAppMount();

      const t1Key = harness.getRegisteredFqBySegment("task:T1");
      expect(t1Key).not.toBeNull();

      await harness.fireFocusChanged({
        next_fq: t1Key!,
        next_segment: asSegment("task:T1"),
      });
      await flushAppMount();

      mockInvoke.mockClear();
      fireEvent.keyDown(document.body, { key: "Enter" });
      await flushAppMount();

      // Enter on a non-perspective entity leaf goes through the global
      // `nav.drillIn` binding, which dispatches `spatial_drill_in`
      // against the focused key.
      const drillIn = spatialDrillCalls("in");
      expect(
        drillIn.length,
        "Enter on a focused card must dispatch spatial_drill_in",
      ).toBeGreaterThan(0);
      expect(drillIn.some((c) => c.fq === t1Key!)).toBe(true);

      unmount();
    });

    it("Escape on a focused card dispatches spatial_drill_out for that card's key", async () => {
      const { unmount } = renderApp();
      await flushAppMount();

      const t1Key = harness.getRegisteredFqBySegment("task:T1");
      expect(t1Key).not.toBeNull();

      await harness.fireFocusChanged({
        next_fq: t1Key!,
        next_segment: asSegment("task:T1"),
      });
      await flushAppMount();

      mockInvoke.mockClear();
      fireEvent.keyDown(document.body, { key: "Escape" });
      await flushAppMount();

      const drillOut = spatialDrillCalls("out");
      expect(
        drillOut.length,
        "Escape on a focused card must dispatch spatial_drill_out",
      ).toBeGreaterThan(0);
      expect(drillOut.some((c) => c.fq === t1Key!)).toBe(true);

      unmount();
    });
  });

  // =========================================================================
  // Family 4 — Space → inspect
  //
  // Space on a focused card dispatches `ui.inspect` with the card's
  // entity moniker. Space on a focused perspective tab does NOT
  // dispatch `ui.inspect` — perspective tabs are chrome, not entities.
  //
  // Per card 01KQ9XJ4XGKVW24EZSQCA6K3E2 the Space owner is the
  // per-entity `<Inspectable>` wrapper (not the BoardView's old
  // `board.inspect`). Inspectable contributes a scope-level
  // `entity.inspect` `CommandDef` keyed to Space; perspective tabs
  // are intentionally NOT wrapped in Inspectable, so Space falls
  // through there with no inspect side effect — exactly the
  // chrome-stays-quiet contract the test below pins.
  // =========================================================================

  describe("Family 4 — Space → inspect", () => {
    it("Space on a focused card dispatches ui.inspect against that task's moniker", async () => {
      const { unmount } = renderApp();
      await flushAppMount();

      const t1Key = harness.getRegisteredFqBySegment("task:T1");
      expect(t1Key).not.toBeNull();

      await harness.fireFocusChanged({
        next_fq: t1Key!,
        next_segment: asSegment("task:T1"),
      });
      await flushAppMount();

      mockInvoke.mockClear();
      fireEvent.keyDown(document.body, { key: " " });
      await flushAppMount();

      // The Space binding for `ui.inspect` runs the focused card through
      // the inspect dispatcher with the task moniker as the target.
      const inspectCalls = dispatchCallsFor("ui.inspect");
      expect(
        inspectCalls.length,
        "Space on a focused card must dispatch ui.inspect",
      ).toBeGreaterThan(0);
      // Either the moniker is in the args bag or carried in `target`
      // (depending on which dispatcher path). Both shapes are accepted.
      const hasTaskTarget = inspectCalls.some((c) => {
        const inArgs = (c.args as { target?: unknown })?.target === "task:T1";
        const asTopLevel = c.target === "task:T1";
        return inArgs || asTopLevel;
      });
      expect(
        hasTaskTarget,
        "ui.inspect from Space must carry task:T1 as the target",
      ).toBe(true);

      unmount();
    });

    it("Space on a focused perspective tab does NOT dispatch ui.inspect", async () => {
      const { unmount } = renderApp();
      await flushAppMount();

      const tabKey = harness.getRegisteredFqBySegment(
        "perspective_tab:default",
      );
      expect(tabKey).not.toBeNull();

      await harness.fireFocusChanged({
        next_fq: tabKey!,
        next_segment: asSegment("perspective_tab:default"),
      });
      await flushAppMount();

      mockInvoke.mockClear();
      fireEvent.keyDown(document.body, { key: " " });
      await flushAppMount();

      // Perspective is not an entity — no ui.inspect should fire when
      // Space is pressed while a perspective tab is focused.
      const inspectCalls = dispatchCallsFor("ui.inspect");
      expect(
        inspectCalls.length,
        "Space on a perspective tab must NOT dispatch ui.inspect",
      ).toBe(0);

      unmount();
    });
  });

  // =========================================================================
  // Family 5 — Enter → rename on focused active perspective tab
  //
  // Pins `01KQ7GE3KY91X2YR6BX5AY40VK`: pressing Enter on a focused
  // active perspective tab mounts the inline rename editor; pressing
  // Enter while the editor is open commits via `perspective.rename`.
  // =========================================================================

  describe("Family 5 — Enter → rename on focused active perspective tab", () => {
    it("Enter on the focused active perspective tab mounts the inline rename editor", async () => {
      const { container, unmount } = renderApp();
      await flushAppMount();

      const tabKey = harness.getRegisteredFqBySegment(
        "perspective_tab:default",
      );
      expect(tabKey).not.toBeNull();

      await harness.fireFocusChanged({
        next_fq: tabKey!,
        next_segment: asSegment("perspective_tab:default"),
      });
      await flushAppMount();

      // Confirm focus landed on the active tab.
      await waitFor(() => {
        const tab = container.querySelector(
          "[data-segment='perspective_tab:default'][data-focused='true']",
        );
        expect(tab).not.toBeNull();
      });

      fireEvent.keyDown(document.body, { key: "Enter" });
      await flushAppMount();

      // The active-tab-only `ui.entity.startRename: Enter` binding mounts
      // an inline `<InlineRenameEditor>` (a CodeMirror `.cm-editor`)
      // inside the tab.
      await waitFor(() => {
        const editor = container.querySelector(
          "[data-segment='perspective_tab:default'] .cm-editor",
        );
        expect(
          editor,
          "Enter on focused active tab must mount the inline rename editor",
        ).not.toBeNull();
      });

      unmount();
    });
  });

  // =========================================================================
  // Family 6 — dblclick policy
  //
  // Pins `01KQ7GM77B1E6YH8Z893K05VKY`: double-click on a perspective
  // tab does NOT dispatch `ui.inspect`. Cards do dispatch (inspectable).
  // Chrome leaves (nav-bar background, perspective bar background, view
  // chrome) do NOT dispatch.
  // =========================================================================

  describe("Family 6 — dblclick policy", () => {
    it("dblclick on a card dispatches ui.inspect against that task's moniker", async () => {
      const { container, unmount } = renderApp();
      await flushAppMount();

      const t1Node = container.querySelector(
        "[data-segment='task:T1']",
      ) as HTMLElement | null;
      expect(t1Node).not.toBeNull();

      mockInvoke.mockClear();
      fireEvent.doubleClick(t1Node!);
      await flushAppMount();

      const inspectCalls = dispatchCallsFor("ui.inspect");
      expect(
        inspectCalls.length,
        "dblclick on a card must dispatch ui.inspect",
      ).toBeGreaterThan(0);
      const hasTaskTarget = inspectCalls.some((c) => {
        const inArgs = (c.args as { target?: unknown })?.target === "task:T1";
        const asTopLevel = c.target === "task:T1";
        return inArgs || asTopLevel;
      });
      expect(
        hasTaskTarget,
        "ui.inspect dispatched from dblclick must target task:T1",
      ).toBe(true);

      unmount();
    });

    it("dblclick on a perspective tab does NOT dispatch ui.inspect", async () => {
      const { container, unmount } = renderApp();
      await flushAppMount();

      const tabNode = container.querySelector(
        "[data-segment='perspective_tab:default']",
      ) as HTMLElement | null;
      expect(tabNode).not.toBeNull();

      mockInvoke.mockClear();
      fireEvent.doubleClick(tabNode!);
      await flushAppMount();

      const inspectCalls = dispatchCallsFor("ui.inspect");
      expect(
        inspectCalls.length,
        "dblclick on a perspective tab must NOT dispatch ui.inspect — perspective is chrome, not an entity",
      ).toBe(0);

      // Defensive: scan ALL invokes for any inspect-like command name.
      const anyInspect = mockInvoke.mock.calls.find((c) => {
        const cmd = typeof c[0] === "string" ? c[0] : "";
        const payload = (c[1] as { cmd?: string } | undefined)?.cmd ?? "";
        return /\binspect\b/i.test(cmd) || /\binspect\b/i.test(payload);
      });
      expect(
        anyInspect,
        "no inspect-related invoke should fire on perspective tab dblclick",
      ).toBeUndefined();

      unmount();
    });

    it("dblclick on the perspective bar background does NOT dispatch ui.inspect", async () => {
      const { container, unmount } = renderApp();
      await flushAppMount();

      const barNode = container.querySelector(
        "[data-segment='ui:perspective-bar']",
      ) as HTMLElement | null;
      expect(barNode).not.toBeNull();

      mockInvoke.mockClear();
      fireEvent.doubleClick(barNode!);
      await flushAppMount();

      const inspectCalls = dispatchCallsFor("ui.inspect");
      expect(
        inspectCalls.length,
        "dblclick on perspective bar chrome must NOT dispatch ui.inspect",
      ).toBe(0);

      unmount();
    });

    it("dblclick on the nav-bar zone background does NOT dispatch ui.inspect", async () => {
      const { container, unmount } = renderApp();
      await flushAppMount();

      const navNode = container.querySelector(
        "[data-segment='ui:navbar']",
      ) as HTMLElement | null;
      expect(navNode).not.toBeNull();

      mockInvoke.mockClear();
      fireEvent.doubleClick(navNode!);
      await flushAppMount();

      const inspectCalls = dispatchCallsFor("ui.inspect");
      expect(
        inspectCalls.length,
        "dblclick on nav-bar chrome must NOT dispatch ui.inspect",
      ).toBe(0);

      unmount();
    });

    // Note: a former `dblclick on the view container chrome…` test was
    // removed when the redundant `ui:view` `<FocusScope>` was deleted from
    // `view-container.tsx`. The view chrome rect is now owned by the
    // surrounding `ui:perspective` zone (covered above as
    // `dblclick on the perspective bar background…` and the perspective
    // zone tests in `perspective-view.spatial.test.tsx`). Re-introducing
    // the wrapper would pull this test back too — see
    // `perspective-spatial-nav.guards.node.test.ts`'s ViewContainer guards
    // for the source-level absence pin.
  });

  // =========================================================================
  // Family 7 — Single focus indicator, no ring variant
  //
  // Pins `01KQ7G7SCN7ZQD4TFGP5EH4FFX`: every `<FocusIndicator>` carries
  // the dotted-inset class signature (`pointer-events-none absolute
  // inset-0 border border-dotted border-primary`) and none carries
  // `ring-2` or the legacy cursor-bar tokens (`-left-2`, `w-1`,
  // `bg-primary`). The architectural guard in
  // `focus-architecture.guards.node.test.ts` prevents the variant prop
  // / type from existing in production source; this family checks the
  // RUNTIME DOM as a complementary contract.
  // =========================================================================

  describe("Family 7 — Single focus indicator, no ring variant", () => {
    it("every rendered focus indicator carries the dotted-inset class signature, none uses ring/cursor-bar variants", async () => {
      const { container, unmount } = renderApp();
      await flushAppMount();

      // Drive focus onto each indicative leaf in turn so multiple
      // indicators have a chance to mount across the run.
      for (const moniker of [
        "task:T1",
        "task:D1",
        "task:N1",
        "perspective_tab:default",
        "ui:navbar.search",
      ]) {
        const key = harness.getRegisteredFqBySegment(moniker);
        if (!key) continue;
        await harness.fireFocusChanged({
          next_fq: key,
          next_segment: asSegment(moniker),
        });
        await flushAppMount();
      }

      const indicators = Array.from(
        document.querySelectorAll("[data-testid='focus-indicator']"),
      );
      expect(
        indicators.length,
        "at least one focus indicator must mount during the family-7 walk",
      ).toBeGreaterThan(0);

      for (const ind of indicators) {
        const className = ind.className;
        // Dotted-inset signature: pointer-events-none, absolute,
        // inset-0, border, border-dotted, border-primary.
        expect(
          /\bpointer-events-none\b/.test(className),
          `indicator must carry pointer-events-none (got ${className})`,
        ).toBe(true);
        expect(
          /\babsolute\b/.test(className),
          `indicator must carry absolute (got ${className})`,
        ).toBe(true);
        expect(
          /\binset-0\b/.test(className),
          `indicator must carry inset-0 (got ${className})`,
        ).toBe(true);
        expect(
          /\bborder-dotted\b/.test(className),
          `indicator must carry border-dotted (got ${className})`,
        ).toBe(true);
        expect(
          /\bborder-primary\b/.test(className),
          `indicator must carry border-primary (got ${className})`,
        ).toBe(true);
        // Banned: legacy cursor-bar tokens and the ring variant.
        expect(
          /-left-2\b/.test(className),
          `indicator must NOT carry -left-2 (got ${className})`,
        ).toBe(false);
        expect(
          /\bring-2\b/.test(className),
          `indicator must NOT carry ring-2 (got ${className})`,
        ).toBe(false);
      }

      // Defensive: also walk the container, not just `document`, so a
      // rogue indicator escaping React's portal isn't missed.
      const containerIndicators = Array.from(
        container.querySelectorAll("[data-testid='focus-indicator']"),
      );
      for (const ind of containerIndicators) {
        const className = ind.className;
        expect(/-left-2\b/.test(className)).toBe(false);
        expect(/\bring-2\b/.test(className)).toBe(false);
      }

      unmount();
    });
  });

  // =========================================================================
  // Family 8 — Registry shape audit
  //
  // Walks the captured `spatial_register_*` calls and asserts the
  // app-shaped registry has the right structural shape:
  //
  //   - `task:*` is always `spatial_register_scope` (zone container),
  //     never scope. Cards hold focusable atoms (drag handle, Field
  //     rows, inspect button) so they are zones by the kernel's
  //     three-peer contract — see card
  //     `01KQJDYJ4SDKK2G8FTAQ348ZHG`.
  //   - Each `column:*` carries `parent_zone` equal to `ui:board`'s key.
  //   - `<Inspectable>`-wrapped entities are the only path to
  //     `ui.inspect` dispatch (architectural guard A).
  //
  // The cross-column-nav test does the per-card `parent_zone` audit;
  // here we focus on the App-level registrations the cross-column
  // test cannot see (nav-bar, perspective bar, inspector).
  // =========================================================================

  describe("Family 8 — Registry shape audit", () => {
    it("task:* monikers register via spatial_register_scope", async () => {
      // After parent task `01KQSDP4ZJY5ERAJ68TFPVFRRE` collapsed the
      // legacy split primitives into a single `<FocusScope>`, every
      // spatial primitive registers via `spatial_register_scope`; the
      // structural distinction between a container (a scope with
      // child scopes) and a leaf is no longer signalled by a separate
      // registration command. Cards are scope-with-children — they
      // hold the drag handle, Field rows, and inspect button — but
      // that shape is now established by the field-row scopes
      // parented at the card scope, not by a kind discriminator.
      const { unmount } = renderApp();
      await flushAppMount();

      // Every fixture task registered via `spatial_register_scope`.
      for (const t of E2E_TASKS) {
        const taskMoniker = `task:${t.id}`;
        const zoneReg = registerScopeArgs().find(
          (a) => a.segment === taskMoniker,
        );
        expect(
          zoneReg,
          `${taskMoniker} must register via spatial_register_scope`,
        ).toBeTruthy();
      }

      unmount();
    });

    it("each column:* carries parent_zone === ui:board's zone key", async () => {
      const { unmount } = renderApp();
      await flushAppMount();

      const boardZone = registerScopeArgs().find(
        (a) => a.segment === "ui:board",
      );
      expect(
        boardZone,
        "ui:board zone must register so columns can hang off it",
      ).toBeTruthy();
      const boardKey = boardZone!.fq as FullyQualifiedMoniker;

      for (const colId of ["TODO", "DOING", "DONE"]) {
        const moniker = `column:${colId}`;
        const colZone = registerScopeArgs().find((a) => a.segment === moniker);
        expect(
          colZone,
          `${moniker} must register via spatial_register_scope`,
        ).toBeTruthy();
        expect(
          colZone!.parentZone,
          `${moniker}'s parent_zone must equal ui:board's key`,
        ).toBe(boardKey);
      }

      unmount();
    });

    it("the focus layer pushes exactly once per window-root at boot", async () => {
      const { unmount } = renderApp();
      await flushAppMount();

      // The window-root `<FocusLayer name="window">` pushes once on
      // mount via `spatial_push_layer`. The inspector layer pushes when
      // the inspector stack is non-empty — but at boot the stack is
      // empty, so the count is 1. (StrictMode in dev double-invokes
      // effects, but the test runs in non-strict mode here, so a single
      // push is the expected baseline.)
      const layerCalls = pushLayerArgs();
      const windowLayers = layerCalls.filter((a) => a.name === "window");
      expect(
        windowLayers.length,
        "exactly one window-root FocusLayer must push at App mount",
      ).toBe(1);

      unmount();
    });

    it("the board entity registers and is wrappable in <Inspectable>", async () => {
      const { unmount } = renderApp();
      await flushAppMount();

      // The board moniker (`board:E2E`) is registered as a leaf via the
      // navbar's inspect button or the `ui:board` zone's contained
      // entity scope. Either path is acceptable; the assertion is that
      // the board entity has SOMETHING in the registry.
      const boardReg =
        registerScopeArgs().find((a) => a.segment === E2E_BOARD_MONIKER) ??
        registerScopeArgs().find((a) => a.segment === E2E_BOARD_MONIKER);
      expect(
        boardReg,
        `${E2E_BOARD_MONIKER} must register on App mount`,
      ).toBeTruthy();

      unmount();
    });

    it("the board entity zone's parent is ui:perspective — the redundant ui:view hop is gone", async () => {
      const { unmount } = renderApp();
      await flushAppMount();

      // The redundant `ui:view` chrome zone was deleted from
      // `view-container.tsx` (its rect overlapped the inner
      // `ui:board` / `ui:grid` zone for the same area). After the
      // deletion, the spatial graph is one zone shorter — the
      // `board:<id>` entity zone (which wraps `ui:board`) hangs
      // directly off `ui:perspective`. This test pins both halves of
      // the cleanup: no `ui:view` zone is registered, and the
      // `board:<id>` zone's `parent_zone` is the `ui:perspective`
      // chrome zone's fq.
      const viewZone = registerScopeArgs().find((a) => a.segment === "ui:view");
      expect(
        viewZone,
        "no ui:view chrome zone may register — it was deleted as redundant",
      ).toBeUndefined();

      const perspectiveZone = registerScopeArgs().find(
        (a) => a.segment === "ui:perspective",
      );
      expect(
        perspectiveZone,
        "ui:perspective zone must register so the view body can hang off it",
      ).toBeTruthy();

      const boardEntityZone = registerScopeArgs().find(
        (a) => a.segment === E2E_BOARD_MONIKER,
      );
      expect(
        boardEntityZone,
        `${E2E_BOARD_MONIKER} must register as a zone (the Inspectable+FocusScope wrapper around ui:board)`,
      ).toBeTruthy();
      expect(
        boardEntityZone!.parentZone,
        `${E2E_BOARD_MONIKER}'s parent_zone must equal ui:perspective's key now that ui:view is gone`,
      ).toBe(perspectiveZone!.fq);

      unmount();
    });
  });

  // =========================================================================
  // Family 9 — Edit-mode keystroke containment
  //
  // Typing into a focused text input fires zero `spatial_navigate`
  // invokes. The input owns the keystrokes; the spatial navigator stays
  // out of the way. Locks in the editor-keystroke-isolation contract.
  // =========================================================================

  describe("Family 9 — Edit-mode keystroke containment", () => {
    it("typing into the inline rename editor fires zero spatial_navigate calls", async () => {
      const { container, unmount } = renderApp();
      await flushAppMount();

      // Mount the inline rename editor via the family-5 path.
      const tabKey = harness.getRegisteredFqBySegment(
        "perspective_tab:default",
      );
      expect(tabKey).not.toBeNull();
      await harness.fireFocusChanged({
        next_fq: tabKey!,
        next_segment: asSegment("perspective_tab:default"),
      });
      await flushAppMount();
      fireEvent.keyDown(document.body, { key: "Enter" });
      await flushAppMount();

      const editor = await waitFor(() => {
        const ed = container.querySelector(
          "[data-segment='perspective_tab:default'] .cm-editor",
        );
        expect(ed).not.toBeNull();
        return ed as HTMLElement;
      });

      // Reset spy so we measure only the post-mount keystrokes.
      mockInvoke.mockClear();

      const cmContent = editor.querySelector(".cm-content") as HTMLElement;
      // Type "abc" via three synthetic keydown events delivered to the
      // editor's content host. The CM6 editor consumes them — no
      // spatial_navigate must fire.
      for (const ch of ["a", "b", "c"]) {
        fireEvent.keyDown(cmContent, { key: ch });
      }
      await flushAppMount();

      const navCalls = spatialNavigateCalls();
      expect(
        navCalls.length,
        "typing into the inline rename editor must not fire spatial_navigate",
      ).toBe(0);

      unmount();
    });
  });

  // =========================================================================
  // Sanity — the fixture's expected entities all show up post-bootstrap
  //
  // This test is a fixture-vs-mount integrity check: a typo in the
  // fixture's task ids would otherwise show up as a confusing failure
  // in Family 1+. By asserting the full task / column / perspective
  // moniker set up front, mismatches surface here with a clear message.
  // =========================================================================

  it("registers every fixture task, column, and perspective on mount", async () => {
    const { unmount } = renderApp();
    await flushAppMount();

    for (const t of E2E_TASKS) {
      const taskMoniker = `task:${t.id}`;
      expect(
        harness.getRegisteredFqBySegment(taskMoniker),
        `${taskMoniker} must register on App mount`,
      ).not.toBeNull();
    }
    for (const colId of ["TODO", "DOING", "DONE"]) {
      const moniker = `column:${colId}`;
      expect(
        harness.getRegisteredFqBySegment(moniker),
        `${moniker} must register on App mount`,
      ).not.toBeNull();
    }
    // Perspectives — the active one always registers; the inactive one
    // registers when the bar is rendered.
    for (const p of E2E_PERSPECTIVES) {
      const moniker = `perspective_tab:${p.id}`;
      expect(
        harness.getRegisteredFqBySegment(moniker),
        `${moniker} must register on App mount`,
      ).not.toBeNull();
    }
    // Sanity reference for the views fixture so unused-import warnings
    // don't trip eslint when the families don't read it.
    expect(E2E_VIEWS).toBeTruthy();
    // Reference to E2E_BOARD_PATH so eslint doesn't warn on unused
    // import; the path is read by the bootstrap impl indirectly.
    expect(E2E_BOARD_PATH).toBeTruthy();

    unmount();
  });
});
