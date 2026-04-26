/**
 * `<FocusLayer>` — React peer of the Rust `swissarmyhammer_focus::FocusLayer`.
 *
 * A modal boundary in the spatial-nav graph. Each Tauri window mounts a root
 * layer (`name="window"`) at the App tree's root; modal surfaces such as
 * inspectors, dialogs, and palettes mount their own nested layer so the
 * Rust-side navigator can scope beam search to the active layer's scopes.
 *
 * Lifecycle:
 *   - Mount: mints a fresh `LayerKey` (unique per instance via
 *     `crypto.randomUUID()`) and invokes `spatial_push_layer` with the
 *     resolved `parent` (explicit prop > nearest ancestor
 *     `FocusLayerContext` > `null` for the window root).
 *   - Unmount: invokes `spatial_pop_layer` to clean up the Rust-side stack.
 *
 * The mounted key is published via `FocusLayerContext.Provider` so descendant
 * primitives (`<FocusZone>`, `<Focusable>`) read it through `useCurrentLayerKey`
 * and pass it to their own register calls.
 *
 * ## What is and is not a layer
 *
 * A surface earns its own [`FocusLayer`] if it supports **multi-element
 * keyboard navigation** — arrow keys move focus between two or more
 * controls inside the surface, and the surface should capture those
 * arrows from anything beneath it. Single-control overlays do not earn a
 * layer because there is nothing for a layer to scope.
 *
 * **Layered surfaces (mount a `<FocusLayer>`):**
 *
 * - `name="window"` — every Tauri webview's React root. One per window.
 * - `name="inspector"` — the inspector panel stack (one layer for all
 *   open panels in a window; each panel is a zone inside that layer).
 * - `name="palette"` — the command palette overlay (the input plus a
 *   navigable list of results — arrows move within the palette only).
 * - `name="dialog"` — confirm / alert dialogs whose body holds two or
 *   more controls (e.g. Confirm + Cancel). Wrap the dialog content in a
 *   `<FocusLayer name="dialog" parentLayerKey={openerKey}>` so arrows
 *   stay inside the dialog and closing the dialog restores focus to the
 *   opener's `last_focused`.
 *
 * **Non-layer transient overlays (intentionally NOT a `<FocusLayer>`):**
 *
 * - **Context menus** (right-click menus). The native menu primitive
 *   owns its own focus loop, dismisses on outside click, and closes on
 *   Escape. There is nothing for the layer model to add — the menu is
 *   not a peer of the surface beneath it; it is a momentary takeover of
 *   the OS-level focus chain that returns to the prior element when the
 *   menu closes.
 * - **Popovers / dropdowns / single-select menus** (`Popover`,
 *   `DropdownMenu`, `Select`). These overlays contain exactly one
 *   interactive control — the chooser itself — so "multi-element
 *   keyboard nav inside the overlay" is meaningless. The chooser owns
 *   its own arrow-key handling for option traversal; arrow keys never
 *   need to escape back to the field beneath because the chooser
 *   intercepts them all.
 * - **Date pickers / calendar popovers** (`Calendar`,
 *   `DateEditor`, `BoardSelector`, `GroupSelector`). Same shape as
 *   single-select: one widget, one focus surface, no need for a layer
 *   boundary above it. Arrow keys traverse days / boards / groups
 *   inside the picker; closing it returns focus to the field that
 *   opened it via the field's own logic, not via layer-pop.
 *
 * The rule of thumb: if you would naturally write a `useEffect` that
 * traps `keydown` for ArrowUp / ArrowDown / Tab to keep focus inside
 * your overlay, you want a `<FocusLayer>`. If the overlay's content is
 * "one thing the user picks", it is not a layer — leave the surrounding
 * layer's `last_focused` untouched and let the picker handle its own
 * keys.
 */

import {
  createContext,
  useContext,
  useEffect,
  useRef,
  type ReactNode,
} from "react";
import { asLayerKey, type LayerKey, type LayerName } from "@/types/spatial";
import { useSpatialFocusActions } from "@/lib/spatial-focus-context";

// ---------------------------------------------------------------------------
// FocusLayerContext — descendants discover their owning layer
// ---------------------------------------------------------------------------

/**
 * The branded `LayerKey` of the nearest ancestor `<FocusLayer>`.
 *
 * `null` outside any layer — `useCurrentLayerKey` (consumed by the leaf
 * primitives) treats that as a hard error since every spatial scope must
 * live inside a layer.
 */
export const FocusLayerContext = createContext<LayerKey | null>(null);

/**
 * Read the `LayerKey` of the enclosing `<FocusLayer>`.
 *
 * Throws when called outside any layer — the spatial-nav contract requires
 * every `<FocusZone>` / `<Focusable>` to be hosted by a layer so the Rust
 * side can route navigation correctly.
 */
export function useCurrentLayerKey(): LayerKey {
  const k = useContext(FocusLayerContext);
  if (!k) {
    throw new Error("useCurrentLayerKey must be called inside a <FocusLayer>");
  }
  return k;
}

/**
 * Read the `LayerKey` of the enclosing `<FocusLayer>`, or `null` when none.
 *
 * Use from primitives (e.g. `<FocusZone>`) that should silently degrade to
 * a no-op when mounted outside the spatial-nav stack — for example, an
 * `<EntityCard>` rendered inside a unit test that does not bother spinning
 * up `<SpatialFocusProvider>` + `<FocusLayer>`. The strict variant
 * (`useCurrentLayerKey`) remains the right choice when the absence of a
 * layer is a contract violation.
 */
export function useOptionalLayerKey(): LayerKey | null {
  return useContext(FocusLayerContext);
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

/** Props for `<FocusLayer>`. */
export interface FocusLayerProps {
  /** Layer role (`"window"`, `"inspector"`, `"dialog"`, `"palette"`). */
  name: LayerName;
  /**
   * Optional override for the parent layer key.
   *
   * Defaults to the nearest ancestor `FocusLayerContext` value. Pass an
   * explicit value when content is portaled out of its React parent (e.g. a
   * dialog rendered into `document.body` whose logical parent layer is the
   * window root rather than the surrounding tree). Pass `null` to deliberately
   * mount this layer at the root, ignoring any ancestor context.
   */
  parentLayerKey?: LayerKey | null;
  children: ReactNode;
}

/**
 * Mounts a layer in the Rust-side stack and exposes its `LayerKey` to
 * descendants via context.
 *
 * The key is generated once on mount (held in a ref) so its identity is
 * stable across re-renders. The push/pop pair fires exactly once for the
 * component's lifetime when the resolved `(name, parent)` tuple does not
 * change; if a caller swaps `name` or `parentLayerKey`, the effect tears
 * the layer down and re-pushes it under the new identity.
 */
export function FocusLayer({
  name,
  parentLayerKey,
  children,
}: FocusLayerProps) {
  const keyRef = useRef<LayerKey | null>(null);
  if (keyRef.current === null) {
    keyRef.current = asLayerKey(crypto.randomUUID());
  }
  const key = keyRef.current;

  const ancestorLayer = useContext(FocusLayerContext);
  // Resolution: explicit prop wins (including `null` for "force-root"); if
  // the prop is `undefined`, fall back to the nearest ancestor (already
  // `null` when no provider wraps us — that's the context's default value).
  const parent: LayerKey | null =
    parentLayerKey !== undefined ? parentLayerKey : ancestorLayer;

  const { pushLayer, popLayer } = useSpatialFocusActions();

  useEffect(() => {
    pushLayer(key, name, parent).catch((err) => {
      console.error("[FocusLayer] push failed", err);
    });
    return () => {
      popLayer(key).catch((err) => {
        console.error("[FocusLayer] pop failed", err);
      });
    };
  }, [key, name, parent, pushLayer, popLayer]);

  return (
    <FocusLayerContext.Provider value={key}>
      {children}
    </FocusLayerContext.Provider>
  );
}
