import { describe, it, expect } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useBoardNav } from "./use-board-nav";

describe("useBoardNav", () => {
  it("initializes in normal mode", () => {
    const { result } = renderHook(() => useBoardNav());
    expect(result.current.mode).toBe("normal");
  });

  it("enterEdit switches to edit mode", () => {
    const { result } = renderHook(() => useBoardNav());
    act(() => result.current.enterEdit());
    expect(result.current.mode).toBe("edit");
  });

  it("exitEdit returns to normal mode", () => {
    const { result } = renderHook(() => useBoardNav());
    act(() => result.current.enterEdit());
    act(() => result.current.exitEdit());
    expect(result.current.mode).toBe("normal");
  });
});
