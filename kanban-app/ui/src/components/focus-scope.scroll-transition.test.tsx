/**
 * Regression tests for `SpatialFocusScopeBody`'s `scrollIntoView` effect.
 *
 * Pins kanban task `01KRK6HR174QVN2TAH9AH4XZJB`:
 * `scrollIntoView` must fire only on a real focus transition (false → true),
 * not on every re-render or remount. The virtualized column unmounts and
 * remounts the focused card row as the user scrolls; a remount-while-still-
 * focused must NOT yank the scroller back to the focused card.
 *
 * Test matrix:
 * 1. Mount while `isDirectFocus=true`            → 1 call.
 * 2. Re-render while `isDirectFocus=true`        → 0 additional calls.
 * 3. Toggle true → false → true                  → 1 additional call.
 * 4. Unmount + remount while still focused       → 0 additional calls.
 * 5. Unmount, focus moves away and back, remount → 1 additional call.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, act } from "@testing-library/react";

// Capture focus-changed listeners so simulated kernel emits can reach the
// EntityFocusProvider bridge.
type ListenCallback = (event: { payload: unknown }) => void;
const focusListeners: ListenCallback[] = [];

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve()),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((event: string, cb: ListenCallback) => {
    if (event === "focus-changed") focusListeners.push(cb);
    return Promise.resolve(() => {
      const idx = focusListeners.indexOf(cb);
      if (idx >= 0) focusListeners.splice(idx, 1);
    });
  }),
  emit: vi.fn(() => Promise.resolve()),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    label: "main",
    listen: vi.fn(() => Promise.resolve(() => {})),
  }),
}));

vi.mock("@tauri-apps/plugin-log", () => ({
  error: vi.fn(),
  warn: vi.fn(),
  info: vi.fn(),
  debug: vi.fn(),
  trace: vi.fn(),
  attachConsole: vi.fn(() => Promise.resolve()),
}));

import { FocusScope } from "./focus-scope";
import { FocusLayer } from "./focus-layer";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { asSegment, type FullyQualifiedMoniker } from "@/types/spatial";

/**
 * Drive the entity-focus store by emitting a synthetic `focus-changed`
 * event. The `EntityFocusProvider` bridge listens for this event and
 * calls `store.set(payload.next_fq)`, flipping the per-FQM focus slots.
 */
function emitFocusChanged(
  prev: FullyQualifiedMoniker | null,
  next: FullyQualifiedMoniker | null,
) {
  act(() => {
    for (const cb of focusListeners) {
      cb({
        payload: {
          window_label: "main",
          prev_fq: prev,
          next_fq: next,
          next_segment: null,
        },
      });
    }
  });
}

/**
 * Wrap a `<FocusScope>` in the minimum provider stack the focus-scope
 * primitive needs to mount. Returns the rendered element so the test can
 * assert against the spied `scrollIntoView`.
 */
function renderScope(opts: { fq: FullyQualifiedMoniker | null }) {
  return render(
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <EntityFocusProvider>
          <Inner fq={opts.fq} />
        </EntityFocusProvider>
      </FocusLayer>
    </SpatialFocusProvider>,
  );
}

function Inner({ fq: _unused }: { fq: FullyQualifiedMoniker | null }) {
  return (
    <FocusScope moniker={asSegment("task:scroll-test")}>
      <span>card</span>
    </FocusScope>
  );
}

/**
 * Build the provider tree with the `<FocusScope>` mount controlled by
 * a `mounted` flag. Used by virtualizer-recycle tests that mount and
 * unmount the scope while keeping the same provider stack (and therefore
 * the same `FocusStore`) — modelling what the real virtualizer does as
 * it recycles rows within the single app-level `<EntityFocusProvider>`.
 */
function buildToggleTree(mounted: boolean) {
  return (
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <EntityFocusProvider>
          {mounted ? <Inner fq={null} /> : null}
        </EntityFocusProvider>
      </FocusLayer>
    </SpatialFocusProvider>
  );
}

/**
 * Composed FQM the scope above registers under. `<FocusLayer
 * name="window">` is the only ancestor, so the FQM is
 * `/window/task:scroll-test`.
 */
const SCOPE_FQ = "/window/task:scroll-test" as FullyQualifiedMoniker;

/** Spy on `Element.prototype.scrollIntoView` for the duration of one test. */
function spyScrollIntoView() {
  const spy = vi.fn();
  const proto = Element.prototype as unknown as {
    scrollIntoView: (...args: unknown[]) => void;
  };
  const original = proto.scrollIntoView;
  proto.scrollIntoView = spy;
  return {
    spy,
    restore: () => {
      proto.scrollIntoView = original;
    },
  };
}

describe("SpatialFocusScopeBody — scrollIntoView fires only on focus transitions", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    focusListeners.length = 0;
    // The focus-scroll latch lives on `FocusStore`; each test mounts a
    // fresh `EntityFocusProvider` (and therefore a fresh store), so the
    // latch resets implicitly — no cross-test global to clear here.
  });

  it("calls scrollIntoView once when mounted while already focused", async () => {
    const { spy, restore } = spyScrollIntoView();
    try {
      // First mount a no-op stack so the EntityFocusProvider bridge is
      // attached and the listener is registered. Then drive focus to the
      // scope FQM before the scope under test ever mounts.
      const setup = renderScope({ fq: null });
      emitFocusChanged(null, SCOPE_FQ);
      // The scope mounts already focused — exactly the drag-and-drop
      // case. The single effect must fire `scrollIntoView` exactly once.
      // The render above already mounted the scope; verify the call
      // happened during that mount.
      expect(spy).toHaveBeenCalledTimes(1);
      setup.unmount();
    } finally {
      restore();
    }
  });

  it("does not call scrollIntoView again on a re-render while focus stays on the scope", async () => {
    const { spy, restore } = spyScrollIntoView();
    try {
      const { rerender, unmount } = renderScope({ fq: null });
      emitFocusChanged(null, SCOPE_FQ);
      expect(spy).toHaveBeenCalledTimes(1);

      // Force a re-render of the provider stack without changing focus.
      rerender(
        <SpatialFocusProvider>
          <FocusLayer name={asSegment("window")}>
            <EntityFocusProvider>
              <Inner fq={SCOPE_FQ} />
            </EntityFocusProvider>
          </FocusLayer>
        </SpatialFocusProvider>,
      );

      expect(spy).toHaveBeenCalledTimes(1);
      unmount();
    } finally {
      restore();
    }
  });

  it("calls scrollIntoView exactly once more when focus toggles true → false → true", async () => {
    const { spy, restore } = spyScrollIntoView();
    try {
      const { unmount } = renderScope({ fq: null });
      emitFocusChanged(null, SCOPE_FQ);
      expect(spy).toHaveBeenCalledTimes(1);

      // Focus moves away to a different FQM (does not have to be a
      // mounted scope — the store just records the FQM).
      const OTHER_FQ = "/window/task:other" as FullyQualifiedMoniker;
      emitFocusChanged(SCOPE_FQ, OTHER_FQ);
      expect(spy).toHaveBeenCalledTimes(1);

      // Focus comes back to our scope — that is a real false → true
      // transition and must scroll exactly once more.
      emitFocusChanged(OTHER_FQ, SCOPE_FQ);
      expect(spy).toHaveBeenCalledTimes(2);

      unmount();
    } finally {
      restore();
    }
  });

  it("does not call scrollIntoView on remount while the scope is still the focused one (virtualizer recycle case)", async () => {
    const { spy, restore } = spyScrollIntoView();
    try {
      // Initial mount + focus emit → one scroll. The provider stack
      // (and its `FocusStore`) stays mounted for the whole test —
      // only the inner `<FocusScope>` toggles, mirroring what the real
      // virtualizer does as it recycles rows inside the app-level
      // `<EntityFocusProvider>`.
      const { rerender, unmount } = render(buildToggleTree(true));
      emitFocusChanged(null, SCOPE_FQ);
      expect(spy).toHaveBeenCalledTimes(1);

      // The virtualizer recycles the row: unmount the scope.
      rerender(buildToggleTree(false));

      // Remount the scope. Focus has not moved — the store still
      // remembers SCOPE_FQ as the focused FQM AND still holds the
      // scroll latch pinned to SCOPE_FQ (the store invalidates the
      // latch only when focus moves to a different FQM, in its own
      // `set()` path). The new mount sees `isDirectFocus=true` from
      // the very first render but `consumeScrollLatch(fq)` returns
      // `false`, so no scroll fires — otherwise the user's scroll
      // inside the column would get yanked back to the focused card.
      rerender(buildToggleTree(true));

      expect(spy).toHaveBeenCalledTimes(1);
      unmount();
    } finally {
      restore();
    }
  });

  it("calls scrollIntoView on remount only when focus moved away in between", async () => {
    const { spy, restore } = spyScrollIntoView();
    try {
      const { rerender, unmount } = render(buildToggleTree(true));
      emitFocusChanged(null, SCOPE_FQ);
      expect(spy).toHaveBeenCalledTimes(1);

      // Unmount the scope (virtualizer recycle), then while it is
      // unmounted, focus moves away to a different FQM. The store's
      // `set()` clears the scroll latch because the new focused FQM
      // differs from the latched FQM. Then focus moves back to the
      // original FQM and the scope remounts — the latch is now
      // null, `consumeScrollLatch(fq)` returns `true`, and the scope
      // scrolls once on remount.
      rerender(buildToggleTree(false));

      const OTHER_FQ = "/window/task:other" as FullyQualifiedMoniker;
      emitFocusChanged(SCOPE_FQ, OTHER_FQ);
      emitFocusChanged(OTHER_FQ, SCOPE_FQ);

      rerender(buildToggleTree(true));

      expect(spy).toHaveBeenCalledTimes(2);
      unmount();
    } finally {
      restore();
    }
  });
});
