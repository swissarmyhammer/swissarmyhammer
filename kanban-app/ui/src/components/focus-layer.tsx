import {
  createContext,
  useContext,
  useEffect,
  useState,
  type ReactNode,
} from "react";
import { ulid } from "ulid";
import { invoke } from "@tauri-apps/api/core";

/**
 * React context carrying the spatial layer key from the nearest FocusLayer.
 * FocusScope reads this to know which layer it belongs to.
 */
export const FocusLayerContext = createContext<string | null>(null);

/**
 * Generate a stable layer key and push it onto the Rust layer stack
 * in the same step — during React's initial render of the component.
 *
 * The push MUST happen during render (via `useState`'s initializer)
 * rather than in a `useEffect`, because React flushes effects
 * bottom-up (children before parents) but renders top-down (parents
 * before children). Registering the layer in a `useEffect` would make
 * outer `FocusLayer`s push *after* their inner children, inverting the
 * stack order so the outermost layer ends up on top. For the
 * single-layer-at-a-time case (one inspector opens per user action)
 * that was invisible, but for the multi-layer-mount-at-once case
 * (three inspectors mounted on initial render — see
 * `spatial-nav-multi-inspector.test.tsx`) it flipped which layer was
 * "active", and the topmost inspector's first field never received
 * focus because `spatial_focus_first_in_layer` guards against being
 * invoked on a non-active layer.
 *
 * Running the initializer from render is safe for our use: React
 * invokes `useState` initializers exactly once per component instance,
 * and the Rust `spatial_push_layer` command is idempotent-by-key (the
 * ULID we generate here never repeats). A `useEffect` cleanup
 * elsewhere in this component removes the layer on unmount so the
 * stack stays consistent with the React tree.
 */
function useLayerKeyAndPush(name: string): string {
  const [layerKey] = useState<string>(() => {
    const key = ulid();
    invoke("spatial_push_layer", { key, name }).catch(() => {});
    return key;
  });
  return layerKey;
}

/**
 * Tear down the layer on unmount, and schedule the initial
 * first-in-layer focus via `requestAnimationFrame`.
 *
 * The RAF delay lets descendant `FocusScope` effects register their
 * rects before the First-selector runs; without the one-frame
 * deferral the layer is still empty when the focus command fires, and
 * the call silently no-ops. Cleanup cancels any still-pending RAF so
 * a layer that mounts and unmounts inside one frame cannot race with
 * `spatial_remove_layer`.
 *
 * `spatial_focus_first_in_layer` short-circuits when the focused key
 * already belongs to the target layer (safe against clicks between
 * push and RAF) and when the layer is no longer the active one
 * (safe against stale RAFs from lower layers).
 */
function useLayerLifecycle(layerKey: string) {
  useEffect(() => {
    const raf = requestAnimationFrame(() => {
      invoke("spatial_focus_first_in_layer", {
        args: { layerKey },
      }).catch(() => {});
    });
    return () => {
      cancelAnimationFrame(raf);
      invoke("spatial_remove_layer", { key: layerKey }).catch(() => {});
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
