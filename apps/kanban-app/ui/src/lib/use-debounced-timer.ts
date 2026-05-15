/**
 * Debounced timer hook with cancel, flush, and unmount flush-cleanup.
 *
 * Generic primitive shared by inline editors that schedule a deferred
 * action (e.g. autosave) on each keystroke. Two callers today:
 *
 *   - `FilterEditor` (perspective formula bar) — debounces filter dispatch.
 *   - `DateEditor` (date picker popover) — debounces parsed-date commits.
 *
 * Contract:
 *
 *   - `schedule(fn, delayMs)` — (re)start the debounce with a new callback.
 *   - `cancel()` — drop any pending callback without invoking it.
 *   - `flush()` — if a timer is pending, clear it and invoke the stored
 *     callback synchronously. Used to commit a pending save on Enter, on
 *     completion accept, or on unmount.
 *
 * On unmount, `flush` is called (not `cancel`) so a pending action still
 * fires — otherwise React reconciliation can silently drop a save scheduled
 * just before the component is keyed away.
 */

import { useCallback, useEffect, useRef } from "react";

/** Handle returned by {@link useDebouncedTimer}. */
export interface DebouncedTimerHandle {
  /** Schedule a callback to fire after `delayMs`, replacing any prior pending callback. */
  schedule: (fn: () => void, delayMs: number) => void;
  /** Drop any pending callback without invoking it. */
  cancel: () => void;
  /** Synchronously invoke the pending callback (if any) and clear the timer. */
  flush: () => void;
}

/**
 * React hook that returns `{ schedule, cancel, flush }` for a single in-flight
 * debounced callback.
 *
 * Only one callback is in flight at a time — calling `schedule` again replaces
 * the pending callback. The hook owns its own timer ref and cleans it up on
 * unmount by flushing the pending callback (so a save scheduled just before
 * the component unmounts still fires).
 */
export function useDebouncedTimer(): DebouncedTimerHandle {
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pendingFnRef = useRef<(() => void) | null>(null);

  const cancel = useCallback(() => {
    if (timerRef.current !== null) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
    pendingFnRef.current = null;
  }, []);

  const flush = useCallback(() => {
    if (timerRef.current !== null) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
    const fn = pendingFnRef.current;
    pendingFnRef.current = null;
    if (fn) fn();
  }, []);

  // Flush (not cancel) on unmount so a pending callback still fires.
  useEffect(() => flush, [flush]);

  const schedule = useCallback((fn: () => void, delayMs: number) => {
    if (timerRef.current !== null) clearTimeout(timerRef.current);
    pendingFnRef.current = fn;
    timerRef.current = setTimeout(() => {
      timerRef.current = null;
      pendingFnRef.current = null;
      fn();
    }, delayMs);
  }, []);

  return { schedule, cancel, flush };
}
