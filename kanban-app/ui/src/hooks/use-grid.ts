import { useState, useCallback, useMemo } from "react";

export type GridMode = "normal" | "edit" | "visual";

export interface GridCursor {
  row: number;
  col: number;
}

export interface GridSelection {
  anchor: GridCursor;
  head: GridCursor;
}

export interface UseGridOptions {
  rowCount: number;
  colCount: number;
  /**
   * Externally-derived cursor position (from the focused moniker).
   * When provided, the grid does not maintain its own cursor state --
   * navigation is driven by Rust spatial nav.
   */
  cursor?: GridCursor;
}

export interface UseGridReturn {
  cursor: GridCursor;
  mode: GridMode;
  selection: GridSelection | null;
  setCursor: (row: number, col: number) => void;
  // Mode
  enterEdit: () => void;
  exitEdit: () => void;
  enterVisual: () => void;
  exitVisual: () => void;
  // Selection
  expandSelection: (direction: "up" | "down" | "left" | "right") => void;
  getSelectedRange: () => {
    startRow: number;
    endRow: number;
    startCol: number;
    endCol: number;
  } | null;
}

/**
 * Hook for managing grid mode (normal/edit/visual) and visual selection.
 *
 * Navigation is pull-based: callers pass the cursor position derived from
 * the focused moniker via options.cursor. This hook no longer drives
 * cursor movement -- that is handled by Rust spatial navigation on each
 * cell's FocusScope.
 *
 * When options.cursor is not provided, falls back to internal state
 * (for setCursor / click handling).
 *
 * @param options - Grid dimensions and optional external cursor
 * @returns Grid state and control functions
 */
export function useGrid({
  rowCount,
  colCount,
  cursor: externalCursor,
}: UseGridOptions): UseGridReturn {
  const [internalCursor, setCursorState] = useState<GridCursor>({
    row: 0,
    col: 0,
  });
  const [mode, setMode] = useState<GridMode>("normal");
  const [selection, setSelection] = useState<GridSelection | null>(null);

  // Use external cursor when provided, otherwise use internal state.
  const cursor = externalCursor ?? internalCursor;

  /** Clamp a row index to valid bounds [0, rowCount-1]. */
  const clampRow = useCallback(
    (r: number) => Math.max(0, Math.min(r, rowCount - 1)),
    [rowCount],
  );

  /** Clamp a column index to valid bounds [0, colCount-1]. */
  const clampCol = useCallback(
    (c: number) => Math.max(0, Math.min(c, colCount - 1)),
    [colCount],
  );

  /** Set the cursor to an exact position, clamped to grid bounds. */
  const setCursor = useCallback(
    (row: number, col: number) => {
      setCursorState({ row: clampRow(row), col: clampCol(col) });
    },
    [clampRow, clampCol],
  );

  /** Enter edit mode if the grid is non-empty. Clears any visual selection. */
  const enterEdit = useCallback(() => {
    if (rowCount > 0 && colCount > 0) {
      setMode("edit");
      setSelection(null);
    }
  }, [rowCount, colCount]);

  /** Exit edit mode, returning to normal mode. */
  const exitEdit = useCallback(() => {
    setMode("normal");
  }, []);

  /** Enter visual mode, anchoring the selection at the current cursor position. */
  const enterVisual = useCallback(() => {
    setMode("visual");
    setSelection({ anchor: { ...cursor }, head: { ...cursor } });
  }, [cursor]);

  /** Exit visual mode, clearing the selection and returning to normal mode. */
  const exitVisual = useCallback(() => {
    setMode("normal");
    setSelection(null);
  }, []);

  /**
   * Expand the visual selection by moving the head one cell in the given direction.
   */
  const expandSelection = useCallback(
    (direction: "up" | "down" | "left" | "right") => {
      setSelection((sel) => {
        if (!sel) return { anchor: { ...cursor }, head: { ...cursor } };
        const next = { ...sel.head };
        switch (direction) {
          case "up":
            next.row = clampRow(sel.head.row - 1);
            break;
          case "down":
            next.row = clampRow(sel.head.row + 1);
            break;
          case "left":
            next.col = clampCol(sel.head.col - 1);
            break;
          case "right":
            next.col = clampCol(sel.head.col + 1);
            break;
        }
        return { ...sel, head: next };
      });
    },
    [cursor, clampRow, clampCol],
  );

  /**
   * Get the normalized selected range as min/max row and column indices.
   * Returns null if there is no active selection.
   */
  const getSelectedRange = useCallback(() => {
    if (!selection) return null;
    return {
      startRow: Math.min(selection.anchor.row, selection.head.row),
      endRow: Math.max(selection.anchor.row, selection.head.row),
      startCol: Math.min(selection.anchor.col, selection.head.col),
      endCol: Math.max(selection.anchor.col, selection.head.col),
    };
  }, [selection]);

  return useMemo(
    () => ({
      cursor,
      mode,
      selection,
      setCursor,
      enterEdit,
      exitEdit,
      enterVisual,
      exitVisual,
      expandSelection,
      getSelectedRange,
    }),
    [
      cursor,
      mode,
      selection,
      setCursor,
      enterEdit,
      exitEdit,
      enterVisual,
      exitVisual,
      expandSelection,
      getSelectedRange,
    ],
  );
}
