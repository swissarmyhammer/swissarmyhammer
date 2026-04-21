/**
 * Board-view spatial navigation contract tests.
 *
 * Codifies the h/j/k/l navigation behavior expected between cards on a
 * 3-column × 3-card board:
 *
 * - `j`/`k` moves within a column (to the card below / above)
 * - `h`/`l` moves between columns (to the nearest card at the same
 *   vertical level)
 * - At column or row edges, nav clamps — no wrap-around
 *
 * Manual testing confirms this works today; no automated test existed
 * before this file. The corresponding kanban task
 * (01KPNWNEN1A9VV67A0XPC2PJP9) calls out that a regression is one bad
 * edit away — these tests lock the contract in so a future change
 * that breaks board nav fails fast.
 *
 * ## Test harness
 *
 * Uses the in-process `SpatialStateShim` via `setupSpatialShim()` —
 * no tauri-driver, no external Tauri backend. The shim is
 * behavior-equivalent to the Rust `SpatialState` (verified by
 * `spatial-shim-parity.test.ts`), so a pass here is a pass for the
 * real navigation engine.
 *
 * See `spatial-board-fixture.tsx` for the fixture and
 * `spatial-nav-canonical.test.tsx` for the grid-equivalent pattern.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { userEvent } from "vitest/browser";
import { render } from "vitest-browser-react";

// Vitest hoists `vi.mock(...)` to the top of this file; the factories
// live in `setup-spatial-shim` so tests only need to copy this
// boilerplate to route `@tauri-apps/api/*` calls into the shim.
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

import {
  setupSpatialShim,
  type SpatialShimHandles,
} from "./setup-spatial-shim";
import {
  AppWithBoardFixture,
  FIXTURE_CARD_MONIKERS,
} from "./spatial-board-fixture";

/** Poll timeout for `data-focused` attribute assertions (ms). */
const FOCUS_POLL_TIMEOUT = 500;

/**
 * Wait for the DOM element's `data-focused` attribute to flip to
 * `"true"`.
 *
 * The spatial shim is synchronous but React claim callbacks flush via
 * state updates, so assertions need a short poll window. 500ms is
 * well above typical flush latency in vitest-browser without tying CI
 * up on failure cases.
 */
async function expectFocused(el: HTMLElement): Promise<void> {
  await expect
    .poll(() => el.getAttribute("data-focused"), {
      timeout: FOCUS_POLL_TIMEOUT,
    })
    .toBe("true");
}

/** Fixture shim handles — fresh per test, captured by `beforeEach`. */
let handles: SpatialShimHandles;

describe("board card navigation", () => {
  beforeEach(() => {
    handles = setupSpatialShim();
  });

  it("j moves focus from card(col 1, row 1) to card(col 1, row 2)", async () => {
    const screen = await render(<AppWithBoardFixture />);
    const card11 = screen
      .getByTestId(`data-moniker:${FIXTURE_CARD_MONIKERS[0][0]}`)
      .element() as HTMLElement;
    const card12 = screen
      .getByTestId(`data-moniker:${FIXTURE_CARD_MONIKERS[0][1]}`)
      .element() as HTMLElement;

    await userEvent.click(card11);
    await expectFocused(card11);

    await userEvent.keyboard("j");
    await expectFocused(card12);
  });

  it("l moves focus across columns at the same vertical level", async () => {
    const screen = await render(<AppWithBoardFixture />);
    const card12 = screen
      .getByTestId(`data-moniker:${FIXTURE_CARD_MONIKERS[0][1]}`)
      .element() as HTMLElement;
    const card22 = screen
      .getByTestId(`data-moniker:${FIXTURE_CARD_MONIKERS[1][1]}`)
      .element() as HTMLElement;

    await userEvent.click(card12);
    await expectFocused(card12);

    await userEvent.keyboard("l");
    await expectFocused(card22);
  });

  it("j at the bottom card of a column stays put (clamped)", async () => {
    const screen = await render(<AppWithBoardFixture />);
    const card23 = screen
      .getByTestId(`data-moniker:${FIXTURE_CARD_MONIKERS[1][2]}`)
      .element() as HTMLElement;

    await userEvent.click(card23);
    await expectFocused(card23);

    // No card below `card-2-3`; `nav.down` from the bottom of a column
    // must NOT wrap to the top of that column or jump to another column.
    // Focus must remain on card-2-3.
    await userEvent.keyboard("j");

    // Negative assertion: confirm focus has NOT moved. The `SpatialStateShim`
    // is strictly synchronous — `invoke("spatial_navigate", ...)` resolves
    // before this line runs, so if the engine had picked a candidate, the
    // subsequent emit + React state update would already be in flight.
    // The fixed wait is purely to let any such React state flush land; it
    // is bounded by React's commit loop, not by spatial nav latency, so it
    // stays valid even if the engine later gets slower. Parity is locked
    // down by `spatial-shim-parity.test.ts`.
    await new Promise((r) => setTimeout(r, 100));
    expect(handles.focusedMoniker()).toBe(FIXTURE_CARD_MONIKERS[1][2]);
    expect(card23.getAttribute("data-focused")).toBe("true");
  });

  it("h at the leftmost column does NOT wrap to the rightmost column", async () => {
    const screen = await render(<AppWithBoardFixture />);
    const card12 = screen
      .getByTestId(`data-moniker:${FIXTURE_CARD_MONIKERS[0][1]}`)
      .element() as HTMLElement;

    await userEvent.click(card12);
    await expectFocused(card12);

    await userEvent.keyboard("h");

    // Same sync-shim rationale as the clamp test above: the shim resolves
    // synchronously inside `userEvent.keyboard`, so any would-be wrap
    // navigation would already be queued as a React state update. The
    // 100ms buffer only covers the React flush, not spatial nav latency.
    await new Promise((r) => setTimeout(r, 100));

    // The engine may either clamp (focus stays on card-1-2) or cross a
    // spatial boundary into a non-card moniker (e.g. left nav) — but it
    // MUST NOT wrap around to a card in column 3. The contract here is
    // "no wrap", not "must stay put".
    //
    // `focused` may be null (focus cleared) or any non-column-3 moniker —
    // guard the regex check so `toMatch` doesn't throw on a null value.
    const focused = handles.focusedMoniker();
    if (focused !== null) {
      expect(focused).not.toMatch(/^task:card-3-/);
    }
  });
});
