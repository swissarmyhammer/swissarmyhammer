/**
 * Perspective bar navigation — React/dispatch boundary tests.
 *
 * Asserts the React wiring: `ScopedPerspectiveTab` registers its
 * `perspective:*` moniker as a spatial entry and keypresses dispatch
 * `nav.*` through the command pipeline. The algorithm that chooses
 * which tab (or card/cell) wins navigation is Rust's concern.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { userEvent } from "vitest/browser";
import { render } from "vitest-browser-react";

// -------------------------------------------------------------------------
// Tauri mocks — route every invoke through the boundary stub.
// -------------------------------------------------------------------------

// Preserve the real `@tauri-apps/api/core` exports (SERIALIZE_TO_IPC_FN,
// Resource, Channel, PluginListener, etc.) so transitively-imported
// sibling modules (`window.js`, `webviewWindow.js`, `dpi.js`) can resolve
// their re-exports. Only override `invoke` to route calls into the stub.
vi.mock("@tauri-apps/api/core", async () => {
  const actual = await vi.importActual<typeof import("@tauri-apps/api/core")>(
    "@tauri-apps/api/core",
  );
  const { tauriCoreMock } = await import("./setup-tauri-stub");
  const { invoke } = tauriCoreMock();
  return { ...actual, invoke };
});
vi.mock("@tauri-apps/api/event", async () => {
  const actual = await vi.importActual<typeof import("@tauri-apps/api/event")>(
    "@tauri-apps/api/event",
  );
  const { tauriEventMock } = await import("./setup-tauri-stub");
  return { ...actual, ...tauriEventMock() };
});
vi.mock("@tauri-apps/api/window", async () => {
  const actual = await vi.importActual<typeof import("@tauri-apps/api/window")>(
    "@tauri-apps/api/window",
  );
  const { tauriWindowMock } = await import("./setup-tauri-stub");
  return { ...actual, ...tauriWindowMock() };
});
vi.mock("@tauri-apps/api/webviewWindow", async () => {
  const actual = await vi.importActual<
    typeof import("@tauri-apps/api/webviewWindow")
  >("@tauri-apps/api/webviewWindow");
  const { tauriWebviewWindowMock } = await import("./setup-tauri-stub");
  return { ...actual, ...tauriWebviewWindowMock() };
});
vi.mock("@tauri-apps/plugin-log", async () => {
  const { tauriPluginLogMock } = await import("./setup-tauri-stub");
  return tauriPluginLogMock();
});

// -------------------------------------------------------------------------
// Context mocks — minimal fixture data for every provider the
// perspective tab bar reads.
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
      { id: "board-1", name: "Board", kind: mockViewKind, icon: "kanban" },
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

import { setupTauriStub, type TauriStubHandles } from "./setup-tauri-stub";
import {
  AppWithBoardAndPerspectiveFixture,
  FIXTURE_PERSPECTIVE_MONIKERS,
} from "./spatial-perspective-fixture";
import { FIXTURE_CARD_MONIKERS } from "./spatial-board-fixture";

const POLL_TIMEOUT = 500;

describe("perspective bar — React/dispatch boundary", () => {
  let handles: TauriStubHandles;

  beforeEach(() => {
    handles = setupTauriStub();
    mockViewKind = "board";
  });

  it("every perspective tab registers its moniker as a spatial entry", async () => {
    await render(<AppWithBoardAndPerspectiveFixture />);

    for (const pMoniker of FIXTURE_PERSPECTIVE_MONIKERS) {
      await expect
        .poll(
          () =>
            handles.invocations().some((i) => {
              if (i.cmd !== "spatial_register") return false;
              const a = i.args as { args?: { moniker?: string } };
              return a.args?.moniker === pMoniker;
            }),
          { timeout: POLL_TIMEOUT },
        )
        .toBe(true);
    }
  });

  it("pressing k from a top-row card dispatches nav.up", async () => {
    const screen = await render(<AppWithBoardAndPerspectiveFixture />);
    const topCard = screen
      .getByTestId(`data-moniker:${FIXTURE_CARD_MONIKERS[1][0]}`)
      .element() as HTMLElement;

    await userEvent.click(topCard);
    const before = handles.dispatchedCommands().length;
    await userEvent.keyboard("k");

    await expect
      .poll(
        () =>
          handles
            .dispatchedCommands()
            .slice(before)
            .some((d) => d.cmd === "nav.up"),
        { timeout: POLL_TIMEOUT },
      )
      .toBe(true);
  });

  it("scripted focus-changed on nav.up lands on the active perspective tab", async () => {
    const screen = await render(<AppWithBoardAndPerspectiveFixture />);
    const topCardMk = FIXTURE_CARD_MONIKERS[1][0];
    const tabMk = FIXTURE_PERSPECTIVE_MONIKERS[0];
    const topCard = screen
      .getByTestId(`data-moniker:${topCardMk}`)
      .element() as HTMLElement;
    const tabEl = screen
      .getByTestId(`data-moniker:${tabMk}`)
      .element() as HTMLElement;

    handles.scriptResponse("dispatch_command:nav.up", () =>
      handles.payloadForFocusMove(topCardMk, tabMk),
    );

    await userEvent.click(topCard);
    await expect
      .poll(() => topCard.getAttribute("data-focused"), {
        timeout: POLL_TIMEOUT,
      })
      .toBe("true");

    await userEvent.keyboard("k");
    await expect
      .poll(() => tabEl.getAttribute("data-focused"), { timeout: POLL_TIMEOUT })
      .toBe("true");
  });

  // ------------------------------------------------------------------
  // Tab-row horizontal nav regression tests (card 01KPVT95H4FTCC5Q4E7G644CHD).
  //
  // The original bug report: with focus on a perspective tab, pressing
  // `l` (nav.right) or `h` (nav.left) "silently lost focus" — no
  // adjacent tab gained decoration. The Rust algorithm is already
  // pinned by `navigate_right_from_tab_reaches_adjacent_tab_not_parent_or_sibling_control`
  // and `navigate_left_from_tab_reaches_adjacent_tab` in
  // `spatial_state.rs`. These tests pin the React-side dispatch path:
  //
  //   click tab1 → `l` keypress → `nav.right` dispatched → scripted
  //   focus-changed(next_key = tab2's key) → tab2's `data-focused`
  //   flips to "true", tab1's clears.
  //
  // This is the exact dispatch→event→decoration loop the live app
  // was silently breaking on. If a future change regresses any link
  // in the chain (keybinding registration, command dispatch,
  // focus-changed listener, `useFocusDecoration` write), one of these
  // tests will fail.
  // ------------------------------------------------------------------

  it("pressing l from a focused perspective tab dispatches nav.right and lands on the adjacent tab", async () => {
    const screen = await render(<AppWithBoardAndPerspectiveFixture />);
    const [mk1, mk2] = FIXTURE_PERSPECTIVE_MONIKERS;
    const tab1 = screen
      .getByTestId(`data-moniker:${mk1}`)
      .element() as HTMLElement;
    const tab2 = screen
      .getByTestId(`data-moniker:${mk2}`)
      .element() as HTMLElement;

    handles.scriptResponse("dispatch_command:nav.right", () =>
      handles.payloadForFocusMove(mk1, mk2),
    );

    await userEvent.click(tab1);
    await expect
      .poll(() => tab1.getAttribute("data-focused"), { timeout: POLL_TIMEOUT })
      .toBe("true");

    const before = handles.dispatchedCommands().length;
    await userEvent.keyboard("l");

    // Assert the real dispatch path — `l` must dispatch `nav.right`
    // through the command pipeline (not a shim), not a silent no-op.
    await expect
      .poll(
        () =>
          handles
            .dispatchedCommands()
            .slice(before)
            .some((d) => d.cmd === "nav.right"),
        { timeout: POLL_TIMEOUT },
      )
      .toBe(true);

    // Assert the scripted focus-changed flips decoration to the
    // adjacent tab — no silent focus loss.
    await expect
      .poll(() => tab2.getAttribute("data-focused"), { timeout: POLL_TIMEOUT })
      .toBe("true");
  });

  it("pressing h from a focused perspective tab dispatches nav.left and lands on the adjacent tab", async () => {
    const screen = await render(<AppWithBoardAndPerspectiveFixture />);
    const [mk1, mk2] = FIXTURE_PERSPECTIVE_MONIKERS;
    const tab1 = screen
      .getByTestId(`data-moniker:${mk1}`)
      .element() as HTMLElement;
    const tab2 = screen
      .getByTestId(`data-moniker:${mk2}`)
      .element() as HTMLElement;

    handles.scriptResponse("dispatch_command:nav.left", () =>
      handles.payloadForFocusMove(mk2, mk1),
    );

    await userEvent.click(tab2);
    await expect
      .poll(() => tab2.getAttribute("data-focused"), { timeout: POLL_TIMEOUT })
      .toBe("true");

    const before = handles.dispatchedCommands().length;
    await userEvent.keyboard("h");

    await expect
      .poll(
        () =>
          handles
            .dispatchedCommands()
            .slice(before)
            .some((d) => d.cmd === "nav.left"),
        { timeout: POLL_TIMEOUT },
      )
      .toBe(true);

    await expect
      .poll(() => tab1.getAttribute("data-focused"), { timeout: POLL_TIMEOUT })
      .toBe("true");
  });
});
