import { describe, it, expect } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useGrid } from "./use-grid";

describe("useGrid", () => {
  const defaults = { rowCount: 5, colCount: 4 };

  it("initializes at 0,0 in normal mode", () => {
    const { result } = renderHook(() => useGrid(defaults));
    expect(result.current.cursor).toEqual({ row: 0, col: 0 });
    expect(result.current.mode).toBe("normal");
    expect(result.current.selection).toBeNull();
  });

  it("uses external cursor when provided", () => {
    const { result } = renderHook(() =>
      useGrid({ ...defaults, cursor: { row: 2, col: 3 } }),
    );
    expect(result.current.cursor).toEqual({ row: 2, col: 3 });
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

  it("expandSelection extends the head", () => {
    const { result } = renderHook(() => useGrid(defaults));
    act(() => result.current.enterVisual());
    act(() => result.current.expandSelection("down"));
    act(() => result.current.expandSelection("right"));
    const range = result.current.getSelectedRange();
    expect(range).toEqual({ startRow: 0, endRow: 1, startCol: 0, endCol: 1 });
  });

  it("exitVisual clears selection", () => {
    const { result } = renderHook(() => useGrid(defaults));
    act(() => result.current.enterVisual());
    act(() => result.current.expandSelection("down"));
    act(() => result.current.exitVisual());
    expect(result.current.mode).toBe("normal");
    expect(result.current.selection).toBeNull();
  });

  it("setCursor sets exact position", () => {
    const { result } = renderHook(() => useGrid(defaults));
    act(() => result.current.setCursor(3, 2));
    expect(result.current.cursor).toEqual({ row: 3, col: 2 });
  });

  it("setCursor clamps out-of-bounds", () => {
    const { result } = renderHook(() => useGrid(defaults));
    act(() => result.current.setCursor(100, 100));
    expect(result.current.cursor).toEqual({ row: 4, col: 3 });
  });
});
