/**
 * useDebouncedSave — React hook that debounces field saves.
 *
 * Returns an `onChange` callback that restarts a debounce timer on every call,
 * and a `flush` function that fires any pending save immediately. The timer is
 * cleaned up on unmount.
 */

import { useCallback, useEffect, useRef } from "react";
import { warn } from "@/lib/log";

/** Options for the useDebouncedSave hook. */
export interface UseDebouncedSaveOptions {
  /** The field update function (from useFieldUpdate). */
  updateField: (
    entityType: string,
    entityId: string,
    fieldName: string,
    value: unknown,
  ) => Promise<void>;
  /** Entity type (e.g. "task", "tag"). */
  entityType: string;
  /** Entity ID. */
  entityId: string;
  /** Field name to update. */
  fieldName: string;
  /** Debounce delay in milliseconds. Defaults to 1000. */
  delayMs?: number;
}

/** Return value of useDebouncedSave. */
export interface DebouncedSaveHandle {
  /** Report an intermediate value change. Restarts the debounce timer. */
  onChange: (value: unknown) => void;
  /** If a save is pending, fire it immediately and clear the timer. */
  flush: () => void;
  /** Cancel any pending save without firing it. */
  cancel: () => void;
}

/**
 * Hook that debounces intermediate field value changes before persisting.
 *
 * - `onChange(value)` starts/restarts a debounce timer; when it fires, calls
 *   `updateField` with the latest value.
 * - `flush()` fires the pending save immediately (used before commit).
 * - `cancel()` discards the pending save without firing (used on cancel/unmount).
 *
 * The timer is automatically cancelled on unmount.
 */
export function useDebouncedSave(
  opts: UseDebouncedSaveOptions,
): DebouncedSaveHandle {
  const { updateField, entityType, entityId, fieldName, delayMs = 1000 } = opts;

  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pendingValueRef = useRef<unknown>(undefined);
  const hasPendingRef = useRef(false);

  // Keep latest opts in refs so callbacks are stable
  const updateFieldRef = useRef(updateField);
  updateFieldRef.current = updateField;
  const entityTypeRef = useRef(entityType);
  entityTypeRef.current = entityType;
  const entityIdRef = useRef(entityId);
  entityIdRef.current = entityId;
  const fieldNameRef = useRef(fieldName);
  fieldNameRef.current = fieldName;

  const cancel = useCallback(() => {
    if (timerRef.current !== null) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
    hasPendingRef.current = false;
    pendingValueRef.current = undefined;
  }, []);

  const flush = useCallback(() => {
    if (!hasPendingRef.current) return;
    if (timerRef.current !== null) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
    const value = pendingValueRef.current;
    hasPendingRef.current = false;
    pendingValueRef.current = undefined;
    updateFieldRef
      .current(
        entityTypeRef.current,
        entityIdRef.current,
        fieldNameRef.current,
        value,
      )
      .catch((e: unknown) => warn(`autosave failed: ${e}`));
  }, []);

  const onChange = useCallback(
    (value: unknown) => {
      pendingValueRef.current = value;
      hasPendingRef.current = true;
      if (timerRef.current !== null) {
        clearTimeout(timerRef.current);
      }
      timerRef.current = setTimeout(() => {
        timerRef.current = null;
        if (hasPendingRef.current) {
          const v = pendingValueRef.current;
          hasPendingRef.current = false;
          pendingValueRef.current = undefined;
          updateFieldRef
            .current(
              entityTypeRef.current,
              entityIdRef.current,
              fieldNameRef.current,
              v,
            )
            .catch((e: unknown) => warn(`autosave failed: ${e}`));
        }
      }, delayMs);
    },
    [delayMs],
  );

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (timerRef.current !== null) {
        clearTimeout(timerRef.current);
        timerRef.current = null;
      }
    };
  }, []);

  return { onChange, flush, cancel };
}
