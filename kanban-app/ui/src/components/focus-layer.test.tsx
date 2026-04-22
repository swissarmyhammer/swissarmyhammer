import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, renderHook } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { FocusLayer, useFocusLayerKey, FocusLayerContext } from "./focus-layer";
import React, { useContext } from "react";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve()),
  transformCallback: vi.fn(),
}));

// (The ulid mock is no longer needed: FocusLayer now derives keys from
//  React's useId(), not ulid. The imports are kept so old test output
//  continues to read cleanly; removing ulid from production code
//  completed the 01KPVDA8NYFFQ8R1D2G9YEATJ3 fix.)

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
    // Keys are derived from React's useId(): `layer-<name>-<id>`.
    expect(capturedKey!.startsWith("layer-window-")).toBe(true);
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

  it("remount cleanly balances push/remove for the same layer key", () => {
    // Under useId-based keys, a FocusLayer at the same tree position
    // produces the same key on remount — that is the whole point of
    // stability for StrictMode. What matters is that the
    // push/remove invocations are balanced: unmount must remove its
    // layer before remount re-pushes it, so the stack ends with
    // exactly one live entry.
    const { unmount } = render(
      <FocusLayer name="window">
        <div />
      </FocusLayer>,
    );
    unmount();

    render(
      <FocusLayer name="window">
        <div />
      </FocusLayer>,
    );

    const mockInvoke = invoke as ReturnType<typeof vi.fn>;
    const pushes = mockInvoke.mock.calls.filter(
      (c) => c[0] === "spatial_push_layer",
    ).length;
    const removes = mockInvoke.mock.calls.filter(
      (c) => c[0] === "spatial_remove_layer",
    ).length;
    expect(pushes - removes).toBe(1);
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

// ---------------------------------------------------------------------------
// Regression: StrictMode double-push race (bug 01KPVDA8NYFFQ8R1D2G9YEATJ3)
// ---------------------------------------------------------------------------
//
// In development, main.tsx wraps <App/> in <React.StrictMode>, which mounts
// every component twice (mount → unmount → mount) to expose impurities.
// When the layer key + push are both produced inside a `useState(() => ...)`
// initializer, each mount generates a new ULID via `ulid()` and fires a new
// `invoke("spatial_push_layer", {key: NEW_KEY, ...})`. Children render with
// whichever key useState settled on (the first call's return value), but
// Rust's layer stack ends up with one or two *different* keys — depending
// on cleanup ordering.
//
// Concretely: the child rendered with layer_key = K1; Rust's active layer
// is K2. Every child FocusScope registers with `layer_key = K1`. Rust's
// `spatial_search` filters candidates by `entry.layer_key ==
// active.layer_key` → no match → candidate pool empty → nav returns
// Ok(None). This reproduces the user-observable symptom: nav keys log
// `result=null` and focus never moves.
//
// The contract these tests pin: after render in StrictMode, the layer key
// the children SEE must equal the layer key that is LIVE in Rust (the
// `spatial_push_layer` invoke whose matching `spatial_remove_layer` has
// NOT been called). If the test fails, children are looking at a dead key
// and all their spatial registrations are invisible to navigation.

/**
 * Simulate Rust's layer-stack book-keeping from the sequence of invoke
 * calls. Returns the currently-active layer key (top of stack with no
 * pending removal).
 */
function liveLayerKey(mockInvoke: ReturnType<typeof vi.fn>): string | null {
  const stack: string[] = [];
  for (const call of mockInvoke.mock.calls) {
    const [cmd, args] = call as [string, { key: string }];
    if (cmd === "spatial_push_layer") stack.push(args.key);
    else if (cmd === "spatial_remove_layer") {
      const idx = stack.lastIndexOf(args.key);
      if (idx >= 0) stack.splice(idx, 1);
    }
  }
  return stack.length > 0 ? stack[stack.length - 1] : null;
}

describe("FocusLayer under React.StrictMode (regression 01KPVDA8NYFFQ8R1D2G9YEATJ3)", () => {
  it("children see the same layer key that is actually live in Rust", () => {
    let capturedChildKey: string | null = null;
    function Probe() {
      capturedChildKey = useContext(FocusLayerContext);
      return null;
    }

    render(
      <React.StrictMode>
        <FocusLayer name="window">
          <Probe />
        </FocusLayer>
      </React.StrictMode>,
    );

    const liveKey = liveLayerKey(invoke as ReturnType<typeof vi.fn>);

    // This is the bug. If children render with a layer key that isn't
    // the active one in Rust, every `spatial_register` they emit ends up
    // culled by the active-layer filter and navigation silently dies.
    expect(capturedChildKey).toBe(liveKey);
  });

  it("mounts exactly one live layer — no orphan push without a matching remove", () => {
    render(
      <React.StrictMode>
        <FocusLayer name="window">
          <div />
        </FocusLayer>
      </React.StrictMode>,
    );

    const mockInvoke = invoke as ReturnType<typeof vi.fn>;
    const pushKeys = mockInvoke.mock.calls
      .filter((c) => c[0] === "spatial_push_layer")
      .map((c) => (c[1] as { key: string }).key);
    const removeKeys = mockInvoke.mock.calls
      .filter((c) => c[0] === "spatial_remove_layer")
      .map((c) => (c[1] as { key: string }).key);

    // Every push except the live one must have a matching remove. If the
    // StrictMode-double-mount leaks a push, this fails.
    const liveCount = pushKeys.length - removeKeys.length;
    expect(liveCount).toBe(1);
  });

  it("nested layers: innermost layer is the active key, children see the matching active key", () => {
    // Inner probe reads its own FocusLayer context value — the inspector
    // key. This simulates the production topology: <FocusLayer
    // name="window"> wraps the app; <FocusLayer name="inspector">
    // wraps a modal. When the modal is open the inspector layer must be
    // ON TOP of the window layer (active = inspector).
    let innerKey: string | null = null;
    function InnerProbe() {
      innerKey = useContext(FocusLayerContext);
      return null;
    }

    render(
      <React.StrictMode>
        <FocusLayer name="window">
          <FocusLayer name="inspector">
            <InnerProbe />
          </FocusLayer>
        </FocusLayer>
      </React.StrictMode>,
    );

    const liveKey = liveLayerKey(invoke as ReturnType<typeof vi.fn>);

    // The innermost layer (inspector) must be the active one. If
    // children's effects run bottom-up and outer FocusLayer pushes after
    // inner, the stack order inverts and this fails.
    expect(innerKey).toBe(liveKey);
  });
});
