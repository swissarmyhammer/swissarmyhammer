import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, renderHook } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { FocusLayer, useFocusLayerKey, FocusLayerContext } from "./focus-layer";
import { useContext } from "react";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve()),
  transformCallback: vi.fn(),
}));

vi.mock("ulid", () => {
  let counter = 0;
  return { ulid: vi.fn(() => "01LAYER" + String(++counter).padStart(19, "0")) };
});

beforeEach(() => {
  vi.clearAllMocks();
});

describe("FocusLayer", () => {
  it("provides a layer key to children via context", () => {
    let capturedKey: string | null = null;
    function Probe() {
      capturedKey = useContext(FocusLayerContext);
      return null;
    }

    render(
      <FocusLayer name="window">
        <Probe />
      </FocusLayer>,
    );

    expect(capturedKey).toBeTruthy();
    expect(capturedKey!.startsWith("01LAYER")).toBe(true);
  });

  it("invokes spatial_push_layer on mount with key and name", () => {
    render(
      <FocusLayer name="inspector">
        <div />
      </FocusLayer>,
    );

    expect(invoke).toHaveBeenCalledWith(
      "spatial_push_layer",
      expect.objectContaining({
        key: expect.any(String),
        name: "inspector",
      }),
    );
  });

  it("invokes spatial_remove_layer on unmount", () => {
    const { unmount } = render(
      <FocusLayer name="dialog">
        <div />
      </FocusLayer>,
    );

    // Capture the key used for push
    const pushCall = (invoke as ReturnType<typeof vi.fn>).mock.calls.find(
      (c: unknown[]) => c[0] === "spatial_push_layer",
    );
    const key = (pushCall![1] as { key: string }).key;

    vi.clearAllMocks();
    unmount();

    expect(invoke).toHaveBeenCalledWith(
      "spatial_remove_layer",
      expect.objectContaining({ key }),
    );
  });

  it("generates a new layer key on remount", () => {
    const keys: string[] = [];
    function Probe() {
      const key = useContext(FocusLayerContext);
      if (key) keys.push(key);
      return null;
    }

    const { unmount } = render(
      <FocusLayer name="window">
        <Probe />
      </FocusLayer>,
    );
    unmount();

    render(
      <FocusLayer name="window">
        <Probe />
      </FocusLayer>,
    );

    expect(keys.length).toBeGreaterThanOrEqual(2);
    expect(keys[0]).not.toBe(keys[keys.length - 1]);
  });
});

describe("useFocusLayerKey", () => {
  it("returns the layer key inside a FocusLayer", () => {
    const wrapper = ({ children }: { children: React.ReactNode }) => (
      <FocusLayer name="test">{children}</FocusLayer>
    );

    const { result } = renderHook(() => useFocusLayerKey(), { wrapper });
    expect(result.current).toBeTruthy();
    expect(typeof result.current).toBe("string");
  });

  it("returns null outside a FocusLayer", () => {
    const { result } = renderHook(() => useFocusLayerKey());
    expect(result.current).toBeNull();
  });
});
