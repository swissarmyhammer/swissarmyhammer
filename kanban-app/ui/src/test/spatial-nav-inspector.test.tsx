/**
 * Regression-proof coverage for inspector-layer j/k/h/l navigation.
 *
 * The inspector lives on its own `FocusLayer` pushed on top of the
 * window layer. While open, spatial nav should be trapped: `j`/`k`
 * move between field rows, and `h`/`l` are no-ops (no sub-field
 * columns in this fixture); `k` at the top field and `j` at the last
 * field clamp instead of wrapping; Escape closes the layer and
 * restores focus to the card the inspector opened from.
 *
 * These tests codify the contract so that future edits to the
 * inspector, focus layer, or broadcast nav path fail fast instead of
 * silently breaking inspector navigation. Historically this was a
 * tacit invariant — confirmed working by manual testing and then
 * broken by an unrelated edit with no failing test. This file closes
 * that gap.
 *
 * ## Infrastructure
 *
 * Uses the shared shim harness (`spatial-shim.ts` +
 * `setup-spatial-shim.ts`) that `spatial-nav-canonical.test.tsx` also
 * uses, plus a dedicated `spatial-inspector-fixture.tsx` tailored to
 * the "window + inspector" layer stack.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { userEvent } from "vitest/browser";
import { render } from "vitest-browser-react";

// Wire the Tauri API mocks into the shim dispatcher. The `vi.mock`
// literals must appear in this file so vitest's hoist catches them;
// the factories live in `setup-spatial-shim` to keep boilerplate DRY.
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
  AppWithInspectorFixture,
  FIXTURE_CARD_MONIKER,
  FIXTURE_FIELD_MONIKERS,
  FIXTURE_FIELD_NAMES,
} from "./spatial-inspector-fixture";

/** Polling timeout for focus/layer transitions — same budget as the canonical test. */
const POLL_TIMEOUT = 500;

/**
 * Wait for the shim's focused moniker to match the predicate.
 *
 * Polls at the default vitest `expect.poll` interval until the focused
 * moniker satisfies `predicate` or the timeout elapses. Returns the
 * moniker observed on the winning poll.
 */
async function waitForFocusedMoniker(
  handles: SpatialShimHandles,
  predicate: (m: string | null) => boolean,
): Promise<string | null> {
  await expect
    .poll(() => predicate(handles.focusedMoniker()), { timeout: POLL_TIMEOUT })
    .toBe(true);
  return handles.focusedMoniker();
}

/** Wait for the shim's layer stack to have exactly `n` entries. */
async function waitForLayerCount(
  handles: SpatialShimHandles,
  n: number,
): Promise<void> {
  await expect
    .poll(() => handles.shim.layerCount(), { timeout: POLL_TIMEOUT })
    .toBe(n);
}

describe("inspector field navigation", () => {
  let handles: SpatialShimHandles;

  beforeEach(() => {
    handles = setupSpatialShim();
  });

  /**
   * Common prelude: render the fixture, click the card to focus it,
   * double-click to open the inspector, and wait for the first field
   * to claim focus.
   *
   * Every test starts from the same "inspector open, first field
   * focused, two layers on the stack" state, so assertions target the
   * specific transition under test without repeating the setup steps.
   */
  async function openInspector() {
    const screen = await render(<AppWithInspectorFixture />);
    const card = screen.getByTestId("fixture-card");

    await userEvent.click(card);
    await expect
      .poll(() => handles.focusedMoniker(), { timeout: POLL_TIMEOUT })
      .toBe(FIXTURE_CARD_MONIKER);

    await userEvent.dblClick(card);

    // Inspector mounts → window + inspector layers exist.
    await waitForLayerCount(handles, 2);
    // First-field mount effect focuses the first field.
    await waitForFocusedMoniker(
      handles,
      (m) => m === FIXTURE_FIELD_MONIKERS[0],
    );
  }

  it("j moves focus from the first field to the second field", async () => {
    await openInspector();

    const first = handles.focusedMoniker();
    expect(first).toBe(FIXTURE_FIELD_MONIKERS[0]);

    await userEvent.keyboard("j");

    const second = await waitForFocusedMoniker(
      handles,
      (m) => m !== null && m !== first,
    );
    expect(second).toBe(FIXTURE_FIELD_MONIKERS[1]);
  });

  it("j at the last field clamps (does not wrap)", async () => {
    await openInspector();

    // Press j (N - 1) times to reach the last field.
    for (let i = 0; i < FIXTURE_FIELD_NAMES.length - 1; i++) {
      await userEvent.keyboard("j");
    }
    await waitForFocusedMoniker(
      handles,
      (m) => m === FIXTURE_FIELD_MONIKERS[FIXTURE_FIELD_MONIKERS.length - 1],
    );

    // One more j — focus must not wrap to the first field.
    await userEvent.keyboard("j");

    // Give React/event loop a beat to process the no-op, then check.
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(handles.focusedMoniker()).toBe(
      FIXTURE_FIELD_MONIKERS[FIXTURE_FIELD_MONIKERS.length - 1],
    );
  });

  it("k at the first field clamps (does not wrap)", async () => {
    await openInspector();

    // Already on the first field after the prelude.
    expect(handles.focusedMoniker()).toBe(FIXTURE_FIELD_MONIKERS[0]);

    await userEvent.keyboard("k");

    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(handles.focusedMoniker()).toBe(FIXTURE_FIELD_MONIKERS[0]);
  });

  it("Escape closes the inspector and restores focus to the card", async () => {
    await openInspector();

    await userEvent.keyboard("{Escape}");

    // Inspector layer removed → only the window layer remains.
    await waitForLayerCount(handles, 1);
    // Focus restored to the card via window.lastFocused on removeLayer.
    await waitForFocusedMoniker(handles, (m) => m === FIXTURE_CARD_MONIKER);
  });

  it("opening the inspector auto-focuses the first field", async () => {
    // Regression guard for the FocusLayer-push → focus-first-in-layer
    // contract: when a layer mounts, spatial_focus_first_in_layer is
    // scheduled on a requestAnimationFrame, and the first (upper-left)
    // registered entry in the new layer must claim focus without any
    // keystroke. Before this wiring, `focused_key` stayed on the card
    // after the inspector opened, so `j` was a no-op in `navigate()`
    // (source key in window layer, filtered out by active-layer scope).
    const screen = await render(<AppWithInspectorFixture />);
    const card = screen.getByTestId("fixture-card");

    // Focus the card (window layer), then open the inspector.
    await userEvent.click(card);
    await expect
      .poll(() => handles.focusedMoniker(), { timeout: POLL_TIMEOUT })
      .toBe(FIXTURE_CARD_MONIKER);

    await userEvent.dblClick(card);

    // Wait for the inspector layer to be on the stack.
    await waitForLayerCount(handles, 2);

    // Assert the first field is focused — no keystroke, no extra
    // interaction. The DOM assertion mirrors the task's acceptance
    // criterion: `data-focused="true"` lands on the first field's
    // FocusScope element.
    const firstFieldRow = screen.getByTestId(
      `field-row-${FIXTURE_FIELD_NAMES[0]}`,
    );
    await expect
      .poll(() => firstFieldRow.element().getAttribute("data-focused"), {
        timeout: POLL_TIMEOUT,
      })
      .toBe("true");

    // And confirm the shim's focused moniker agrees — a sanity check
    // that the DOM attribute isn't stale.
    expect(handles.focusedMoniker()).toBe(FIXTURE_FIELD_MONIKERS[0]);
  });

  it("inspector nav is trapped — k at the top field does NOT escape to the card", async () => {
    await openInspector();

    // First field is already the top field after the prelude.
    expect(handles.focusedMoniker()).toBe(FIXTURE_FIELD_MONIKERS[0]);

    await userEvent.keyboard("k");

    // Give the event loop time to process the no-op, then assert no
    // leakage: focus stayed on a field moniker, never landed on the
    // card moniker (which lives in the window layer and is filtered
    // out by the shim's active-layer candidate set).
    await new Promise((resolve) => setTimeout(resolve, 50));
    const focused = handles.focusedMoniker();
    expect(focused).not.toBe(FIXTURE_CARD_MONIKER);
    expect(focused).toMatch(/^field:task:card-1-1\./);
  });
});
