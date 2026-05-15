/**
 * Tests for {@link useDebouncedTimer} — a single-slot debounced-callback hook.
 *
 * Covers the three callsite contracts:
 *
 *   - schedule + wait → callback fires after the delay
 *   - schedule + cancel → callback never fires
 *   - schedule + flush → callback fires synchronously, timer cleared
 *   - schedule + reschedule → only the latest callback fires
 *   - unmount with pending → callback flushes (does not silently drop)
 *
 * Uses fake timers so the assertions are deterministic.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useDebouncedTimer } from "./use-debounced-timer";

describe("useDebouncedTimer", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("fires the scheduled callback after the delay", () => {
    const { result } = renderHook(() => useDebouncedTimer());
    const fn = vi.fn();

    act(() => {
      result.current.schedule(fn, 100);
    });

    expect(fn).not.toHaveBeenCalled();
    act(() => {
      vi.advanceTimersByTime(99);
    });
    expect(fn).not.toHaveBeenCalled();
    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(fn).toHaveBeenCalledOnce();
  });

  it("cancel drops the pending callback", () => {
    const { result } = renderHook(() => useDebouncedTimer());
    const fn = vi.fn();

    act(() => {
      result.current.schedule(fn, 100);
      result.current.cancel();
    });

    act(() => {
      vi.advanceTimersByTime(500);
    });
    expect(fn).not.toHaveBeenCalled();
  });

  it("flush invokes the pending callback synchronously and clears the timer", () => {
    const { result } = renderHook(() => useDebouncedTimer());
    const fn = vi.fn();

    act(() => {
      result.current.schedule(fn, 100);
      result.current.flush();
    });

    expect(fn).toHaveBeenCalledOnce();

    // Subsequent timer advance should not double-fire.
    act(() => {
      vi.advanceTimersByTime(500);
    });
    expect(fn).toHaveBeenCalledOnce();
  });

  it("flush is a no-op when nothing is pending", () => {
    const { result } = renderHook(() => useDebouncedTimer());
    expect(() => {
      act(() => {
        result.current.flush();
      });
    }).not.toThrow();
  });

  it("rescheduling replaces the pending callback — only the latest fires", () => {
    const { result } = renderHook(() => useDebouncedTimer());
    const first = vi.fn();
    const second = vi.fn();

    act(() => {
      result.current.schedule(first, 100);
      result.current.schedule(second, 100);
    });

    act(() => {
      vi.advanceTimersByTime(100);
    });

    expect(first).not.toHaveBeenCalled();
    expect(second).toHaveBeenCalledOnce();
  });

  it("flushes the pending callback on unmount instead of silently dropping it", () => {
    const { result, unmount } = renderHook(() => useDebouncedTimer());
    const fn = vi.fn();

    act(() => {
      result.current.schedule(fn, 1000);
    });

    unmount();

    expect(fn).toHaveBeenCalledOnce();
  });
});
