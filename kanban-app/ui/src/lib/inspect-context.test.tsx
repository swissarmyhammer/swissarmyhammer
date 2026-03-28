import { describe, it, expect, vi } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { InspectProvider, useInspect } from "./inspect-context";

describe("useInspect", () => {
  it("throws outside provider", () => {
    expect(() => renderHook(() => useInspect())).toThrow(
      "useInspect must be used within an InspectProvider",
    );
  });

  it("parses moniker and calls onInspect", () => {
    const onInspect = vi.fn();
    const wrapper = ({ children }: { children: React.ReactNode }) => (
      <InspectProvider onInspect={onInspect} onDismiss={() => false}>
        {children}
      </InspectProvider>
    );
    const { result } = renderHook(() => useInspect(), { wrapper });
    act(() => {
      result.current("task:abc");
    });
    expect(onInspect).toHaveBeenCalledWith("task", "abc");
  });
});
