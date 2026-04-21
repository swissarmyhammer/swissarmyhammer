/**
 * Card interior spatial navigation tests.
 *
 * Pins down the contract for h/j/k/l navigation into and within a
 * card's sub-parts (tag pills, assignees, titles, fields). The
 * scenarios covered:
 *
 * - `l` from a focused card enters its first tag pill (beam-test hit
 *   because the card's body rect lies to the left of the pill row).
 * - `h` from the first tag pill falls back through to the card's body
 *   (no sibling pill to the left; full-layer fallback picks the body).
 * - `j` from a focused pill falls through to the card body directly
 *   below (container-first finds no sibling pill in the Down direction
 *   because all siblings sit in the same pill row, so the full-layer
 *   beam test picks the nearest card body below — never a pill on a
 *   different card).
 * - `l` steps through sibling pills on the same card (container-first
 *   picks the next pill before any full-layer candidate).
 *
 * ## Harness
 *
 * Uses the shared `SpatialStateShim` via `setupSpatialShim()` — the
 * same pattern as `spatial-nav-board.test.tsx`. The fixture places
 * two tag pills to the right of `card-1-1`'s body; other cards
 * render with the default (no-pill) shape. Pills register their
 * enclosing card moniker as `parent_scope` through
 * `FocusScopeContext`, which is the wiring under test.
 *
 * ## Why these tests matter
 *
 * Before this task, `FocusScope` always passed `parent_scope: null`
 * on `spatial_register`, which defeated the Rust engine's
 * container-first search for card sub-parts — there was no way to
 * step from a card into its tag pills or back. Locking the contract
 * into automated tests guards against that regression if a future
 * refactor reintroduces the `null`-only wiring.
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
vi.mock("@tauri-apps/api/webviewWindow", async () => {
  const { tauriWebviewWindowMock } = await import("./setup-spatial-shim");
  return tauriWebviewWindowMock();
});
vi.mock("@tauri-apps/api/window", async () => {
  const { tauriWindowMock } = await import("./setup-spatial-shim");
  return tauriWindowMock();
});
vi.mock("@tauri-apps/plugin-log", async () => {
  const { tauriPluginLogMock } = await import("./setup-spatial-shim");
  return tauriPluginLogMock();
});

import { moniker } from "@/lib/moniker";
import {
  setupSpatialShim,
  type SpatialShimHandles,
} from "./setup-spatial-shim";
import {
  AppWithBoardFixture,
  FIXTURE_CARD_MONIKERS,
  fixtureTagId,
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

/** Build the moniker for the Nth tag pill on the (col, row) card. */
function tagMoniker(col: number, row: number, idx: number): string {
  return moniker("tag", fixtureTagId(col, row, idx));
}

/** Fixture shim handles — fresh per test, captured by `beforeEach`. */
let handles: SpatialShimHandles;

describe("card interior navigation", () => {
  beforeEach(() => {
    handles = setupSpatialShim();
  });

  it("l from a focused card moves focus to the first tag pill on that card", async () => {
    const screen = await render(<AppWithBoardFixture />);
    const cardBody = screen
      .getByTestId(`data-moniker:${FIXTURE_CARD_MONIKERS[0][0]}`)
      .element() as HTMLElement;
    const pill1 = screen
      .getByTestId(`data-moniker:${tagMoniker(1, 1, 1)}`)
      .element() as HTMLElement;

    await userEvent.click(cardBody);
    await expectFocused(cardBody);

    await userEvent.keyboard("l");
    await expectFocused(pill1);
  });

  it("h from a pill moves back to the card body", async () => {
    const screen = await render(<AppWithBoardFixture />);
    const cardBody = screen
      .getByTestId(`data-moniker:${FIXTURE_CARD_MONIKERS[0][0]}`)
      .element() as HTMLElement;
    const pill1 = screen
      .getByTestId(`data-moniker:${tagMoniker(1, 1, 1)}`)
      .element() as HTMLElement;

    await userEvent.click(pill1);
    await expectFocused(pill1);

    await userEvent.keyboard("h");
    await expectFocused(cardBody);
  });

  it("j from a focused pill falls through to the card body directly below", async () => {
    // `j` from pill1 on card-1-1 must never land on a pill belonging to
    // another card — the container-first search keeps sibling pills (pill2,
    // to the right) from winning the Down direction, and the full-layer
    // fallback only reaches other cards' bodies, never their pills.
    //
    // Geometry:
    //   - pill1 sits to the right of card-1-1's narrow body at row 1.
    //   - card-1-2 is a full-column-width body directly below pill1 in
    //     the Down beam.
    //   - Other pills live only on card-1-1, so no non-sibling pill is in
    //     any beam.
    //
    // Expected concrete outcome: focus moves to card-1-2's body — the
    // nearest full-layer candidate in the Down direction after
    // container-first finds no sibling pill below.
    const screen = await render(<AppWithBoardFixture />);
    const pill1 = screen
      .getByTestId(`data-moniker:${tagMoniker(1, 1, 1)}`)
      .element() as HTMLElement;
    const cardBelowMoniker = FIXTURE_CARD_MONIKERS[0][1];
    const cardBelow = screen
      .getByTestId(`data-moniker:${cardBelowMoniker}`)
      .element() as HTMLElement;

    await userEvent.click(pill1);
    await expectFocused(pill1);

    await userEvent.keyboard("j");
    await expectFocused(cardBelow);

    // Explicit discriminator: focus must not be on any other pill — the
    // container-first contract forbids cross-card pill jumps even when
    // the geometry happens to place them in the beam.
    expect(handles.focusedMoniker()).toBe(cardBelowMoniker);
  });

  it("l through sibling pills on the same card", async () => {
    const screen = await render(<AppWithBoardFixture />);
    const pill1 = screen
      .getByTestId(`data-moniker:${tagMoniker(1, 1, 1)}`)
      .element() as HTMLElement;
    const pill2 = screen
      .getByTestId(`data-moniker:${tagMoniker(1, 1, 2)}`)
      .element() as HTMLElement;

    await userEvent.click(pill1);
    await expectFocused(pill1);

    await userEvent.keyboard("l");
    await expectFocused(pill2);
  });

  it("pill spatial entries carry the enclosing card moniker as parent_scope", async () => {
    // Directly asserts the wiring this task is about: `FocusScope`
    // must thread the nearest ancestor moniker through to
    // `spatial_register`'s `parent_scope` argument. The geometric
    // tests above pass even without this (pills happen to be closer
    // than non-siblings in the beam test), so this test is the only
    // one that fails when the implementation regresses to
    // `parent_scope: null`.
    await render(<AppWithBoardFixture />);

    const cardMoniker = FIXTURE_CARD_MONIKERS[0][0];
    const pill1Moniker = tagMoniker(1, 1, 1);
    const pill2Moniker = tagMoniker(1, 1, 2);

    // Poll: the ResizeObserver in `FocusScope` flushes a
    // `spatial_register` call after mount; wait until both pills
    // are registered before asserting.
    await expect
      .poll(
        () =>
          handles.shim
            .entriesSnapshot()
            .filter(
              (e) => e.moniker === pill1Moniker || e.moniker === pill2Moniker,
            ).length,
        { timeout: FOCUS_POLL_TIMEOUT },
      )
      .toBe(2);

    const pillEntries = handles.shim
      .entriesSnapshot()
      .filter((e) => e.moniker === pill1Moniker || e.moniker === pill2Moniker);

    for (const entry of pillEntries) {
      expect(entry.parentScope).toBe(cardMoniker);
    }
  });
});
