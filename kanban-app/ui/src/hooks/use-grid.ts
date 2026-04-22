import {
  useState,
  useCallback,
  useMemo,
  type Dispatch,
  type SetStateAction,
} from "react";

/**
 * The grid's interaction mode.
 *
 * - `"normal"` — navigation only; `h`/`j`/`k`/`l` move spatial focus
 *   between cells and no cell-local editor is open.
 * - `"edit"` — the cursor cell's editor is active; keystrokes land in
 *   the editor instead of driving navigation.
 * - `"visual"` — a rectangular range is being selected; movement keys
 *   extend `GridSelection.head` while the anchor stays pinned.
 */
export type GridMode = "normal" | "edit" | "visual";

/**
 * A cursor position in the grid, referencing a single cell by its
 * zero-based data row index and column index. Not a pixel coordinate —
 * the DOM rect for the cell is looked up separately.
 */
export interface GridCursor {
  row: number;
  col: number;
}

/**
 * A rectangular visual-mode selection: an `anchor` (where selection started)
 * and a `head` (the cursor end that expands with `expandSelection`).
 */
export interface GridSelection {
  anchor: GridCursor;
  head: GridCursor;
}

/** Options accepted by `useGrid`: grid dimensions and an externally-derived cursor. */
export interface UseGridOptions {
  rowCount: number;
  colCount: number;
  /**
   * Externally-derived cursor position (from the focused moniker).
   *
   * Spatial focus is the single source of truth for "where the user is":
   * callers compute this by looking up the focused moniker in their cell
   * moniker map and passing `{ row, col }` when the focus points at a
   * data cell, or `null` when focus is on a non-cell target (column
   * header, row selector, perspective tab) or no cell is focused.
   *
   * The hook does not maintain its own cursor state — there is no parallel
   * state machine. When this option is `null` or `undefined`, `cursor` is
   * `null` and no cell is treated as the cursor target.
   */
  cursor?: GridCursor | null;
}

/**
 * Return shape of `useGrid`: the derived cursor, current mode, active
 * selection, plus mode/selection control callbacks. Callers destructure
 * only what they need; identities are stable across re-renders.
 */
export interface UseGridReturn {
  /**
   * Current cursor position derived from spatial focus, or `null` when
   * no data cell is focused.
   */
  cursor: GridCursor | null;
  mode: GridMode;
  selection: GridSelection | null;
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

/** Clamp helpers bounded by the grid's `rowCount` and `colCount`. */
function useClampers(rowCount: number, colCount: number) {
  const clampRow = useCallback(
    (r: number) => Math.max(0, Math.min(r, rowCount - 1)),
    [rowCount],
  );
  const clampCol = useCallback(
    (c: number) => Math.max(0, Math.min(c, colCount - 1)),
    [colCount],
  );
  return { clampRow, clampCol };
}

function useModeControls(
  rowCount: number,
  colCount: number,
  cursor: GridCursor | null,
  setMode: Dispatch<SetStateAction<GridMode>>,
  setSelection: Dispatch<SetStateAction<GridSelection | null>>,
) {
  const enterEdit = useCallback(() => {
    if (rowCount > 0 && colCount > 0) {
      setMode("edit");
      setSelection(null);
    }
  }, [rowCount, colCount, setMode, setSelection]);

  const exitEdit = useCallback(() => setMode("normal"), [setMode]);

  const enterVisual = useCallback(() => {
    if (!cursor) return;
    setMode("visual");
    setSelection({ anchor: { ...cursor }, head: { ...cursor } });
  }, [cursor, setMode, setSelection]);

  const exitVisual = useCallback(() => {
    setMode("normal");
    setSelection(null);
  }, [setMode, setSelection]);

  return { enterEdit, exitEdit, enterVisual, exitVisual };
}

function moveHead(
  head: { row: number; col: number },
  direction: "up" | "down" | "left" | "right",
  clampRow: (r: number) => number,
  clampCol: (c: number) => number,
) {
  const next = { ...head };
  switch (direction) {
    case "up":
      next.row = clampRow(head.row - 1);
      break;
    case "down":
      next.row = clampRow(head.row + 1);
      break;
    case "left":
      next.col = clampCol(head.col - 1);
      break;
    case "right":
      next.col = clampCol(head.col + 1);
      break;
  }
  return next;
}

function useExpandSelection(
  cursor: GridCursor | null,
  clampRow: (r: number) => number,
  clampCol: (c: number) => number,
  setSelection: Dispatch<SetStateAction<GridSelection | null>>,
) {
  return useCallback(
    (direction: "up" | "down" | "left" | "right") => {
      setSelection((sel) => {
        if (!sel) {
          if (!cursor) return null;
          return { anchor: { ...cursor }, head: { ...cursor } };
        }
        return {
          ...sel,
          head: moveHead(sel.head, direction, clampRow, clampCol),
        };
      });
    },
    [cursor, clampRow, clampCol, setSelection],
  );
}

function useSelectedRange(selection: GridSelection | null) {
  return useCallback(() => {
    if (!selection) return null;
    return {
      startRow: Math.min(selection.anchor.row, selection.head.row),
      endRow: Math.max(selection.anchor.row, selection.head.row),
      startCol: Math.min(selection.anchor.col, selection.head.col),
      endCol: Math.max(selection.anchor.col, selection.head.col),
    };
  }, [selection]);
}

/**
 * Hook for managing grid mode (normal/edit/visual) and visual selection.
 *
 * The cursor is a pure derivation of spatial focus — it is never an
 * independent source of truth. Callers pass the cursor position they
 * compute from `useFocusedMoniker()` via `options.cursor`. When spatial
 * focus is on a non-cell target (or nothing), callers pass `null` and
 * no row/column is treated as the cursor. This guarantees the grid
 * never shows a "ghost" cursor highlight that disagrees with the actual
 * focused element.
 *
 * Navigation between cells is driven by Rust spatial nav on each cell's
 * `FocusScope`; this hook only manages mode (normal/edit/visual) and the
 * visual-mode selection range — both of which are orthogonal to the
 * cursor position.
 *
 * @param options - Grid dimensions and the externally-derived cursor
 * @returns Grid state and mode/selection control functions
 */
export function useGrid({
  rowCount,
  colCount,
  cursor: externalCursor,
}: UseGridOptions): UseGridReturn {
  const [mode, setMode] = useState<GridMode>("normal");
  const [selection, setSelection] = useState<GridSelection | null>(null);
  const cursor = externalCursor ?? null;
  const { clampRow, clampCol } = useClampers(rowCount, colCount);
  const modeControls = useModeControls(
    rowCount,
    colCount,
    cursor,
    setMode,
    setSelection,
  );
  const expandSelection = useExpandSelection(
    cursor,
    clampRow,
    clampCol,
    setSelection,
  );
  const getSelectedRange = useSelectedRange(selection);

  return useMemo(
    () => ({
      cursor,
      mode,
      selection,
      ...modeControls,
      expandSelection,
      getSelectedRange,
    }),
    [cursor, mode, selection, modeControls, expandSelection, getSelectedRange],
  );
}
