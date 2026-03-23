import { describe, it, expect } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useInspectorNav } from "./use-inspector-nav";

describe("useInspectorNav", () => {
  const defaults = { fieldCount: 5 };

  it("initializes at index 0 in normal mode", () => {
    const { result } = renderHook(() => useInspectorNav(defaults));
    expect(result.current.focusedIndex).toBe(0);
    expect(result.current.mode).toBe("normal");
  });

  it("moveDown increments index", () => {
    const { result } = renderHook(() => useInspectorNav(defaults));
    act(() => result.current.moveDown());
    expect(result.current.focusedIndex).toBe(1);
  });

  it("moveUp decrements index", () => {
    const { result } = renderHook(() => useInspectorNav(defaults));
    act(() => result.current.moveDown(2));
    act(() => result.current.moveUp());
    expect(result.current.focusedIndex).toBe(1);
  });

  it("moveDown accepts a count", () => {
    const { result } = renderHook(() => useInspectorNav(defaults));
    act(() => result.current.moveDown(3));
    expect(result.current.focusedIndex).toBe(3);
  });

  it("moveUp accepts a count", () => {
    const { result } = renderHook(() => useInspectorNav(defaults));
    act(() => result.current.moveDown(4));
    act(() => result.current.moveUp(2));
    expect(result.current.focusedIndex).toBe(2);
  });

  it("clamps index to lower bound", () => {
    const { result } = renderHook(() => useInspectorNav(defaults));
    act(() => result.current.moveUp(10));
    expect(result.current.focusedIndex).toBe(0);
  });

  it("clamps index to upper bound", () => {
    const { result } = renderHook(() => useInspectorNav(defaults));
    act(() => result.current.moveDown(100));
    expect(result.current.focusedIndex).toBe(4);
  });

  it("moveToFirst goes to index 0", () => {
    const { result } = renderHook(() => useInspectorNav(defaults));
    act(() => result.current.moveDown(3));
    act(() => result.current.moveToFirst());
    expect(result.current.focusedIndex).toBe(0);
  });

  it("moveToLast goes to last index", () => {
    const { result } = renderHook(() => useInspectorNav(defaults));
    act(() => result.current.moveToLast());
    expect(result.current.focusedIndex).toBe(4);
  });

  it("enterEdit switches to edit mode", () => {
    const { result } = renderHook(() => useInspectorNav(defaults));
    act(() => result.current.enterEdit());
    expect(result.current.mode).toBe("edit");
  });

  it("exitEdit returns to normal mode", () => {
    const { result } = renderHook(() => useInspectorNav(defaults));
    act(() => result.current.enterEdit());
    act(() => result.current.exitEdit());
    expect(result.current.mode).toBe("normal");
  });

  it("enterEdit does nothing on empty field list", () => {
    const { result } = renderHook(() => useInspectorNav({ fieldCount: 0 }));
    act(() => result.current.enterEdit());
    expect(result.current.mode).toBe("normal");
  });

  it("setFocusedIndex sets exact position", () => {
    const { result } = renderHook(() => useInspectorNav(defaults));
    act(() => result.current.setFocusedIndex(3));
    expect(result.current.focusedIndex).toBe(3);
  });

  it("setFocusedIndex clamps out-of-bounds", () => {
    const { result } = renderHook(() => useInspectorNav(defaults));
    act(() => result.current.setFocusedIndex(100));
    expect(result.current.focusedIndex).toBe(4);
    act(() => result.current.setFocusedIndex(-5));
    expect(result.current.focusedIndex).toBe(0);
  });

  it("exposes fieldCount from options", () => {
    const { result } = renderHook(() => useInspectorNav(defaults));
    expect(result.current.fieldCount).toBe(5);
  });
});
