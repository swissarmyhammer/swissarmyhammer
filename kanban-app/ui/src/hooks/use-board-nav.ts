import { useState, useCallback, useMemo } from "react";

export type BoardMode = "normal" | "edit";

export interface BoardCursor {
  col: number; // column index
  card: number; // card index within column (-1 = column header focused)
}

export interface UseBoardNavOptions {
  columnCount: number;
  cardCounts: number[]; // length === columnCount
}

export interface UseBoardNavReturn {
  cursor: BoardCursor;
  mode: BoardMode;
  moveLeft: () => void;
  moveRight: () => void;
  moveUp: () => void;
  moveDown: () => void;
  moveToFirstColumn: () => void;
  moveToLastColumn: () => void;
  moveToFirstCard: () => void;
  moveToLastCard: () => void;
  setCursor: (col: number, card: number) => void;
  enterEdit: () => void;
  exitEdit: () => void;
}

/**
 * Hook for managing 2D cursor navigation on a kanban board.
 *
 * The board is a 2D space: columns (horizontal) × cards within a column
 * (vertical). The cursor tracks { col, card } where card -1 means the
 * column header is focused. Moving up from card 0 goes to -1 (header),
 * moving down from -1 goes to card 0.
 *
 * When moving left/right, the card index is clamped to the new column's
 * card count to preserve approximate vertical position.
 *
 * @param options - columnCount and per-column cardCounts array
 * @returns Board cursor state and control functions
 */
export function useBoardNav({
  columnCount,
  cardCounts,
}: UseBoardNavOptions): UseBoardNavReturn {
  const initialCard = (cardCounts[0] ?? 0) === 0 ? -1 : 0;
  const [cursor, setCursorState] = useState<BoardCursor>({
    col: 0,
    card: initialCard,
  });
  const [mode, setMode] = useState<BoardMode>("normal");

  /** Clamp a column index to valid bounds [0, columnCount-1]. */
  const clampCol = useCallback(
    (c: number) => Math.max(0, Math.min(c, columnCount - 1)),
    [columnCount],
  );

  /**
   * Clamp a card index for a given column.
   * Valid range is [-1, cardCounts[col]-1]. -1 = column header.
   * Empty columns always return -1.
   */
  const clampCard = useCallback(
    (col: number, card: number) => {
      const count = cardCounts[col] ?? 0;
      if (count === 0) return -1;
      return Math.max(-1, Math.min(card, count - 1));
    },
    [cardCounts],
  );

  /** Set the cursor to an exact position, clamped to column and card bounds. */
  const setCursor = useCallback(
    (col: number, card: number) => {
      const clampedCol = clampCol(col);
      const clampedCard = clampCard(clampedCol, card);
      setCursorState({ col: clampedCol, card: clampedCard });
    },
    [clampCol, clampCard],
  );

  /** Move cursor left one column, clamping card to the new column's count. */
  const moveLeft = useCallback(() => {
    setCursorState((prev) => {
      const col = clampCol(prev.col - 1);
      const card = clampCard(col, prev.card);
      return { col, card };
    });
  }, [clampCol, clampCard]);

  /** Move cursor right one column, clamping card to the new column's count. */
  const moveRight = useCallback(() => {
    setCursorState((prev) => {
      const col = clampCol(prev.col + 1);
      const card = clampCard(col, prev.card);
      return { col, card };
    });
  }, [clampCol, clampCard]);

  /**
   * Move cursor up one card within the current column.
   * From card 0, moves to -1 (column header).
   * From -1, this is a no-op.
   */
  const moveUp = useCallback(() => {
    setCursorState((prev) => {
      if (prev.card === -1) return prev;
      return { ...prev, card: prev.card - 1 };
    });
  }, []);

  /**
   * Move cursor down one card within the current column.
   * From -1 (header), moves to card 0 if column has cards.
   * In an empty column, this is a no-op.
   */
  const moveDown = useCallback(
    (count = 1) => {
      setCursorState((prev) => {
        const colCount = cardCounts[prev.col] ?? 0;
        if (colCount === 0) return prev; // empty column
        const card = clampCard(prev.col, prev.card + count);
        return { ...prev, card };
      });
    },
    [clampCard, cardCounts],
  );

  /** Move cursor to the first column, clamping card to that column's count. */
  const moveToFirstColumn = useCallback(() => {
    setCursorState((prev) => {
      const col = 0;
      const card = clampCard(col, prev.card);
      return { col, card };
    });
  }, [clampCard]);

  /** Move cursor to the last column, clamping card to that column's count. */
  const moveToLastColumn = useCallback(() => {
    setCursorState((prev) => {
      const col = clampCol(columnCount - 1);
      const card = clampCard(col, prev.card);
      return { col, card };
    });
  }, [clampCol, clampCard, columnCount]);

  /**
   * Move cursor to the first card (index 0) in the current column.
   * If the column is empty, stays at -1.
   */
  const moveToFirstCard = useCallback(() => {
    setCursorState((prev) => {
      const count = cardCounts[prev.col] ?? 0;
      return { ...prev, card: count === 0 ? -1 : 0 };
    });
  }, [cardCounts]);

  /**
   * Move cursor to the last card in the current column.
   * If the column is empty, stays at -1.
   */
  const moveToLastCard = useCallback(() => {
    setCursorState((prev) => {
      const count = cardCounts[prev.col] ?? 0;
      const card = count === 0 ? -1 : count - 1;
      return { ...prev, card };
    });
  }, [cardCounts]);

  /** Enter edit mode. */
  const enterEdit = useCallback(() => {
    setMode("edit");
  }, []);

  /** Exit edit mode, returning to normal mode. */
  const exitEdit = useCallback(() => {
    setMode("normal");
  }, []);

  return useMemo(
    () => ({
      cursor,
      mode,
      moveLeft,
      moveRight,
      moveUp,
      moveDown,
      moveToFirstColumn,
      moveToLastColumn,
      moveToFirstCard,
      moveToLastCard,
      setCursor,
      enterEdit,
      exitEdit,
    }),
    [
      cursor,
      mode,
      moveLeft,
      moveRight,
      moveUp,
      moveDown,
      moveToFirstColumn,
      moveToLastColumn,
      moveToFirstCard,
      moveToLastCard,
      setCursor,
      enterEdit,
      exitEdit,
    ],
  );
}
