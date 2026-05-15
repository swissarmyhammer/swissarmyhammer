import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, fireEvent, act } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Tauri API mocks — must come before importing the component under test.
// ---------------------------------------------------------------------------

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve()),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

vi.mock("@/lib/command-scope", () => ({
  useDispatchCommand: () => vi.fn(() => Promise.resolve()),
}));

import { SlidePanel } from "./slide-panel";

/**
 * Force `window.innerWidth` to a known value so the upper-bound clamp
 * (`min(800, 0.85 * viewport)`) is deterministic. Use 1600 px so the
 * upper bound stays at 800 (the absolute cap), not pinned by the
 * 0.85 × viewport rule.
 */
function setViewportWidth(px: number) {
  Object.defineProperty(window, "innerWidth", {
    configurable: true,
    writable: true,
    value: px,
  });
}

/** Simulate a left-edge drag from `startX` to `endX` and release. */
function dragHandle(handle: HTMLElement, startX: number, endX: number) {
  fireEvent.mouseDown(handle, { clientX: startX });
  fireEvent.mouseMove(window, { clientX: endX });
  fireEvent.mouseUp(window, { clientX: endX });
}

describe("SlidePanel resize bounds", () => {
  beforeEach(() => {
    setViewportWidth(1600);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("clamps a drag that would shrink below 320 px to 320 px", () => {
    const onResize = vi.fn();
    const onResizeEnd = vi.fn();

    const { container } = render(
      <SlidePanel
        open={true}
        onClose={() => {}}
        width={420}
        onResize={onResize}
        onResizeEnd={onResizeEnd}
      >
        Body
      </SlidePanel>,
    );

    const handle = container.querySelector(
      "[data-inspector-resize-handle]",
    ) as HTMLElement;
    expect(handle).not.toBeNull();

    // Panel starts at width 420 with right edge at viewport's right.
    // Dragging the LEFT edge to the right (positive deltaX) shrinks
    // the panel. Drag from x=1180 (panel.left = innerWidth - width)
    // to x=1500 — that would give a negative or sub-320 width if not
    // clamped.
    act(() => {
      dragHandle(handle, 1180, 1500);
    });

    // Final emitted width must be the lower clamp (320), not whatever
    // the raw delta computes to.
    expect(onResizeEnd).toHaveBeenCalledTimes(1);
    expect(onResizeEnd).toHaveBeenCalledWith(320);

    // Every onResize call along the way must also stay >= 320.
    for (const call of onResize.mock.calls) {
      expect(call[0]).toBeGreaterThanOrEqual(320);
    }
  });

  it("clamps a drag that would exceed min(800, 0.85*viewport) to that bound", () => {
    setViewportWidth(1600); // upper bound = min(800, 0.85*1600=1360) = 800
    const onResize = vi.fn();
    const onResizeEnd = vi.fn();

    const { container } = render(
      <SlidePanel
        open={true}
        onClose={() => {}}
        width={420}
        onResize={onResize}
        onResizeEnd={onResizeEnd}
      >
        Body
      </SlidePanel>,
    );

    const handle = container.querySelector(
      "[data-inspector-resize-handle]",
    ) as HTMLElement;

    // Drag the LEFT edge far to the LEFT (negative deltaX) to grow the
    // panel beyond the upper bound.
    act(() => {
      dragHandle(handle, 1180, 0);
    });

    expect(onResizeEnd).toHaveBeenCalledTimes(1);
    expect(onResizeEnd).toHaveBeenCalledWith(800);

    for (const call of onResize.mock.calls) {
      expect(call[0]).toBeLessThanOrEqual(800);
    }
  });

  it("clamps to 0.85 * viewport when that is below the 800 absolute cap", () => {
    // viewport 600 px → upper bound = min(800, 0.85*600=510) = 510.
    setViewportWidth(600);
    const onResize = vi.fn();
    const onResizeEnd = vi.fn();

    const { container } = render(
      <SlidePanel
        open={true}
        onClose={() => {}}
        width={420}
        onResize={onResize}
        onResizeEnd={onResizeEnd}
      >
        Body
      </SlidePanel>,
    );

    const handle = container.querySelector(
      "[data-inspector-resize-handle]",
    ) as HTMLElement;

    // Drag the LEFT edge far to the LEFT to try to grow to 700 px.
    act(() => {
      dragHandle(handle, 180, -200);
    });

    expect(onResizeEnd).toHaveBeenCalledTimes(1);
    expect(onResizeEnd).toHaveBeenCalledWith(510);
  });

  it("does not call onResizeEnd for a tap with no mousemove", () => {
    // A mousedown → mouseup with zero intervening mousemove (or movement
    // that never crosses a clamp boundary) is a tap, not a drag. It must
    // NOT fire `onResizeEnd`, because doing so would dispatch
    // `ui.inspector.set_width { width: 420 }` and flip the persisted
    // `inspector_width` from `None` to `Some(420)` for a no-op
    // interaction. Regression guard for the 2026-05-09 review finding.
    const onResize = vi.fn();
    const onResizeEnd = vi.fn();

    const { container } = render(
      <SlidePanel
        open={true}
        onClose={() => {}}
        width={420}
        onResize={onResize}
        onResizeEnd={onResizeEnd}
      >
        Body
      </SlidePanel>,
    );

    const handle = container.querySelector(
      "[data-inspector-resize-handle]",
    ) as HTMLElement;

    act(() => {
      fireEvent.mouseDown(handle, { clientX: 1180 });
      // No mousemove between down and up → tap, not drag.
      fireEvent.mouseUp(window, { clientX: 1180 });
    });

    expect(onResizeEnd).not.toHaveBeenCalled();
    expect(onResize).not.toHaveBeenCalled();
  });

  it("does not call onResizeEnd when the drag stays within the start width after clamping", () => {
    // Even if the user moves the pointer, a movement that is fully
    // absorbed by clamping (the panel was already at the floor and the
    // user dragged further into the floor) yields a clamped width equal
    // to startWidth. That is functionally a tap — no visible width
    // change — and must not persist.
    setViewportWidth(1600);
    const onResizeEnd = vi.fn();

    const { container } = render(
      <SlidePanel
        open={true}
        onClose={() => {}}
        width={320}
        onResize={() => {}}
        onResizeEnd={onResizeEnd}
      >
        Body
      </SlidePanel>,
    );

    const handle = container.querySelector(
      "[data-inspector-resize-handle]",
    ) as HTMLElement;

    act(() => {
      // Already at the 320 floor; dragging right (shrinking) gets
      // clamped back to 320 every step.
      fireEvent.mouseDown(handle, { clientX: 1280 });
      fireEvent.mouseMove(window, { clientX: 1400 });
      fireEvent.mouseUp(window, { clientX: 1400 });
    });

    expect(onResizeEnd).not.toHaveBeenCalled();
  });

  it("removes window listeners after the drag ends", () => {
    // The listeners installed at mousedown must come back off at
    // mouseup so an idle, mounted panel does not respond to global
    // mousemove. Regression guard for the "always-on listeners"
    // finding from the 2026-05-09 review — `onResize` should fire
    // during the active drag but stop firing after release.
    const onResize = vi.fn();
    const onResizeEnd = vi.fn();

    const { container } = render(
      <SlidePanel
        open={true}
        onClose={() => {}}
        width={420}
        onResize={onResize}
        onResizeEnd={onResizeEnd}
      >
        Body
      </SlidePanel>,
    );

    const handle = container.querySelector(
      "[data-inspector-resize-handle]",
    ) as HTMLElement;

    act(() => {
      fireEvent.mouseDown(handle, { clientX: 1180 });
      fireEvent.mouseMove(window, { clientX: 1100 });
      fireEvent.mouseUp(window, { clientX: 1100 });
    });

    expect(onResizeEnd).toHaveBeenCalledTimes(1);
    const callsDuringDrag = onResize.mock.calls.length;
    expect(callsDuringDrag).toBeGreaterThan(0);

    // After release, a fresh mousemove on `window` must NOT invoke
    // `onResize` again — the listener was torn down in `endDrag`.
    act(() => {
      fireEvent.mouseMove(window, { clientX: 800 });
    });
    expect(onResize.mock.calls.length).toBe(callsDuringDrag);
  });

  it("renders the resize handle with cursor-col-resize", () => {
    const { container } = render(
      <SlidePanel open={true} onClose={() => {}} width={420}>
        Body
      </SlidePanel>,
    );
    const handle = container.querySelector(
      "[data-inspector-resize-handle]",
    ) as HTMLElement;
    expect(handle).not.toBeNull();
    expect(handle.className).toContain("cursor-col-resize");
  });
});
