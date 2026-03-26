import { useState, useCallback, useEffect, useMemo } from "react";

export type InspectorMode = "normal" | "edit";

export interface UseInspectorNavOptions {
  fieldCount: number;
}

export interface UseInspectorNavReturn {
  focusedIndex: number;
  mode: InspectorMode;
  fieldCount: number;
  // Navigation
  moveUp: (count?: number) => void;
  moveDown: (count?: number) => void;
  moveToFirst: () => void;
  moveToLast: () => void;
  setFocusedIndex: (index: number) => void;
  // Pill navigation (horizontal within badge-list fields)
  pillIndex: number;
  pillCount: number;
  setPillCount: (n: number) => void;
  movePillLeft: () => void;
  movePillRight: () => void;
  // Mode
  enterEdit: () => void;
  exitEdit: () => void;
}

/**
 * Hook for managing 1D cursor navigation through inspector fields.
 *
 * Provides vertical navigation with clamping and normal/edit mode toggling.
 * Modeled after useGrid but simplified for a single-column field list.
 *
 * @param options - Inspector dimensions: fieldCount
 * @returns Inspector nav state and control functions
 */
export function useInspectorNav({
  fieldCount,
}: UseInspectorNavOptions): UseInspectorNavReturn {
  const [focusedIndex, setFocusedIndexState] = useState(0);
  const [mode, setMode] = useState<InspectorMode>("normal");
  const [pillIndex, setPillIndexState] = useState(-1);
  const [pillCount, setPillCountState] = useState(0);

  /** Clamp an index to valid bounds [0, fieldCount-1]. */
  const clampIndex = useCallback(
    (i: number) => Math.max(0, Math.min(i, fieldCount - 1)),
    [fieldCount],
  );

  /** Set the focused index to an exact position, clamped to bounds. */
  const setFocusedIndex = useCallback(
    (index: number) => {
      setFocusedIndexState(clampIndex(index));
    },
    [clampIndex],
  );

  /** Move focus up by count fields (default 1), clamped to index 0. */
  const moveUp = useCallback(
    (count = 1) => {
      setFocusedIndexState((prev) => clampIndex(prev - count));
    },
    [clampIndex],
  );

  /** Move focus down by count fields (default 1), clamped to last index. */
  const moveDown = useCallback(
    (count = 1) => {
      setFocusedIndexState((prev) => clampIndex(prev + count));
    },
    [clampIndex],
  );

  /** Move focus to the first field (index 0). */
  const moveToFirst = useCallback(() => {
    setFocusedIndexState(0);
  }, []);

  /** Move focus to the last field. */
  const moveToLast = useCallback(() => {
    setFocusedIndexState(clampIndex(fieldCount - 1));
  }, [clampIndex, fieldCount]);

  /** Enter edit mode if the field list is non-empty. */
  const enterEdit = useCallback(() => {
    if (fieldCount > 0) {
      setMode("edit");
    }
  }, [fieldCount]);

  /** Exit edit mode, returning to normal mode. */
  const exitEdit = useCallback(() => {
    setMode("normal");
  }, []);

  /** Update pill count and clamp pillIndex if it exceeds the new count. */
  const setPillCount = useCallback((n: number) => {
    setPillCountState(n);
    setPillIndexState((prev) => (prev < 0 ? prev : n <= 0 ? -1 : Math.min(prev, n - 1)));
  }, []);

  /** Move pill focus left by one, clamped to 0. No-op if pill nav is inactive (-1). */
  const movePillLeft = useCallback(() => {
    setPillIndexState((prev) => (prev < 0 ? -1 : prev === 0 ? 0 : prev - 1));
  }, []);

  /** Move pill focus right by one. Enters pill nav from -1 → 0, clamps at pillCount - 1. */
  const movePillRight = useCallback(() => {
    setPillIndexState((prev) =>
      pillCount <= 0 ? -1 : prev < 0 ? 0 : Math.min(prev + 1, pillCount - 1),
    );
  }, [pillCount]);

  // Reset pill nav to inactive whenever the focused field changes
  useEffect(() => {
    setPillIndexState(-1);
  }, [focusedIndex]);

  return useMemo(
    () => ({
      focusedIndex,
      mode,
      fieldCount,
      moveUp,
      moveDown,
      moveToFirst,
      moveToLast,
      setFocusedIndex,
      pillIndex,
      pillCount,
      setPillCount,
      movePillLeft,
      movePillRight,
      enterEdit,
      exitEdit,
    }),
    [
      focusedIndex,
      mode,
      fieldCount,
      moveUp,
      moveDown,
      moveToFirst,
      moveToLast,
      setFocusedIndex,
      pillIndex,
      pillCount,
      setPillCount,
      movePillLeft,
      movePillRight,
      enterEdit,
      exitEdit,
    ],
  );
}
