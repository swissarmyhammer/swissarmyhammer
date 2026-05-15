/**
 * Regression tests for `useScrollFocusedIntoView` in `board-view.tsx`.
 *
 * Pins kanban task `01KRK6HR174QVN2TAH9AH4XZJB`:
 * the board-level "follow the focus bar" hook must call `scrollIntoView`
 * only when the focused FQM actually changes — not every render where the
 * effect's deps appear to change because of a non-focus prop churn.
 *
 * Test matrix:
 * 1. First non-null `focusedFq`            → 1 call.
 * 2. Same `focusedFq` passed again         → 0 additional calls.
 * 3. Different `focusedFq`                 → 1 additional call.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render } from "@testing-library/react";
import { useRef, useEffect } from "react";

// Tauri API mocks — must come before the board-view import because
// `board-view.tsx` transitively pulls in modules (`views-context`,
// `entity-focus-context`) that touch `@tauri-apps/api/window` and
// `@tauri-apps/api/event` at import time.
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve()),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
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

import type { FullyQualifiedMoniker } from "@/types/spatial";
import { useScrollFocusedIntoView } from "./board-view";

/**
 * Host component that mounts the hook against a deterministic container
 * holding two `[data-moniker]` elements. Re-renders are driven by the
 * test changing the `focusedFq` prop.
 */
function HookHost({ focusedFq }: { focusedFq: FullyQualifiedMoniker | null }) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  // Seed the container ref once on mount with a real DOM element that
  // has the two `[data-moniker]` children the hook queries for.
  const sentinelRef = useRef<HTMLDivElement | null>(null);
  useEffect(() => {
    if (containerRef.current && sentinelRef.current) {
      containerRef.current = sentinelRef.current;
    }
  });
  return (
    <div
      ref={(node) => {
        containerRef.current = node;
        sentinelRef.current = node;
      }}
    >
      <div data-moniker="/window/board/column:a/task:1">A</div>
      <div data-moniker="/window/board/column:b/task:2">B</div>
      <HookConsumer containerRef={containerRef} focusedFq={focusedFq} />
    </div>
  );
}

function HookConsumer({
  containerRef,
  focusedFq,
}: {
  containerRef: React.RefObject<HTMLDivElement | null>;
  focusedFq: FullyQualifiedMoniker | null;
}) {
  useScrollFocusedIntoView(containerRef, focusedFq);
  return null;
}

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

describe("useScrollFocusedIntoView — fires only when focusedFq actually changes", () => {
  let restoreSpy: () => void = () => {};

  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    restoreSpy();
  });

  it("calls scrollIntoView once when focusedFq goes from null → a value", () => {
    const { spy, restore } = spyScrollIntoView();
    restoreSpy = restore;

    const FQ_A = "/window/board/column:a/task:1" as FullyQualifiedMoniker;
    const { rerender, unmount } = render(<HookHost focusedFq={null} />);

    expect(spy).not.toHaveBeenCalled();

    rerender(<HookHost focusedFq={FQ_A} />);
    expect(spy).toHaveBeenCalledTimes(1);
    unmount();
  });

  it("does not call scrollIntoView when focusedFq is re-passed with the same value", () => {
    const { spy, restore } = spyScrollIntoView();
    restoreSpy = restore;

    const FQ_A = "/window/board/column:a/task:1" as FullyQualifiedMoniker;
    const { rerender, unmount } = render(<HookHost focusedFq={FQ_A} />);

    expect(spy).toHaveBeenCalledTimes(1);

    // Re-render with the same focusedFq value. Even if the container
    // ref's identity churns across renders, the focused-FQM has not
    // changed; the hook must not re-scroll.
    rerender(<HookHost focusedFq={FQ_A} />);
    expect(spy).toHaveBeenCalledTimes(1);

    rerender(<HookHost focusedFq={FQ_A} />);
    expect(spy).toHaveBeenCalledTimes(1);
    unmount();
  });

  it("calls scrollIntoView again when focusedFq changes to a different value", () => {
    const { spy, restore } = spyScrollIntoView();
    restoreSpy = restore;

    const FQ_A = "/window/board/column:a/task:1" as FullyQualifiedMoniker;
    const FQ_B = "/window/board/column:b/task:2" as FullyQualifiedMoniker;
    const { rerender, unmount } = render(<HookHost focusedFq={FQ_A} />);

    expect(spy).toHaveBeenCalledTimes(1);

    rerender(<HookHost focusedFq={FQ_B} />);
    expect(spy).toHaveBeenCalledTimes(2);
    unmount();
  });
});
