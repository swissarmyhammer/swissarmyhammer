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

    // The focused LeftNav button must render a visible focus indicator
    // — spatial focus is correct only when the user can *see* which
    // button carries it. `data-focused="true"` is written by
    // `FocusScope`'s centralized `useFocusDecoration` hook; the global
    // `[data-focused]` CSS rule in `index.css` paints the ring.
    const focusedMk = handles.focusedMoniker();
    expect(focusedMk).toBeTruthy();
    const focusedButton = screen
      .getByTestId(`data-moniker:${focusedMk}`)
      .element() as HTMLElement;
    await expectDataFocused(focusedButton, "true");
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

    // Same visual-indicator contract as the board counterpart — the
    // focused LeftNav button must carry `data-focused="true"`. The
    // ring is painted by the global `[data-focused]` CSS rule in
    // `index.css`.
    const focusedMk = handles.focusedMoniker();
    expect(focusedMk).toBeTruthy();
    const focusedButton = screen
      .getByTestId(`data-moniker:${focusedMk}`)
      .element() as HTMLElement;
    await expectDataFocused(focusedButton, "true");
  });

  it("j moves focus between LeftNav view buttons", async () => {
    const screen = await render(<AppWithBoardAndLeftNavFixture />);

    // The fixture provides two views; the first (index 0) is
    // topmost. Click it, then `j` should land on the next view's
    // button below it.
    //
    // Assertions here read from the shim's `focusedMoniker()`
    // snapshot rather than resolving the button element first —
    // polling the moniker directly avoids a stale-node race between
    // the `j` keypress and the next render.
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

  it("Enter on a focused LeftNav button dispatches view.switch", async () => {
    const screen = await render(<AppWithBoardAndLeftNavFixture />);

    // Focus the second view's button by clicking it — the click path
    // and the Enter path must dispatch the exact same command
    // (`view.switch:<id>` for the focused button's view), so we assert
    // on the dispatch that happens *after* the click. Focus stays on
    // the button because `handleClick` calls `setFocus(mk)` before
    // dispatching.
    const gridButton = screen
      .getByTestId(`data-moniker:${FIXTURE_VIEW_MONIKERS[1]}`)
      .element() as HTMLElement;

    await userEvent.click(gridButton);
    await expect
      .poll(() => handles.focusedMoniker(), { timeout: FOCUS_POLL_TIMEOUT_MS })
      .toBe(FIXTURE_VIEW_MONIKERS[1]);

    // The click itself already fired `view.switch:grid` through the
    // backend; clear the log so the Enter-press dispatch is the only
    // one the subsequent assertion sees.
    const afterClickCount = handles.dispatchedCommands().length;
    expect(
      handles.dispatchedCommands().some((d) => d.cmd === "view.switch:grid"),
    ).toBe(true);

    // Enter must resolve to the LeftNav button's scope-local
    // `view.activate.<id>` command, which reuses `handleClick` → so
    // another `view.switch:grid` lands on the backend. Assert the new
    // dispatch observed after the Enter press, and that focus remains
    // on the same button (identical to the click path).
    await userEvent.keyboard("{Enter}");

    await expect
      .poll(() => handles.dispatchedCommands().length, {
        timeout: FOCUS_POLL_TIMEOUT_MS,
      })
      .toBeGreaterThan(afterClickCount);

    const enterDispatches = handles.dispatchedCommands().slice(afterClickCount);
    expect(enterDispatches.some((d) => d.cmd === "view.switch:grid")).toBe(
      true,
    );

    // Focus has not moved — the Enter path lands focus on the same
    // button, matching the click path's `setFocus(mk)` side-effect.
    await expect
      .poll(() => handles.focusedMoniker(), { timeout: FOCUS_POLL_TIMEOUT_MS })
      .toBe(FIXTURE_VIEW_MONIKERS[1]);
  });
});

/**
 * Focus-invariant regression tests (kanban `01KPRGGCB5NYPW28AJZNM3D0QT`).
 *
 * The "something is always focused" invariant says: while at least one
 * scope is registered in the active layer, `focused_moniker` is never
 * null after any sequence of register/unregister/navigate. The
 * LeftNav-triggered view swap is the reproduction the user reported —
 * clicking a view-switcher button unmounts the old view, which would
 * previously clear `focused_key` to `None` and wedge the user.
 *
 * These tests simulate the critical half of that flow — the focused
 * body scope going away without the view actually swapping — and
 * assert that focus is always recoverable by a nav key.
 */
describe("LeftNav focus invariant", () => {
  beforeEach(() => {
    handles = setupSpatialShim();
  });

  /**
   * Silently delete an entry from the shim without emitting
   * `focus-changed`. Modeling a React/Rust desync where Rust has
   * forgotten the focused key but React hasn't caught up yet — the
   * exact state a user can fall into during a view swap if any of the
   * emission paths race with the next keypress.
   *
   * Lives inline in this test (rather than being imported) because
   * reaching into the shim's internals is something that must not leak
   * into production helpers.
   */
  function silentlyDropEntry(h: SpatialShimHandles, key: string): void {
    const internals = h.shim as unknown as { entries: Map<string, unknown> };
    internals.entries.delete(key);
  }

  it("nav key after focused body scope disappears recovers onto a registered moniker", async () => {
    const screen = await render(<AppWithBoardAndLeftNavFixture />);

    // 1) Click a card in the leftmost column. React thinks the card is
    //    focused; Rust agrees. This is the body-cell start state.
    const card = screen
      .getByTestId(`data-moniker:${FIXTURE_CARD_MONIKERS[0][0]}`)
      .element() as HTMLElement;
    await userEvent.click(card);
    await expectDataFocused(card, "true");

    // 2) Silently drop Rust's entry for the card, simulating what a
    //    view swap would do (unmount → unregister) but without the
    //    `focus-changed` emission. The old behavior: Rust's
    //    `focused_key` stays pointing at a gone entry, the next nav
    //    key sends a stale key, Rust returns `Ok(None)`, JS's
    //    short-circuit on null moniker also returns false, and the
    //    user is wedged. The new behavior: even though Rust is
    //    silently desynced, the fallback-to-first safety net picks a
    //    real entry and emits `focus-changed`, so React recovers.
    const focusedMk = handles.focusedMoniker();
    expect(focusedMk).toBeTruthy();
    // Find the spatial key for the focused moniker by looking it up
    // in the shim's entries (parallels `keyForMoniker` in the
    // stale-key repro test).
    const staleKey = handles.shim
      .entriesSnapshot()
      .find((e) => e.moniker === focusedMk)?.key;
    expect(staleKey).toBeTruthy();
    silentlyDropEntry(handles, staleKey!);

    // 3) Press `j`. Rust sees the unknown source key, falls back to
    //    first-in-layer, emits `focus-changed` with a real successor.
    //    The safety net MUST restore a non-null focused moniker.
    await userEvent.keyboard("j");

    await expect
      .poll(() => handles.focusedMoniker(), { timeout: FOCUS_POLL_TIMEOUT_MS })
      .toBeTruthy();
  });

  it("nav key always produces a focused moniker, even after focus goes null", async () => {
    const screen = await render(<AppWithBoardAndLeftNavFixture />);

    // The fixture auto-focuses the active view button on mount. Clear
    // focus by dispatching `spatial_clear_focus` through the shim so
    // React's `focusedMoniker` returns to null — this is the exact
    // state that short-circuited `broadcastNavCommand` before the
    // fix, leaving the user wedged on "nothing focused, nav keys do
    // nothing."
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("spatial_clear_focus", {});

    await expect
      .poll(() => handles.focusedMoniker(), { timeout: FOCUS_POLL_TIMEOUT_MS })
      .toBeNull();

    // Re-focus the fixture root so the keyboard event is routed to
    // the correct window — clearing focus unfocused the DOM element
    // that was previously receiving keystrokes.
    (
      screen
        .getByTestId("board-and-leftnav-fixture-root")
        .element() as HTMLElement
    ).focus();

    await userEvent.keyboard("j");

    // The invariant: a nav key must always produce a non-null focused
    // moniker when the active layer has any registered scope, even
    // when React's pre-nav moniker was null.
    await expect
      .poll(() => handles.focusedMoniker(), { timeout: FOCUS_POLL_TIMEOUT_MS })
      .toBeTruthy();
  });
});
