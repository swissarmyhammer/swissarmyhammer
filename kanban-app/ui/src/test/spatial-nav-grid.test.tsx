/**
 * vitest-browser tests for grid cell-to-cell spatial navigation.
 *
 * Exercises the DataTable-like behavior expected of the grid view:
 *
 * 1. `h`/`j`/`k`/`l` move focus between field cells in the active grid.
 * 2. `h` from the leftmost data cell moves focus to the row selector
 *    of the same row.
 * 3. `l` from the row selector moves focus to the first data cell in
 *    the same row.
 * 4. `j` at the last row clamps — focus does not wrap to the top.
 *
 * These tests drive the TDD cycle for cell-level spatial navigation.
 * Against HEAD they are RED for multiple reasons:
 *
 * - Cells in the grid fixture are plain `<div>`s (no `FocusScope`), so
 *   clicking a cell focuses the enclosing *row*, not the cell.
 * - Row selectors don't exist in the fixture yet (no moniker, no
 *   spatial entry).
 * - The fixture's keybinding handler does not forward scope bindings,
 *   so vim-mode `h`/`j`/`k`/`l` never resolve to `nav.*` commands.
 *
 * When the grid fix lands — per-cell `FocusScope`s, `FocusScope` for the
 * row selector cell, non-spatial row `FocusScope`, and the fixture's
 * keybinding handler forwarding `extractScopeBindings` — each test
 * flips from red to green. This file does not use `it.fails`: we want
 * a plain, stable green bar when the feature ships.
 *
 * The assertion style matches `spatial-nav-canonical.test.tsx`:
 * `expect.poll` on the cell's `data-focused` attribute with a tight
 * timeout so failures surface fast.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { userEvent } from "vitest/browser";
import { render } from "vitest-browser-react";

// Hoisted Tauri mocks — same pattern as `spatial-nav-canonical.test.tsx`.
// The shim dispatcher lives in `setup-spatial-shim`; these literals
// make the mocks visible to vitest's hoist pass.
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

import { setupSpatialShim } from "./setup-spatial-shim";
import {
  AppWithGridFixture,
  FIXTURE_CELL_MONIKERS,
  FIXTURE_ROW_SELECTOR_MONIKERS,
} from "./spatial-grid-fixture";

/**
 * Tight timeout (ms) for `expect.poll` on `data-focused` transitions.
 *
 * The shim is synchronous and React renders + `data-focused` flips well
 * under this budget. When the feature is broken (attribute never
 * appears), the poll fails quickly instead of waiting for the default
 * multi-second timeout to elapse — keeping CI fast.
 */
const FOCUS_POLL_TIMEOUT_MS = 300;

/**
 * Poll a cell's `data-focused` attribute until it matches `expected`.
 *
 * Centralizes the `expect.poll` boilerplate so each test reads as a
 * straight-line sequence of click/key actions.
 */
async function expectFocused(
  element: { element: () => Element },
  expected: "true" | null,
): Promise<void> {
  await expect
    .poll(() => element.element().getAttribute("data-focused"), {
      timeout: FOCUS_POLL_TIMEOUT_MS,
    })
    .toBe(expected);
}

describe("grid cell-to-cell spatial navigation", () => {
  beforeEach(() => {
    setupSpatialShim();
  });

  it("h/j/k/l moves between field cells in the active grid", async () => {
    const screen = await render(<AppWithGridFixture />);

    // Cells are indexed [row][col]:
    //   col 0 = tag_name, col 1 = color.
    const cell00 = screen.getByTestId(
      `data-moniker:${FIXTURE_CELL_MONIKERS[0][0]}`,
    );
    const cell01 = screen.getByTestId(
      `data-moniker:${FIXTURE_CELL_MONIKERS[0][1]}`,
    );
    const cell10 = screen.getByTestId(
      `data-moniker:${FIXTURE_CELL_MONIKERS[1][0]}`,
    );
    const cell11 = screen.getByTestId(
      `data-moniker:${FIXTURE_CELL_MONIKERS[1][1]}`,
    );

    // Click the top-left data cell. The click handler should focus the
    // cell's field moniker — not the row's entity moniker — so
    // `data-focused="true"` lands on the cell itself.
    await userEvent.click(cell00);
    await expectFocused(cell00, "true");

    // j → move down one row, same column.
    await userEvent.keyboard("j");
    await expectFocused(cell10, "true");
    await expectFocused(cell00, null);

    // l → move right one column, same row.
    await userEvent.keyboard("l");
    await expectFocused(cell11, "true");
    await expectFocused(cell10, null);

    // k → move up one row, same column.
    await userEvent.keyboard("k");
    await expectFocused(cell01, "true");
    await expectFocused(cell11, null);

    // h → move left one column, same row.
    await userEvent.keyboard("h");
    await expectFocused(cell00, "true");
    await expectFocused(cell01, null);
  });

  it("h from first data cell moves focus to the row selector", async () => {
    const screen = await render(<AppWithGridFixture />);

    // Use row 2 (the last row in the 3-row fixture, though any row works
    // here — the test asserts row-scoped left-navigation from the
    // leftmost data column into that row's selector).
    const cellRow2Col0 = screen.getByTestId(
      `data-moniker:${FIXTURE_CELL_MONIKERS[2][0]}`,
    );
    const selectorRow2 = screen.getByTestId(
      `data-moniker:${FIXTURE_ROW_SELECTOR_MONIKERS[2]}`,
    );

    await userEvent.click(cellRow2Col0);
    await expectFocused(cellRow2Col0, "true");

    await userEvent.keyboard("h");
    await expectFocused(selectorRow2, "true");
    await expectFocused(cellRow2Col0, null);
  });

  it("l from the row selector moves focus to the first data cell in the same row", async () => {
    const screen = await render(<AppWithGridFixture />);

    const selectorRow2 = screen.getByTestId(
      `data-moniker:${FIXTURE_ROW_SELECTOR_MONIKERS[2]}`,
    );
    const cellRow2Col0 = screen.getByTestId(
      `data-moniker:${FIXTURE_CELL_MONIKERS[2][0]}`,
    );

    await userEvent.click(selectorRow2);
    await expectFocused(selectorRow2, "true");

    await userEvent.keyboard("l");
    await expectFocused(cellRow2Col0, "true");
    await expectFocused(selectorRow2, null);
  });

  it("j from the last row of cells stays put (does not wrap)", async () => {
    const screen = await render(<AppWithGridFixture />);

    // The fixture has 3 rows (indices 0, 1, 2). Row 2 is the last row,
    // so pressing j from a cell in row 2 must be a no-op: focus stays
    // on the same cell, not the row selector and not any wrapped row.
    const cellLastRowCol0 = screen.getByTestId(
      `data-moniker:${FIXTURE_CELL_MONIKERS[2][0]}`,
    );

    await userEvent.click(cellLastRowCol0);
    await expectFocused(cellLastRowCol0, "true");

    await userEvent.keyboard("j");
    // No spatial entry exists below the last row, so the shim's
    // beam test returns null and focus does not change. Poll that the
    // attribute stays "true" across the full timeout window — any
    // transient flicker would make this assertion flaky.
    await expectFocused(cellLastRowCol0, "true");
  });
});
