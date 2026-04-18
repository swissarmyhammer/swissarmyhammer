import { createContext, useContext, useEffect, useRef, type ReactNode } from "react";
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

/** Register/unregister the layer with the Rust spatial layer stack. */
function useLayerRegistration(layerKey: string, name: string) {
  useEffect(() => {
    invoke("spatial_push_layer", { key: layerKey, name }).catch(() => {});
    return () => {
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
