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
}

export interface UseGridReturn {
  cursor: GridCursor;
  mode: GridMode;
  selection: GridSelection | null;
  // Navigation
  moveUp: (count?: number) => void;
  moveDown: (count?: number) => void;
  moveLeft: (count?: number) => void;
  moveRight: (count?: number) => void;
  moveToFirst: () => void;
  moveToLast: () => void;
  moveToRowStart: () => void;
  moveToRowEnd: () => void;
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
 * Hook for managing grid cursor position, navigation, mode, and visual selection.
 *
 * Provides vim-like navigation (move up/down/left/right with count),
 * mode switching (normal/edit/visual), and rectangular visual selection.
 *
 * @param options - Grid dimensions: rowCount and colCount
 * @returns Grid state and control functions
 */
export function useGrid({ rowCount, colCount }: UseGridOptions): UseGridReturn {
  const [cursor, setCursorState] = useState<GridCursor>({ row: 0, col: 0 });
  const [mode, setMode] = useState<GridMode>("normal");
  const [selection, setSelection] = useState<GridSelection | null>(null);

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

  /** Move cursor up by count rows (default 1), clamped to row 0. */
  const moveUp = useCallback(
    (count = 1) => {
      setCursorState((prev) => ({ ...prev, row: clampRow(prev.row - count) }));
    },
    [clampRow],
  );

  /** Move cursor down by count rows (default 1), clamped to last row. */
  const moveDown = useCallback(
    (count = 1) => {
      setCursorState((prev) => ({ ...prev, row: clampRow(prev.row + count) }));
    },
    [clampRow],
  );

  /** Move cursor left by count columns (default 1), clamped to column 0. */
  const moveLeft = useCallback(
    (count = 1) => {
      setCursorState((prev) => ({ ...prev, col: clampCol(prev.col - count) }));
    },
    [clampCol],
  );

  /** Move cursor right by count columns (default 1), clamped to last column. */
  const moveRight = useCallback(
    (count = 1) => {
      setCursorState((prev) => ({ ...prev, col: clampCol(prev.col + count) }));
    },
    [clampCol],
  );

  /** Move cursor to the first cell (0, 0). */
  const moveToFirst = useCallback(() => {
    setCursorState({ row: 0, col: 0 });
  }, []);

  /** Move cursor to the last cell (last row, last col). */
  const moveToLast = useCallback(() => {
    setCursorState({
      row: clampRow(rowCount - 1),
      col: clampCol(colCount - 1),
    });
  }, [clampRow, clampCol, rowCount, colCount]);

  /** Move cursor to column 0, keeping the current row. */
  const moveToRowStart = useCallback(() => {
    setCursorState((prev) => ({ ...prev, col: 0 }));
  }, []);

  /** Move cursor to the last column, keeping the current row. */
  const moveToRowEnd = useCallback(() => {
    setCursorState((prev) => ({ ...prev, col: clampCol(colCount - 1) }));
  }, [clampCol, colCount]);

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
    setCursorState((prev) => {
      setSelection({ anchor: { ...prev }, head: { ...prev } });
      return prev;
    });
  }, []);

  /** Exit visual mode, clearing the selection and returning to normal mode. */
  const exitVisual = useCallback(() => {
    setMode("normal");
    setSelection(null);
  }, []);

  /**
   * Expand the visual selection by moving the head one cell in the given direction.
   * Also moves the cursor to track the selection head.
   */
  const expandSelection = useCallback(
    (direction: "up" | "down" | "left" | "right") => {
      setCursorState((prev) => {
        const next = { ...prev };
        switch (direction) {
          case "up":
            next.row = clampRow(prev.row - 1);
            break;
          case "down":
            next.row = clampRow(prev.row + 1);
            break;
          case "left":
            next.col = clampCol(prev.col - 1);
            break;
          case "right":
            next.col = clampCol(prev.col + 1);
            break;
        }
        setSelection((sel) =>
          sel ? { ...sel, head: next } : { anchor: prev, head: next },
        );
        return next;
      });
    },
    [clampRow, clampCol],
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
      moveUp,
      moveDown,
      moveLeft,
      moveRight,
      moveToFirst,
      moveToLast,
      moveToRowStart,
      moveToRowEnd,
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
      moveUp,
      moveDown,
      moveLeft,
      moveRight,
      moveToFirst,
      moveToLast,
      moveToRowStart,
      moveToRowEnd,
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
