import { describe, it, expect } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useGrid } from "./use-grid";

describe("useGrid", () => {
  const defaults = { rowCount: 5, colCount: 4 };

  it("cursor is null when no external cursor is provided", () => {
    // With the collapse-to-one-source-of-truth refactor, the hook no longer
    // maintains an internal (0,0) cursor. A null cursor is the correct
    // signal that spatial focus is not on a data cell.
    const { result } = renderHook(() => useGrid(defaults));
    expect(result.current.cursor).toBeNull();
    expect(result.current.mode).toBe("normal");
    expect(result.current.selection).toBeNull();
  });

  it("uses external cursor when provided", () => {
    const { result } = renderHook(() =>
      useGrid({ ...defaults, cursor: { row: 2, col: 3 } }),
    );
    expect(result.current.cursor).toEqual({ row: 2, col: 3 });
  });

  it("cursor becomes null when external cursor is cleared", () => {
    type Props = { cursor: { row: number; col: number } | null };
    const { result, rerender } = renderHook(
      ({ cursor }: Props) => useGrid({ ...defaults, cursor }),
      { initialProps: { cursor: { row: 2, col: 3 } } as Props },
    );
    expect(result.current.cursor).toEqual({ row: 2, col: 3 });
    rerender({ cursor: null });
    expect(result.current.cursor).toBeNull();
  });

  it("cursor follows external input across re-renders", () => {
    // Simulates spatial focus moving between cells: each time the caller
    // derives a new cursor from `useFocusedMoniker()` and re-renders,
    // `grid.cursor` must track that — no lag, no stale (0,0) fallback.
    type Props = { cursor: { row: number; col: number } | null };
    const { result, rerender } = renderHook(
      ({ cursor }: Props) => useGrid({ ...defaults, cursor }),
      { initialProps: { cursor: { row: 0, col: 0 } } as Props },
    );
    expect(result.current.cursor).toEqual({ row: 0, col: 0 });
    rerender({ cursor: { row: 2, col: 1 } });
    expect(result.current.cursor).toEqual({ row: 2, col: 1 });
    rerender({ cursor: { row: 1, col: 3 } });
    expect(result.current.cursor).toEqual({ row: 1, col: 3 });
  });

  it("enterEdit switches to edit mode", () => {
    const { result } = renderHook(() => useGrid(defaults));
    act(() => result.current.enterEdit());
    expect(result.current.mode).toBe("edit");
  });

  it("exitEdit returns to normal mode", () => {
    const { result } = renderHook(() => useGrid(defaults));
    act(() => result.current.enterEdit());
    act(() => result.current.exitEdit());
    expect(result.current.mode).toBe("normal");
  });

  it("enterEdit does nothing on empty grid", () => {
    const { result } = renderHook(() => useGrid({ rowCount: 0, colCount: 0 }));
    act(() => result.current.enterEdit());
    expect(result.current.mode).toBe("normal");
  });

  it("enterVisual starts selection at cursor", () => {
    const { result } = renderHook(() =>
      useGrid({ ...defaults, cursor: { row: 1, col: 1 } }),
    );
    act(() => result.current.enterVisual());
    expect(result.current.mode).toBe("visual");
    expect(result.current.selection).toEqual({
      anchor: { row: 1, col: 1 },
      head: { row: 1, col: 1 },
    });
  });

  it("enterVisual is a no-op when cursor is null", () => {
    // Visual selection needs a concrete anchor cell — without one, the
    // selection would be ambiguous. Enter is silently ignored.
    const { result } = renderHook(() => useGrid(defaults));
    act(() => result.current.enterVisual());
    expect(result.current.mode).toBe("normal");
    expect(result.current.selection).toBeNull();
  });

  it("expandSelection extends the head", () => {
    const { result } = renderHook(() =>
      useGrid({ ...defaults, cursor: { row: 0, col: 0 } }),
    );
    act(() => result.current.enterVisual());
    act(() => result.current.expandSelection("down"));
    act(() => result.current.expandSelection("right"));
    const range = result.current.getSelectedRange();
    expect(range).toEqual({ startRow: 0, endRow: 1, startCol: 0, endCol: 1 });
  });

  it("exitVisual clears selection", () => {
    const { result } = renderHook(() =>
      useGrid({ ...defaults, cursor: { row: 0, col: 0 } }),
    );
    act(() => result.current.enterVisual());
    act(() => result.current.expandSelection("down"));
    act(() => result.current.exitVisual());
    expect(result.current.mode).toBe("normal");
    expect(result.current.selection).toBeNull();
  });
});
