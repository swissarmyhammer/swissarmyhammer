import {
  createContext,
  useContext,
  useEffect,
  useId,
  useState,
  type ReactNode,
} from "react";
import { invoke } from "@tauri-apps/api/core";

/**
 * React context carrying the spatial layer key from the nearest FocusLayer.
 * FocusScope reads this to know which layer it belongs to.
 */
export const FocusLayerContext = createContext<string | null>(null);

/**
 * Generate a stable layer key and push / remove it against the Rust
 * layer stack across the component's lifecycle.
 *
 * ## Design constraints
 *
 * 1. **Ordering.** Nested `<FocusLayer>`s must push in outer-to-inner
 *    order so the innermost layer is on top of the stack. Children
 *    must see their enclosing layer's key through `FocusLayerContext`
 *    *at the moment they register spatial scopes*. Effects run
 *    bottom-up (children before parents), so pushing in `useEffect`
 *    inverts the stack — inspector ends up below window, and window
 *    (not inspector) becomes active. The regression test
 *    `nested layers: innermost layer is the active key, children see
 *    the matching active key` pins this invariant.
 *
 * 2. **StrictMode purity.** `<React.StrictMode>` (enabled in
 *    `main.tsx`) double-invokes `useState` initializers, component
 *    bodies, and the whole mount cycle (mount → unmount → mount) to
 *    expose side-effect impurity. The key must be **deterministic**
 *    across every StrictMode invocation for the same tree position —
 *    non-pure generators like `ulid()` produced a new key on each
 *    invocation and pushed a second unreachable layer on top of the
 *    real one, which is why every `nav.*` returned `Ok(None)` in dev
 *    builds (the candidate-pool-empty bug). Pinned by the regression
 *    test `children see the same layer key that is actually live in
 *    Rust`.
 *
 * ## How this implementation satisfies both
 *
 * - `useId()` gives a key that is stable for a given React tree
 *   position across all StrictMode invocations (double-render,
 *   double-init, mount-unmount-mount). The same component instance at
 *   the same spot in the tree gets the same key every time, so every
 *   push targets the same key.
 * - The push lives in a `useState` initializer so it runs **during
 *   render**, which is top-down (parent before child). Outer
 *   `<FocusLayer>`s push before inner ones, stack order matches DOM
 *   tree order.
 * - `spatial_push_layer` is idempotent on the Rust side: pushing a
 *   key that is already on the stack is a no-op. StrictMode's
 *   double-invoke of the initializer pushes the same key twice, which
 *   collapses to a single stack entry. The mount-unmount-mount cycle
 *   removes and re-pushes the same key, again landing in the same
 *   stable state.
 * - The remove lives in a `useEffect` cleanup so React's normal
 *   unmount path (including StrictMode's simulated unmount) removes
 *   the layer once. Remove is unconditionally idempotent already
 *   (`LayerStack::remove` retains by key).
 */
function useLayerKeyAndPush(name: string): string {
  const id = useId();
  // Sanitize colons out of React's id so the key passes through Rust
  // command argument parsing cleanly. `useId` may return values like
  // `:r0:` — strip the colons to a plain `rN` suffix.
  const layerKey = `layer-${name}-${id.replace(/:/g, "")}`;

  useState<boolean>(() => {
    invoke("spatial_push_layer", { key: layerKey, name }).catch((err) => {
      console.warn(
        "[FocusLayer] spatial_push_layer failed",
        name,
        layerKey,
        err,
      );
    });
    return true;
  });

  useEffect(() => {
    return () => {
      invoke("spatial_remove_layer", { key: layerKey }).catch((err) => {
        console.warn("[FocusLayer] spatial_remove_layer failed", layerKey, err);
      });
    };
  }, [layerKey]);

  return layerKey;
}

/**
 * Schedule the initial first-in-layer focus via `requestAnimationFrame`.
 *
 * The RAF delay lets descendant `FocusScope` effects register their
 * rects before the First-selector runs; without the one-frame
 * deferral the layer is still empty when the focus command fires, and
 * the call silently no-ops. Cleanup cancels any still-pending RAF so
 * a layer that mounts and unmounts inside one frame cannot leak the
 * focus-first invocation.
 *
 * `spatial_focus_first_in_layer` short-circuits when the focused key
 * already belongs to the target layer (safe against clicks between
 * push and RAF) and when the layer is no longer the active one
 * (safe against stale RAFs from lower layers).
 *
 * Note: the layer's `spatial_remove_layer` lives inside
 * `useLayerKeyAndPush`'s useEffect cleanup — co-located with the push
 * so the two invokes share a single effect boundary and React's
 * mount-unmount-mount StrictMode pattern naturally balances them.
 */
function useLayerLifecycle(layerKey: string) {
  useEffect(() => {
    const raf = requestAnimationFrame(() => {
      invoke("spatial_focus_first_in_layer", {
        args: { layerKey },
      }).catch((err) => {
        console.warn(
          "[FocusLayer] spatial_focus_first_in_layer failed",
          layerKey,
          err,
        );
      });
    });
    return () => {
      cancelAnimationFrame(raf);
    };
  }, [layerKey]);
}

/**
 * Declares a focus layer boundary. Navigation stays within the active
 * (topmost) layer — entries in other layers are invisible to `navigate()`.
 *
 * Every FocusScope must be inside a FocusLayer. The app root should be
 * wrapped in `<FocusLayer name="window">`.
 *
 * @example
 * ```tsx
 * <FocusLayer name="window">
 *   <Board />
 *   {inspectorOpen && (
 *     <FocusLayer name="inspector">
 *       <Inspector />
 *     </FocusLayer>
 *   )}
 * </FocusLayer>
 * ```
 */
export function FocusLayer({
  name,
  children,
}: {
  name: string;
  children: ReactNode;
}) {
  const layerKey = useLayerKeyAndPush(name);
  useLayerLifecycle(layerKey);

  return (
    <FocusLayerContext.Provider value={layerKey}>
      {children}
    </FocusLayerContext.Provider>
  );
}

/**
 * Returns the layer key from the nearest FocusLayer ancestor, or `null`
 * if no FocusLayer is present. When `null`, FocusScope skips spatial
 * registration (useful in tests that don't wrap in FocusLayer).
 */
export function useFocusLayerKey(): string | null {
  return useContext(FocusLayerContext);
}
