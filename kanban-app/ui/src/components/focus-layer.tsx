import {
  createContext,
  useContext,
  useEffect,
  useRef,
  type ReactNode,
} from "react";
import { ulid } from "ulid";
import { invoke } from "@tauri-apps/api/core";

/**
 * React context carrying the spatial layer key from the nearest FocusLayer.
 * FocusScope reads this to know which layer it belongs to.
 */
export const FocusLayerContext = createContext<string | null>(null);

/** Generate a stable ULID layer key per mount — new on remount. */
function useLayerKey(): string {
  const keyRef = useRef<string | null>(null);
  if (keyRef.current === null) keyRef.current = ulid();
  return keyRef.current;
}

/**
 * Register/unregister the layer with the Rust spatial layer stack.
 *
 * After pushing, schedule a `requestAnimationFrame` to invoke
 * `spatial_focus_first_in_layer` so the layer's upper-left entry claims
 * focus on mount. The RAF delay is essential: descendant `FocusScope`
 * effects run bottom-up during the same commit, so this parent effect
 * fires before any child has registered its rect. The RAF gives React
 * one frame to flush the child registrations before the First-selector
 * runs; without it the layer is always empty at the moment the focus
 * command fires and the call is a silent no-op.
 *
 * Cleanup cancels a still-pending RAF — otherwise a layer that mounts
 * and unmounts inside one frame could call `focus_first_in_layer` after
 * the layer was already popped, racing with `spatial_remove_layer`.
 *
 * The method short-circuits when the focused key is already inside the
 * target layer (see `SpatialState::focus_first_in_layer`), so this is
 * also safe against a user who clicks between the push and the RAF.
 */
function useLayerRegistration(layerKey: string, name: string) {
  useEffect(() => {
    invoke("spatial_push_layer", { key: layerKey, name }).catch(() => {});
    const raf = requestAnimationFrame(() => {
      invoke("spatial_focus_first_in_layer", {
        args: { layerKey },
      }).catch(() => {});
    });
    return () => {
      cancelAnimationFrame(raf);
      invoke("spatial_remove_layer", { key: layerKey }).catch(() => {});
    };
  }, [layerKey, name]);
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
  const layerKey = useLayerKey();
  useLayerRegistration(layerKey, name);

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
