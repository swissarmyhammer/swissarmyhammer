/**
 * Spatial-nav contract tests for the perspective tab bar.
 *
 * Asserts the universal top-edge nav contract for `<PerspectiveTabBar />`:
 *
 * 1. `k` (vim nav.up) from a top-row card on the board moves focus to
 *    the active perspective tab.
 * 2. `k` (vim nav.up) from a top-row cell in the grid moves focus to
 *    the active perspective tab.
 * 3. `h`/`l` (vim nav.left/nav.right) moves between perspective tabs
 *    within the same bar.
 * 4. `j` (vim nav.down) from an active perspective tab moves into the
 *    active view (card on the board, cell in the grid).
 *
 * The suite must fail against HEAD until `ScopedPerspectiveTab` wraps
 * its children in a `FocusScope` with a `perspective:` moniker — today
 * the tab only has a `CommandScopeProvider`, so no spatial entry is
 * registered, and beam-test searches from the top row never land on a
 * tab.
 *
 * ## Harness
 *
 * Uses the in-process `SpatialStateShim` + shared `FixtureShell` from
 * `spatial-fixture-shell.tsx`, stacked by
 * `spatial-perspective-fixture.tsx`. Context providers for
 * perspective/views/schema/ui-state/board-data/context-menu are mocked
 * at the module level — same pattern as
 * `perspective-tab-bar.test.tsx`.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { userEvent } from "vitest/browser";
import { render } from "vitest-browser-react";

// -------------------------------------------------------------------------
// Tauri mocks — route spatial_* calls into the shim, other invokes → null.
// -------------------------------------------------------------------------

// Preserve the real `@tauri-apps/api/core` exports (SERIALIZE_TO_IPC_FN,
// Resource, Channel, PluginListener, etc.) so transitively-imported
// sibling modules (`window.js`, `webviewWindow.js`, `dpi.js`) can resolve
// their re-exports. Only override `invoke` to route spatial_* calls into
// the shim.
vi.mock("@tauri-apps/api/core", async () => {
  const actual = await vi.importActual<typeof import("@tauri-apps/api/core")>(
    "@tauri-apps/api/core",
  );
  const { tauriCoreMock } = await import("./setup-spatial-shim");
  const { invoke } = tauriCoreMock();
  return { ...actual, invoke };
});
vi.mock("@tauri-apps/api/event", async () => {
  const actual = await vi.importActual<typeof import("@tauri-apps/api/event")>(
    "@tauri-apps/api/event",
  );
  const { tauriEventMock } = await import("./setup-spatial-shim");
  return { ...actual, ...tauriEventMock() };
});
vi.mock("@tauri-apps/api/window", async () => {
  const actual = await vi.importActual<typeof import("@tauri-apps/api/window")>(
    "@tauri-apps/api/window",
  );
  const { tauriWindowMock } = await import("./setup-spatial-shim");
  return { ...actual, ...tauriWindowMock() };
});
vi.mock("@tauri-apps/api/webviewWindow", async () => {
  const actual = await vi.importActual<
    typeof import("@tauri-apps/api/webviewWindow")
  >("@tauri-apps/api/webviewWindow");
  const { tauriWebviewWindowMock } = await import("./setup-spatial-shim");
  return { ...actual, ...tauriWebviewWindowMock() };
});
vi.mock("@tauri-apps/plugin-log", async () => {
  const { tauriPluginLogMock } = await import("./setup-spatial-shim");
  return tauriPluginLogMock();
});

// -------------------------------------------------------------------------
// Context mocks — same shape as perspective-tab-bar.test.tsx.
//
// Kept identical (minus perspective-bar-specific behavior like rename
// dispatch) so changes to the production providers surface here via the
// real component rather than diverging mock implementations.
// -------------------------------------------------------------------------

/** Perspective shape the mock `usePerspectives()` returns. */
type MockPerspective = {
  id: string;
  name: string;
  view: string;
  filter?: string;
  group?: string;
};

const fixturePerspectives: MockPerspective[] = [
  { id: "default", name: "Default", view: "board" },
  { id: "archive", name: "Archive", view: "board" },
];

let mockViewKind: "board" | "grid" = "board";

vi.mock("@/lib/perspective-context", () => ({
  usePerspectives: () => ({
    // Filter by the mock view kind so switching fixtures between the
    // board and grid variants also switches which perspectives the bar
    // renders. Production does the same filtering in
    // `usePerspectiveTabBar`.
    perspectives: fixturePerspectives.map((p) => ({
      ...p,
      view: mockViewKind,
    })),
    activePerspective: {
      ...fixturePerspectives[0],
      view: mockViewKind,
    },
    setActivePerspectiveId: vi.fn(),
    refresh: vi.fn(() => Promise.resolve()),
  }),
}));

vi.mock("@/lib/views-context", () => ({
  useViews: () => ({
    views: [
      {
        id: "board-1",
        name: "Board",
        kind: mockViewKind,
        icon: "kanban",
      },
    ],
    activeView: {
      id: "board-1",
      name: "Board",
      kind: mockViewKind,
      icon: "kanban",
    },
    setActiveViewId: vi.fn(),
    refresh: vi.fn(() => Promise.resolve()),
  }),
}));

vi.mock("@/lib/schema-context", () => ({
  useSchema: () => ({
    getSchema: () => ({ entity: { name: "task", fields: [] }, fields: [] }),
    getFieldDef: () => undefined,
    getEntityCommands: () => [],
    mentionableTypes: [],
    loading: false,
  }),
}));

vi.mock("@/lib/ui-state-context", () => ({
  useUIState: () => ({
    keymap_mode: "cua",
    scope_chain: [],
    open_boards: [],
    has_clipboard: false,
    clipboard_entity_type: null,
    windows: {},
    recent_boards: [],
  }),
  useUIStateLoading: () => ({
    state: {
      keymap_mode: "cua",
      scope_chain: [],
      open_boards: [],
      has_clipboard: false,
      clipboard_entity_type: null,
      windows: {},
      recent_boards: [],
    },
    loading: false,
  }),
}));

vi.mock("@/lib/entity-store-context", () => ({
  useEntityStore: () => ({ getEntities: () => [] }),
}));

vi.mock("@/components/window-container", () => ({
  useBoardData: () => ({ virtualTagMeta: [] }),
}));

// -------------------------------------------------------------------------
// Imports that see the mocks above.
// -------------------------------------------------------------------------

import { setupSpatialShim } from "./setup-spatial-shim";
import {
  AppWithBoardAndPerspectiveFixture,
  AppWithGridAndPerspectiveFixture,
  FIXTURE_PERSPECTIVE_MONIKERS,
} from "./spatial-perspective-fixture";
import { FIXTURE_CARD_MONIKERS } from "./spatial-board-fixture";
import { FIXTURE_CELL_MONIKERS } from "./spatial-grid-fixture";

/**
 * Poll timeout (ms) for `data-focused` / focused-moniker transitions.
 *
 * The shim is synchronous and React flushes well under this budget; when
 * the feature is broken (no perspective scope registered, focus never
 * lands on a tab), the poll fails fast instead of waiting for the default
 * multi-second timeout.
 */
const FOCUS_POLL_TIMEOUT_MS = 500;

/**
 * Resolve the current focused moniker from the shim handles.
 *
 * The handle returned by `setupSpatialShim` reads through the live shim,
 * so assertions stay correct across the inevitable React commit flush
 * after `userEvent.keyboard`.
 */
function makeExpectPerspectiveFocus(handles: {
  focusedMoniker: () => string | null;
}) {
  return async function expectPerspectiveFocused(): Promise<string> {
    await expect
      .poll(() => handles.focusedMoniker(), {
        timeout: FOCUS_POLL_TIMEOUT_MS,
      })
      .toMatch(/^perspective:/);
    return handles.focusedMoniker() as string;
  };
}

describe("perspective bar reachable from all views", () => {
  let handles: ReturnType<typeof setupSpatialShim>;

  beforeEach(() => {
    handles = setupSpatialShim();
  });

  it("k from top-row card in the board moves focus to the active perspective tab", async () => {
    mockViewKind = "board";
    const expectPerspectiveFocused = makeExpectPerspectiveFocus(handles);
    const screen = await render(<AppWithBoardAndPerspectiveFixture />);

    // Top-row cards: card-1-1, card-2-1, card-3-1 (row 1 is the topmost
    // row in the 3x3 board fixture). Use `card-2-1` because `card-1-1`
    // holds the fixture tag pills (see `CARD_TAG_COUNTS` in the board
    // fixture) — those pills' FocusScope children create nested spatial
    // entries that complicate visibility assertions for the top-row
    // contract. The pill-free card exercises the same contract with
    // simpler geometry.
    const topCard = screen
      .getByTestId(`data-moniker:${FIXTURE_CARD_MONIKERS[1][0]}`)
      .element() as HTMLElement;

    await userEvent.click(topCard);
    await expect
      .poll(() => topCard.getAttribute("data-focused"), {
        timeout: FOCUS_POLL_TIMEOUT_MS,
      })
      .toBe("true");

    // `k` is the vim binding for nav.up; the fixture shell runs in vim
    // mode so vim keys drive navigation.
    await userEvent.keyboard("k");
    const focusedMoniker = await expectPerspectiveFocused();

    // Visual indicator: the focused tab's root <div> must carry
    // `data-focused="true"` so the user can see where spatial nav
    // landed. Without this the user sees no change and concludes `k`
    // did nothing (the original bug report). The ring is painted by
    // the global `[data-focused]` CSS rule in `index.css` — see the
    // centralized `useFocusDecoration` in `focus-scope.tsx`.
    const tabEl = screen
      .getByTestId(`data-moniker:${focusedMoniker}`)
      .element() as HTMLElement;
    expect(tabEl.getAttribute("data-focused")).toBe("true");
  });

  it("k from top-row cell in a grid moves focus to the active perspective tab", async () => {
    mockViewKind = "grid";
    const expectPerspectiveFocused = makeExpectPerspectiveFocus(handles);
    const screen = await render(<AppWithGridAndPerspectiveFixture />);

    // Top-row cells: row index 0 in the grid fixture.
    const topLeftCell = screen
      .getByTestId(`data-moniker:${FIXTURE_CELL_MONIKERS[0][0]}`)
      .element() as HTMLElement;

    await userEvent.click(topLeftCell);
    await expect
      .poll(() => topLeftCell.getAttribute("data-focused"), {
        timeout: FOCUS_POLL_TIMEOUT_MS,
      })
      .toBe("true");

    // `k` is the vim binding for nav.up.
    await userEvent.keyboard("k");
    const focusedMoniker = await expectPerspectiveFocused();

    // Visual indicator: the focused tab's root <div> must carry
    // `data-focused="true"` so the user can see where spatial nav
    // landed. Without this the user sees no change and concludes `k`
    // did nothing (the original bug report). The ring is painted by
    // the global `[data-focused]` CSS rule in `index.css` — see the
    // centralized `useFocusDecoration` in `focus-scope.tsx`.
    const tabEl = screen
      .getByTestId(`data-moniker:${focusedMoniker}`)
      .element() as HTMLElement;
    expect(tabEl.getAttribute("data-focused")).toBe("true");
  });

  it("h/l between perspective tabs in the same bar", async () => {
    mockViewKind = "board";
    const expectPerspectiveFocused = makeExpectPerspectiveFocus(handles);
    const screen = await render(<AppWithBoardAndPerspectiveFixture />);

    // Click the TabButton (the perspective name button) specifically.
    // The outer scope `<div>` contains the filter and group icon buttons
    // in addition to the TabButton; clicking the tab body by rect
    // center would land on the filter icon and focus the filter editor
    // instead of the scope. Targeting the name-bearing `<button>` by
    // role is stable against reordering of the secondary icon buttons.
    const defaultTabButton = screen
      .getByRole("button", { name: "Default" })
      .element() as HTMLElement;

    await userEvent.click(defaultTabButton);
    await expect
      .poll(() => handles.focusedMoniker(), { timeout: FOCUS_POLL_TIMEOUT_MS })
      .toBe(FIXTURE_PERSPECTIVE_MONIKERS[0]);

    const defaultTabEl = screen
      .getByTestId(`data-moniker:${FIXTURE_PERSPECTIVE_MONIKERS[0]}`)
      .element() as HTMLElement;
    // First tab has the focus ring right after clicking it.
    await expect
      .poll(() => defaultTabEl.getAttribute("data-focused"), {
        timeout: FOCUS_POLL_TIMEOUT_MS,
      })
      .toBe("true");

    // `l` is the vim binding for nav.right; the fixture shell runs in
    // vim mode.
    await userEvent.keyboard("l");
    const focusedAfterRight = await expectPerspectiveFocused();
    expect(focusedAfterRight).not.toBe(FIXTURE_PERSPECTIVE_MONIKERS[0]);

    // Visual indicator must move with focus: the new tab gains
    // `data-focused="true"`, the previous tab loses it. Guards against
    // a "stuck" ring that would mislead the user about where focus is.
    const newTabEl = screen
      .getByTestId(`data-moniker:${focusedAfterRight}`)
      .element() as HTMLElement;
    await expect
      .poll(() => newTabEl.getAttribute("data-focused"), {
        timeout: FOCUS_POLL_TIMEOUT_MS,
      })
      .toBe("true");
    expect(defaultTabEl.getAttribute("data-focused")).toBeNull();
  });

  it("j from an active perspective tab moves into the active view", async () => {
    mockViewKind = "board";
    const screen = await render(<AppWithBoardAndPerspectiveFixture />);

    // Click the TabButton rather than the scope's outer `<div>` —
    // see the `h/l` test above for why. Clicking the name button
    // focuses the perspective scope without routing the click into
    // the filter icon.
    const defaultTabButton = screen
      .getByRole("button", { name: "Default" })
      .element() as HTMLElement;

    await userEvent.click(defaultTabButton);
    await expect
      .poll(() => handles.focusedMoniker(), { timeout: FOCUS_POLL_TIMEOUT_MS })
      .toBe(FIXTURE_PERSPECTIVE_MONIKERS[0]);

    // `j` is the vim binding for nav.down; the fixture shell runs in
    // vim mode.
    await userEvent.keyboard("j");

    // The perspective moniker prefix is `perspective:`; any non-perspective
    // moniker (a card, column, or cell) is a valid landing spot. The
    // contract is "leaves the tab bar", not "lands on a specific entry".
    await expect
      .poll(() => handles.focusedMoniker(), { timeout: FOCUS_POLL_TIMEOUT_MS })
      .not.toMatch(/^perspective:/);
    expect(handles.focusedMoniker()).not.toBeNull();
  });
});
