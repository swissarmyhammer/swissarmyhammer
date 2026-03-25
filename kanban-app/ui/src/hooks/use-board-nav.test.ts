import { describe, it, expect } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useBoardNav } from "./use-board-nav";

describe("useBoardNav", () => {
  // 3 columns: [3 cards, 1 card, 0 cards]
  const defaults = { columnCount: 3, cardCounts: [3, 1, 0] };

  it("initializes at col 0, card 0 in normal mode", () => {
    const { result } = renderHook(() => useBoardNav(defaults));
    expect(result.current.cursor).toEqual({ col: 0, card: 0 });
    expect(result.current.mode).toBe("normal");
  });

  it("initializes at card -1 for empty-only board", () => {
    const { result } = renderHook(() =>
      useBoardNav({ columnCount: 1, cardCounts: [0] }),
    );
    expect(result.current.cursor).toEqual({ col: 0, card: -1 });
  });

  it("moveRight increments column", () => {
    const { result } = renderHook(() => useBoardNav(defaults));
    act(() => result.current.moveRight());
    expect(result.current.cursor.col).toBe(1);
  });

  it("moveLeft decrements column", () => {
    const { result } = renderHook(() => useBoardNav(defaults));
    act(() => result.current.moveRight());
    act(() => result.current.moveLeft());
    expect(result.current.cursor.col).toBe(0);
  });

  it("moveLeft clamps to column 0", () => {
    const { result } = renderHook(() => useBoardNav(defaults));
    act(() => result.current.moveLeft());
    expect(result.current.cursor.col).toBe(0);
  });

  it("moveRight clamps to last column", () => {
    const { result } = renderHook(() => useBoardNav(defaults));
    act(() => result.current.moveRight());
    act(() => result.current.moveRight());
    act(() => result.current.moveRight());
    expect(result.current.cursor.col).toBe(2);
  });

  it("moveRight clamps card to new column's card count", () => {
    const { result } = renderHook(() => useBoardNav(defaults));
    // Move to card 2 in col 0 (3 cards)
    act(() => result.current.moveDown()); // 0 -> 1
    act(() => result.current.moveDown()); // 1 -> 2
    expect(result.current.cursor).toEqual({ col: 0, card: 2 });
    // Move right to col 1 (only 1 card, index 0)
    act(() => result.current.moveRight());
    expect(result.current.cursor).toEqual({ col: 1, card: 0 });
  });

  it("moveRight to empty column resets card to -1", () => {
    const { result } = renderHook(() => useBoardNav(defaults));
    act(() => result.current.moveRight()); // col 0 -> 1
    act(() => result.current.moveRight()); // col 1 -> 2 (empty)
    expect(result.current.cursor).toEqual({ col: 2, card: -1 });
  });

  it("moveDown from header goes to card 0", () => {
    const { result } = renderHook(() => useBoardNav(defaults));
    act(() => result.current.moveUp()); // 0 -> -1
    act(() => result.current.moveDown()); // -1 -> 0
    expect(result.current.cursor.card).toBe(0);
  });

  it("moveDown increments card within column", () => {
    const { result } = renderHook(() => useBoardNav(defaults));
    act(() => result.current.moveDown()); // 0 -> 1
    expect(result.current.cursor.card).toBe(1);
  });

  it("moveDown clamps to last card", () => {
    const { result } = renderHook(() => useBoardNav(defaults));
    act(() => result.current.moveDown(100));
    expect(result.current.cursor.card).toBe(2); // col 0 has 3 cards, max index 2
  });

  it("moveDown in empty column is a no-op", () => {
    const { result } = renderHook(() =>
      useBoardNav({ columnCount: 1, cardCounts: [0] }),
    );
    act(() => result.current.moveDown());
    expect(result.current.cursor.card).toBe(-1);
  });

  it("moveUp from card 0 goes to -1 (column header)", () => {
    const { result } = renderHook(() => useBoardNav(defaults));
    act(() => result.current.moveUp());
    expect(result.current.cursor.card).toBe(-1);
  });

  it("moveUp from -1 is a no-op", () => {
    const { result } = renderHook(() => useBoardNav(defaults));
    act(() => result.current.moveUp()); // 0 -> -1
    act(() => result.current.moveUp()); // -1 -> still -1
    expect(result.current.cursor.card).toBe(-1);
  });

  it("moveUp decrements card index", () => {
    const { result } = renderHook(() => useBoardNav(defaults));
    act(() => result.current.moveDown(2)); // 0 -> 2
    act(() => result.current.moveUp());
    expect(result.current.cursor.card).toBe(1);
  });

  it("moveToFirstColumn goes to column 0", () => {
    const { result } = renderHook(() => useBoardNav(defaults));
    act(() => result.current.moveRight());
    act(() => result.current.moveRight());
    act(() => result.current.moveToFirstColumn());
    expect(result.current.cursor.col).toBe(0);
  });

  it("moveToLastColumn goes to last column", () => {
    const { result } = renderHook(() => useBoardNav(defaults));
    act(() => result.current.moveToLastColumn());
    expect(result.current.cursor.col).toBe(2);
  });

  it("moveToFirstCard goes to card 0", () => {
    const { result } = renderHook(() => useBoardNav(defaults));
    act(() => result.current.moveDown(2));
    act(() => result.current.moveToFirstCard());
    expect(result.current.cursor.card).toBe(0);
  });

  it("moveToFirstCard in empty column stays at -1", () => {
    const { result } = renderHook(() =>
      useBoardNav({ columnCount: 1, cardCounts: [0] }),
    );
    act(() => result.current.moveToFirstCard());
    expect(result.current.cursor.card).toBe(-1);
  });

  it("moveToLastCard goes to last card in column", () => {
    const { result } = renderHook(() => useBoardNav(defaults));
    act(() => result.current.moveToLastCard());
    expect(result.current.cursor.card).toBe(2); // col 0 has 3 cards
  });

  it("moveToLastCard in empty column stays at -1", () => {
    const { result } = renderHook(() =>
      useBoardNav({ columnCount: 1, cardCounts: [0] }),
    );
    act(() => result.current.moveToLastCard());
    expect(result.current.cursor.card).toBe(-1);
  });

  it("setCursor sets exact position", () => {
    const { result } = renderHook(() => useBoardNav(defaults));
    act(() => result.current.setCursor(1, 0));
    expect(result.current.cursor).toEqual({ col: 1, card: 0 });
  });

  it("setCursor clamps col out-of-bounds", () => {
    const { result } = renderHook(() => useBoardNav(defaults));
    act(() => result.current.setCursor(100, 0));
    expect(result.current.cursor.col).toBe(2);
  });

  it("setCursor clamps card to new column's count", () => {
    const { result } = renderHook(() => useBoardNav(defaults));
    act(() => result.current.setCursor(1, 100)); // col 1 has 1 card, max index 0
    expect(result.current.cursor).toEqual({ col: 1, card: 0 });
  });

  it("enterEdit switches to edit mode", () => {
    const { result } = renderHook(() => useBoardNav(defaults));
    act(() => result.current.enterEdit());
    expect(result.current.mode).toBe("edit");
  });

  it("exitEdit returns to normal mode", () => {
    const { result } = renderHook(() => useBoardNav(defaults));
    act(() => result.current.enterEdit());
    act(() => result.current.exitEdit());
    expect(result.current.mode).toBe("normal");
  });

  it("moveToFirstColumn clamps card to new column's count", () => {
    const { result } = renderHook(() =>
      useBoardNav({ columnCount: 2, cardCounts: [1, 3] }),
    );
    // Start at col 0 card 0, move right, then to card 2
    act(() => result.current.moveRight()); // col -> 1
    act(() => result.current.moveDown()); // 0 -> 1
    act(() => result.current.moveDown()); // 1 -> 2
    expect(result.current.cursor).toEqual({ col: 1, card: 2 });
    // Move to first column (only 1 card, max index 0)
    act(() => result.current.moveToFirstColumn());
    expect(result.current.cursor).toEqual({ col: 0, card: 0 });
  });
});
