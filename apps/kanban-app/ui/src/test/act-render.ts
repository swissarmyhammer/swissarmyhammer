/**
 * Test-only helpers that wrap `@testing-library/react`'s `render()` and
 * keyboard events inside `act()` so React's post-mount state updates
 * settle inside the test's act scope instead of flushing later — which
 * is what surfaces as the "An update to X inside a test was not wrapped
 * in act(...)" warnings on stderr.
 *
 * Most call sites in this repository mount provider stacks (UIStateProvider,
 * SchemaProvider, FocusScope, Tooltip, etc.) whose `useEffect` callbacks
 * fire setState after the first paint. When `render()` runs outside an
 * `act` boundary, those updates land asynchronously and React's
 * dev-mode warning fires. Wrapping the render call in `act(async () =>
 * { ... })` keeps the work inside the boundary.
 */

import { act, render, type RenderOptions } from "@testing-library/react";
import { userEvent } from "vitest/browser";
import type { ReactElement } from "react";

/**
 * Render a React tree inside an `act()` boundary so post-mount effects
 * flush before the helper returns. Mirrors `render()`'s signature —
 * returns the same `RenderResult` shape so callers can destructure
 * `{ container, unmount, rerender, ... }` from it.
 *
 * Usage:
 *   const { container, unmount } = await renderInAct(<Tree />);
 *
 * The returned `RenderResult.rerender` is NOT wrapped — call
 * `rerenderInAct(rerender, <Tree />)` (defined below) when you need to
 * trigger updates from inside an act scope as well.
 */
export async function renderInAct(
  ui: ReactElement,
  options?: RenderOptions,
): Promise<ReturnType<typeof render>> {
  let result!: ReturnType<typeof render>;
  await act(async () => {
    result = render(ui, options);
  });
  return result;
}

/**
 * Re-render the tree inside an act boundary. Use when a test mutates
 * state via the parent's render output (e.g. changing props for a
 * controlled component) and the resulting effects must settle before
 * the next assertion.
 */
export async function rerenderInAct(
  rerender: ReturnType<typeof render>["rerender"],
  ui: ReactElement,
): Promise<void> {
  await act(async () => {
    rerender(ui);
  });
}

/**
 * Press a key (vitest/browser `userEvent.keyboard` syntax) inside an
 * `act()` boundary. The keyboard event itself fires synchronously, but
 * downstream listeners (Tooltip open/close, FocusScope re-render,
 * KeybindingHandler state) schedule async setState. Wrapping the call
 * lets the test settle those updates inside scope.
 *
 * Optional `settleMs` argument adds a `setTimeout` delay inside the same
 * act boundary so kernel `focus-changed` round-trips and other
 * post-input async cascades have time to commit. Defaults to 0 — pass
 * a non-zero value when the test relies on cross-process IPC echo.
 */
export async function pressKeyInAct(key: string, settleMs = 0): Promise<void> {
  await act(async () => {
    await userEvent.keyboard(key);
    if (settleMs > 0) {
      await new Promise<void>((resolve) => setTimeout(resolve, settleMs));
    }
  });
}

/**
 * Hover over an element inside an `act()` boundary. Radix tooltips
 * open via async effects in response to pointer enter; wrapping the
 * hover call so the tooltip's `open` state setState fires inside the
 * boundary suppresses the "update to Tooltip not wrapped in act"
 * warning.
 */
export async function hoverInAct(
  element: Element,
  settleMs = 0,
): Promise<void> {
  await act(async () => {
    await userEvent.hover(element);
    if (settleMs > 0) {
      await new Promise<void>((resolve) => setTimeout(resolve, settleMs));
    }
  });
}

/**
 * Click an element inside an `act()` boundary. Production click
 * handlers commonly dispatch IPC + update React state in the same
 * event; the state updates flush after the synthetic click returns,
 * so wrap the call to keep them in scope.
 */
export async function clickInAct(
  element: Element,
  settleMs = 0,
): Promise<void> {
  await act(async () => {
    await userEvent.click(element);
    if (settleMs > 0) {
      await new Promise<void>((resolve) => setTimeout(resolve, settleMs));
    }
  });
}

/**
 * Flush pending React effects + a configurable settle window inside an
 * `act()` boundary. The common pattern in this repo's spatial-nav
 * tests is `render(); await flushSetup();` — replace `flushSetup` with
 * `await flushActSettle(80)` for the canonical 80ms settle.
 */
export async function flushActSettle(settleMs = 0): Promise<void> {
  await act(async () => {
    if (settleMs > 0) {
      await new Promise<void>((resolve) => setTimeout(resolve, settleMs));
    } else {
      await Promise.resolve();
    }
  });
}
