import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useDebouncedSave } from "./use-debounced-save";

describe("useDebouncedSave", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  const makeOpts = (overrides?: Record<string, unknown>) => ({
    updateField: vi.fn().mockResolvedValue(undefined),
    entityType: "task",
    entityId: "t1",
    fieldName: "title",
    delayMs: 500,
    ...overrides,
  });

  it("fires updateField after the debounce delay", () => {
    const opts = makeOpts();
    const { result } = renderHook(() => useDebouncedSave(opts));

    act(() => {
      result.current.onChange("hello");
    });

    // Not called yet
    expect(opts.updateField).not.toHaveBeenCalled();

    // Advance past the delay
    act(() => {
      vi.advanceTimersByTime(500);
    });

    expect(opts.updateField).toHaveBeenCalledOnce();
    expect(opts.updateField).toHaveBeenCalledWith(
      "task",
      "t1",
      "title",
      "hello",
    );
  });

  it("restarts the timer on subsequent onChange calls", () => {
    const opts = makeOpts();
    const { result } = renderHook(() => useDebouncedSave(opts));

    act(() => {
      result.current.onChange("h");
    });
    act(() => {
      vi.advanceTimersByTime(300);
    });
    act(() => {
      result.current.onChange("he");
    });
    act(() => {
      vi.advanceTimersByTime(300);
    });

    // Only 300ms since last onChange — should not have fired
    expect(opts.updateField).not.toHaveBeenCalled();

    act(() => {
      vi.advanceTimersByTime(200);
    });

    // 500ms since last onChange — should fire with latest value
    expect(opts.updateField).toHaveBeenCalledOnce();
    expect(opts.updateField).toHaveBeenCalledWith("task", "t1", "title", "he");
  });

  it("flush fires the pending save immediately", () => {
    const opts = makeOpts();
    const { result } = renderHook(() => useDebouncedSave(opts));

    act(() => {
      result.current.onChange("flushed");
    });

    expect(opts.updateField).not.toHaveBeenCalled();

    act(() => {
      result.current.flush();
    });

    expect(opts.updateField).toHaveBeenCalledOnce();
    expect(opts.updateField).toHaveBeenCalledWith(
      "task",
      "t1",
      "title",
      "flushed",
    );
  });

  it("flush is a no-op when nothing is pending", () => {
    const opts = makeOpts();
    const { result } = renderHook(() => useDebouncedSave(opts));

    act(() => {
      result.current.flush();
    });

    expect(opts.updateField).not.toHaveBeenCalled();
  });

  it("flush prevents the timer from firing again", () => {
    const opts = makeOpts();
    const { result } = renderHook(() => useDebouncedSave(opts));

    act(() => {
      result.current.onChange("value");
    });
    act(() => {
      result.current.flush();
    });

    // Advance past original delay — should not fire a second time
    act(() => {
      vi.advanceTimersByTime(1000);
    });

    expect(opts.updateField).toHaveBeenCalledOnce();
  });

  it("cancel discards the pending save without firing", () => {
    const opts = makeOpts();
    const { result } = renderHook(() => useDebouncedSave(opts));

    act(() => {
      result.current.onChange("discarded");
    });
    act(() => {
      result.current.cancel();
    });

    // Advance past delay — should not fire
    act(() => {
      vi.advanceTimersByTime(1000);
    });

    expect(opts.updateField).not.toHaveBeenCalled();
  });

  it("cleans up the timer on unmount", () => {
    const opts = makeOpts();
    const { result, unmount } = renderHook(() => useDebouncedSave(opts));

    act(() => {
      result.current.onChange("will-unmount");
    });

    unmount();

    // Advance past delay — should not fire (timer cleared on unmount)
    act(() => {
      vi.advanceTimersByTime(1000);
    });

    expect(opts.updateField).not.toHaveBeenCalled();
  });

  it("defaults delayMs to 1000ms", () => {
    const opts = makeOpts();
    delete (opts as Record<string, unknown>).delayMs;
    const { result } = renderHook(() => useDebouncedSave(opts));

    act(() => {
      result.current.onChange("default-delay");
    });

    act(() => {
      vi.advanceTimersByTime(999);
    });
    expect(opts.updateField).not.toHaveBeenCalled();

    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(opts.updateField).toHaveBeenCalledOnce();
  });
});
