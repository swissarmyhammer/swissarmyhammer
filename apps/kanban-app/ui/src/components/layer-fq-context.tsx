/**
 * `LayerFqContext` — carries the FQM of the enclosing `<FocusLayer>` to
 * descendant zones and scopes.
 *
 * The kernel's `spatial_register_*` calls take both the zone/scope's
 * own FQM and the owning layer's FQM. The layer FQM is needed
 * separately so the kernel can scope cascade resolution to a single
 * layer's scopes; `FullyQualifiedMonikerContext` carries the *immediate*
 * primitive's FQM, which is not the same as the nearest ancestor layer
 * for nested zones.
 *
 * Lives in its own module to avoid a `focus-zone.tsx` ↔ `focus-layer.tsx`
 * import cycle.
 */

import { createContext, useContext } from "react";
import type { FullyQualifiedMoniker } from "@/types/spatial";

/**
 * The `FullyQualifiedMoniker` of the nearest ancestor `<FocusLayer>`,
 * or `null` when no layer wraps the caller.
 *
 * Provided by every `<FocusLayer>` so descendant zones and scopes can
 * thread the layer FQM into their `spatial_register_*` payloads.
 */
export const LayerFqContext = createContext<FullyQualifiedMoniker | null>(null);

/**
 * Read the FQM of the enclosing `<FocusLayer>`.
 *
 * Throws when called outside any layer — the spatial-nav contract
 * requires every `<FocusZone>` / `<FocusScope>` registration to be
 * scoped to a layer. The fallback body in `<FocusZone>` /
 * `<FocusScope>` (no-spatial-context branch) does NOT call this hook,
 * so a missing layer in a test that mounts a single primitive without
 * `<SpatialFocusProvider>` is tolerated by skipping the spatial
 * registration entirely.
 */
export function useEnclosingLayerFq(): FullyQualifiedMoniker {
  const fq = useContext(LayerFqContext);
  if (fq === null) {
    throw new Error("useEnclosingLayerFq must be called inside a <FocusLayer>");
  }
  return fq;
}

/**
 * Read the FQM of the enclosing `<FocusLayer>`, or `null` when no layer
 * wraps the caller.
 *
 * Used by `<FocusZone>` and `<FocusScope>` to detect whether they are
 * mounted inside the spatial-nav stack: a non-null layer FQ means the
 * primitive should register with the kernel; `null` means it should
 * fall back to the entity-focus-only path.
 */
export function useOptionalEnclosingLayerFq(): FullyQualifiedMoniker | null {
  return useContext(LayerFqContext);
}
