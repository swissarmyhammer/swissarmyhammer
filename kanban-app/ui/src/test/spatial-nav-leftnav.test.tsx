/**
 * Spatial-navigation contract between the LeftNav strip and view bodies.
 *
 * LeftNav sits on the left edge of the window. The contract under test:
 *
 * 1. `h` from the leftmost spatial entry of a view body (leftmost card
 *    in a board column, row selector in a grid) crosses the left edge
 *    into the nearest LeftNav button — no clamp at the view's left
 *    edge.
 * 2. `j` / `k` between LeftNav buttons moves focus between views
 *    (the strip is vertical).
 * 3. `l` from a LeftNav button re-enters the active view body.
 *
 * These tests render the real production `<LeftNav />` composed with
 * the standard board and grid fixtures. `useViews()` is mocked at the
 * module level so the LeftNav receives a deterministic two-view list.
 *
 * Under HEAD (LeftNav renders plain `<button>`s with no FocusScope)
 * every test is RED. When LeftNav wraps each button in a
 * `FocusScope` with `moniker("view", id)`, `renderContainer={false}`,
 * `showFocusBar={false}` (per kanban task 01KPNWPX9NWSVGTJAHB4Z1VSED),
 * the tests flip green.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { userEvent } from "vitest/browser";
import { render } from "vitest-browser-react";

// Hoisted Tauri mocks — route every `invoke()` into `SpatialStateShim`
// and ignore everything else (returns null). Same pattern as
// `spatial-nav-board.test.tsx`.
vi.mock("@tauri-apps/api/core", async () => {
  const { tauriCoreMock } = await import("./setup-spatial-shim");
  return tauriCoreMock();
});
vi.mock("@tauri-apps/api/event", async () => {
  const { tauriEventMock } = await import("./setup-spatial-shim");
  return tauriEventMock();
});
vi.mock("@tauri-apps/api/window", async () => {
  const { tauriWindowMock } = await import("./setup-spatial-shim");
  return tauriWindowMock();
});
vi.mock("@tauri-apps/api/webviewWindow", async () => {
  const { tauriWebviewWindowMock } = await import("./setup-spatial-shim");
  return tauriWebviewWindowMock();
});
vi.mock("@tauri-apps/plugin-log", async () => {
  const { tauriPluginLogMock } = await import("./setup-spatial-shim");
  return tauriPluginLogMock();
});

// Mock `useViews()` so LeftNav renders a deterministic two-view list
// without a real UIState / Tauri backend. The factory must be
// self-contained — vitest hoists `vi.mock(...)` to the top of the
// file so any value referenced inside the factory must be defined
// inside it (no closure over module-level names). The view list
// shape duplicated here matches `FIXTURE_VIEWS` in
// `./spatial-leftnav-fixture`; both sides reference the same
// `moniker("view", id)` shape so the monikers line up.
vi.mock("@/lib/views-context", () => {
  const views = [
    { id: "board", name: "Board", kind: "board", icon: "kanban" },
    { id: "grid", name: "Grid", kind: "grid", icon: "table" },
  ] as const;
  return {
    ViewsProvider: ({ children }: { children: React.ReactNode }) => children,
    useViews: () => ({
      views,
      activeView: views[0],
      setActiveViewId: vi.fn(),
      refresh: vi.fn(),
    }),
  };
});

import {
  setupSpatialShim,
  type SpatialShimHandles,
} from "./setup-spatial-shim";
import {
  AppWithBoardAndLeftNavFixture,
  AppWithGridAndLeftNavFixture,
  FIXTURE_CARD_MONIKERS,
  FIXTURE_ROW_SELECTOR_MONIKERS,
  FIXTURE_VIEW_MONIKERS,
} from "./spatial-leftnav-fixture";

/**
 * Tight poll timeout (ms) for `data-focused` attribute assertions.
 *
 * The shim is synchronous; React render + state flip happens well
 * under this budget. Keeping the window tight makes failures surface
 * fast instead of waiting for the default multi-second timeout.
 */
const FOCUS_POLL_TIMEOUT_MS = 500;

/** Poll for an element's `data-focused` attribute value. */
async function expectDataFocused(
  el: HTMLElement,
  expected: "true" | null,
): Promise<void> {
  await expect
    .poll(() => el.getAttribute("data-focused"), {
      timeout: FOCUS_POLL_TIMEOUT_MS,
    })
    .toBe(expected);
}

/** Poll until the shim reports a focused moniker matching the regex. */
async function expectFocusedMonikerMatches(
  handles: SpatialShimHandles,
  pattern: RegExp,
): Promise<void> {
  await expect
    .poll(() => handles.focusedMoniker(), { timeout: FOCUS_POLL_TIMEOUT_MS })
    .toMatch(pattern);
}

/** Fresh shim handles per test. */
let handles: SpatialShimHandles;

describe("LeftNav reachable from all views", () => {
  beforeEach(() => {
    handles = setupSpatialShim();
  });

  it("h from leftmost card in a board column moves focus to a LeftNav button", async () => {
    const screen = await render(<AppWithBoardAndLeftNavFixture />);

    // Leftmost column (col 1), first card (row 1) — the card at the
    // western edge of the board body. `h` from here must cross the
    // board's left edge into the LeftNav strip.
    const card11 = screen
      .getByTestId(`data-moniker:${FIXTURE_CARD_MONIKERS[0][0]}`)
      .element() as HTMLElement;

    await userEvent.click(card11);
    await expectDataFocused(card11, "true");

    await userEvent.keyboard("h");

    // Cross the board's left edge — focus must land on *some* view
    // button. The spatial engine picks the closest one by vertical
    // alignment; we don't assert which one, only that the moniker has
    // the `view:` prefix so a future re-ordering of LeftNav entries
    // doesn't make this test brittle.
    await expectFocusedMonikerMatches(handles, /^view:/);
  });

  it("h from a grid row selector moves focus to a LeftNav button", async () => {
    const screen = await render(<AppWithGridAndLeftNavFixture />);

    // Row selector for row 0 — the westernmost spatial entry in the
    // grid body. `h` from here must cross the grid's left edge into
    // the LeftNav strip.
    const selectorRow0 = screen
      .getByTestId(`data-moniker:${FIXTURE_ROW_SELECTOR_MONIKERS[0]}`)
      .element() as HTMLElement;

    await userEvent.click(selectorRow0);
    await expectDataFocused(selectorRow0, "true");

    await userEvent.keyboard("h");

    await expectFocusedMonikerMatches(handles, /^view:/);
  });

  it("j moves focus between LeftNav view buttons", async () => {
    const screen = await render(<AppWithBoardAndLeftNavFixture />);

    // The fixture provides two views; the first (index 0) is
    // topmost. Click it, then `j` should land on the next view's
    // button below it.
    //
    // LeftNav buttons use `showFocusBar={false}` because the strip's
    // own `data-active` styling is already the primary visual —
    // duplicating it with the `FocusScope` focus bar would be
    // redundant. Consequently, the `<button>`'s `data-focused`
    // attribute never flips to `"true"`, and assertions here read
    // from the shim's focused-moniker snapshot instead.
    const topButton = screen
      .getByTestId(`data-moniker:${FIXTURE_VIEW_MONIKERS[0]}`)
      .element() as HTMLElement;

    await userEvent.click(topButton);
    await expect
      .poll(() => handles.focusedMoniker(), { timeout: FOCUS_POLL_TIMEOUT_MS })
      .toBe(FIXTURE_VIEW_MONIKERS[0]);

    await userEvent.keyboard("j");

    await expect
      .poll(() => handles.focusedMoniker(), { timeout: FOCUS_POLL_TIMEOUT_MS })
      .toBe(FIXTURE_VIEW_MONIKERS[1]);
  });

  it("l from an active LeftNav button moves focus into the active view body", async () => {
    const screen = await render(<AppWithBoardAndLeftNavFixture />);

    // Start on the active view's button. The production LeftNav
    // marks the first view as active via `data-active`, which in the
    // fixture corresponds to `FIXTURE_VIEW_MONIKERS[0]`.
    const activeButton = screen
      .getByTestId(`data-moniker:${FIXTURE_VIEW_MONIKERS[0]}`)
      .element() as HTMLElement;

    await userEvent.click(activeButton);

    // Focus is on the view button after the click.
    await expect
      .poll(() => handles.focusedMoniker(), { timeout: FOCUS_POLL_TIMEOUT_MS })
      .toBe(FIXTURE_VIEW_MONIKERS[0]);

    await userEvent.keyboard("l");

    // Crossing into the board body — focus lands on some card, column,
    // or other non-view moniker. The test asserts only the negative:
    // we are no longer focused on a `view:` moniker. This keeps the
    // test resilient to the spatial engine's choice of which card to
    // land on (beam-test picks whichever rect sits nearest the
    // button's vertical centre).
    await expect
      .poll(() => handles.focusedMoniker(), { timeout: FOCUS_POLL_TIMEOUT_MS })
      .not.toMatch(/^view:/);
  });
});
