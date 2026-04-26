/**
 * `<FocusZone>` — React peer of the Rust `swissarmyhammer_focus::FocusZone`.
 *
 * A navigable container in the spatial-nav graph. Zones group related
 * `<Focusable>` leaves (e.g. a column of cards, a toolbar of buttons) so the
 * navigator can drill in/out and remember a `last_focused` slot for fallback.
 *
 * This is a **primitive**: it registers with Rust via `spatial_register_zone`,
 * publishes its branded `SpatialKey` via `FocusZoneContext.Provider` so
 * descendants pick it up as their `parent_zone`, and unregisters on unmount.
 * It does NOT bind to an entity moniker chain or create a `CommandScope` —
 * those concerns live in the composite `<FocusScope>` wrapper.
 *
 * On mount the zone reads its enclosing `<FocusLayer>` and (optional) parent
 * `<FocusZone>` from context, mints a fresh `SpatialKey`, and publishes its
 * own bounding rect via `spatial_register_zone`. A ResizeObserver keeps the
 * Rust-side rect in sync as the layout shifts.
 *
 * Focus claim and visual decoration: like `<Focusable>`, the zone subscribes
 * to its own focus claim via `useFocusClaim`. When the Rust kernel marks
 * this zone's key as the focused key for its window, the primitive flips a
 * React `focused` state, renders `data-focused` (output-only), and renders
 * a `<FocusIndicator>` child. Container zones that should not show a bar
 * around their entire body (board, grid, perspective, view) opt out via
 * `showFocusBar={false}`. The visual decoration lives in exactly one place
 * (`<FocusIndicator>`); CSS does not read `[data-focused]`.
 */

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
  type HTMLAttributes,
  type ReactNode,
  type Ref,
} from "react";
import {
  asPixels,
  asSpatialKey,
  type FocusOverrides,
  type Moniker,
  type SpatialKey,
} from "@/types/spatial";
import {
  useFocusClaim,
  useSpatialFocusActions,
} from "@/lib/spatial-focus-context";
import { cn } from "@/lib/utils";
import { useCurrentLayerKey } from "./focus-layer";
import { FocusIndicator } from "./focus-indicator";

// ---------------------------------------------------------------------------
// FocusZoneContext — descendants discover their nearest zone ancestor
// ---------------------------------------------------------------------------

/**
 * The branded `SpatialKey` of the nearest ancestor `<FocusZone>`, or `null`
 * when the descendant is mounted directly under the layer root.
 */
export const FocusZoneContext = createContext<SpatialKey | null>(null);

/**
 * Read the `SpatialKey` of the enclosing `<FocusZone>`, or `null` when no
 * zone wraps the caller.
 *
 * Used by `<Focusable>` and nested `<FocusZone>` instances to populate the
 * `parent_zone` argument of their register calls. A `null` parent is valid:
 * it means the scope is anchored directly at the layer root.
 */
export function useParentZoneKey(): SpatialKey | null {
  return useContext(FocusZoneContext);
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

/** Own props for `<FocusZone>`; standard HTML attributes (className, style, data-*) pass through. */
export interface FocusZoneOwnProps {
  /** Entity moniker for this zone (e.g. `"ui:toolbar.actions"`, `"column:01ABC"`). */
  moniker: Moniker;
  /** Optional per-direction navigation overrides (walls/redirects). */
  navOverride?: FocusOverrides;
  /**
   * When false, suppresses the visible `<FocusIndicator>`. The zone still
   * subscribes to focus claims and renders `data-focused` so tests and
   * e2e selectors keep working — this prop only hides the visual bar.
   *
   * Most container zones (board, grid, perspective, view, nav-bar) want
   * `showFocusBar={false}` because a focus bar around the whole body is
   * visually noisy. Zones that ARE focusable items in their own right
   * (an inspector field row, a column body that should advertise its
   * focus) keep the default of `true`.
   */
  showFocusBar?: boolean;
  /** Children rendered inside the zone container. */
  children: ReactNode;
  /**
   * Optional ref to the rendered `<div>` element. The primitive holds an
   * internal ref for its ResizeObserver and click handler; if you supply
   * one here it is attached alongside that internal ref so callers can
   * reach the same DOM node (e.g. to call `scrollIntoView`). Both
   * `RefObject`-style and callback refs are supported.
   */
  ref?: Ref<HTMLDivElement>;
}

/**
 * Full props for `<FocusZone>` — `FocusZoneOwnProps` + passthrough HTML attrs.
 *
 * `onClick` is intentionally omitted from the passthrough: the primitive owns
 * the click handler so it can call `spatial_focus`. Allowing a consumer to
 * spread their own `onClick` would silently replace the spatial handler (the
 * inline handler is set before `{...rest}`), breaking focus-on-click. This
 * mirrors the same convention applied to `<Focusable>` and the existing
 * `<FocusScope>`.
 */
export type FocusZoneProps = FocusZoneOwnProps &
  Omit<HTMLAttributes<HTMLDivElement>, keyof FocusZoneOwnProps | "onClick">;

/**
 * Mounts a zone in the Rust-side spatial graph and publishes its key via
 * `FocusZoneContext` so descendants register with the correct `parent_zone`.
 *
 * The key is minted once on mount (held in a ref) so it stays stable across
 * re-renders. A ResizeObserver attached to the zone's root element keeps
 * the Rust-side rect in sync; the initial rect is registered alongside the
 * zone in the same `spatial_register_zone` call.
 */
export function FocusZone({
  moniker,
  navOverride,
  showFocusBar = true,
  children,
  ref: externalRef,
  ...rest
}: FocusZoneProps) {
  const keyRef = useRef<SpatialKey | null>(null);
  if (keyRef.current === null) {
    keyRef.current = asSpatialKey(crypto.randomUUID());
  }
  const key = keyRef.current;

  const layerKey = useCurrentLayerKey();
  const parentZone = useParentZoneKey();
  const ref = useRef<HTMLDivElement | null>(null);

  // Subscribe to this zone's focus claim. When the Rust kernel marks this
  // zone's `SpatialKey` as the focused key for its window, `setFocused(true)`
  // fires; on a focus move elsewhere it drops back to false. Like
  // `<Focusable>` this re-renders only when *this zone's* slot flips, so
  // a focus move in a 12k-cell grid wakes at most two primitives (the
  // losing key + the gaining key).
  const [focused, setFocused] = useState(false);
  useFocusClaim(key, setFocused);

  // Callback ref that writes to the internal `ref` (used by the
  // ResizeObserver and click handler) AND forwards to any external ref the
  // caller passed. Memoised on `externalRef` identity so React does not
  // detach/reattach the DOM ref on every render.
  const setRef = useCallback(
    (node: HTMLDivElement | null) => {
      ref.current = node;
      if (typeof externalRef === "function") {
        externalRef(node);
      } else if (externalRef) {
        externalRef.current = node;
      }
    },
    [externalRef],
  );

  const { registerZone, unregisterScope, updateRect, focus } =
    useSpatialFocusActions();

  // ---------------------------------------------------------------------
  // navOverride contract
  // ---------------------------------------------------------------------
  // `navOverride` is read from a ref and snapshotted into the Rust-side
  // registry **only when the registration effect runs** — i.e. on mount
  // and whenever one of (`key`, `moniker`, `layerKey`, `parentZone`) flips
  // identity. Mid-life changes to `navOverride` while those four deps stay
  // stable are intentionally ignored: there is no Tauri command for
  // patching overrides in place, and joining `navOverride` to the dep list
  // would cause an unregister/re-register churn for every parent render
  // that hands us a fresh-identity literal (a common pattern for inline
  // `{ left: null }` props).
  //
  // Callers must therefore treat `navOverride` as effectively-stable for
  // the lifetime of a given (moniker, layerKey, parentZone) tuple. If you
  // genuinely need walls/redirects to flip on the fly, change the
  // `moniker` (e.g. encode the variant into the moniker tail) so the
  // effect re-fires and the latest overrides are pushed to Rust.
  const navOverrideRef = useRef<FocusOverrides | undefined>(navOverride);
  navOverrideRef.current = navOverride;

  useEffect(() => {
    const node = ref.current;
    if (!node) return;

    const overrides: FocusOverrides = navOverrideRef.current ?? {};
    const initialRect = node.getBoundingClientRect();
    registerZone(
      key,
      moniker,
      {
        x: asPixels(initialRect.x),
        y: asPixels(initialRect.y),
        width: asPixels(initialRect.width),
        height: asPixels(initialRect.height),
      },
      layerKey,
      parentZone,
      overrides,
    ).catch((err) => console.error("[FocusZone] register failed", err));

    const observer = new ResizeObserver(() => {
      // Re-read `ref.current` — the observer fires asynchronously and the
      // mounted DOM node may have been swapped (e.g. by a parent re-key)
      // between the initial register call and this resize callback.
      const node = ref.current;
      if (!node) return;
      const r = node.getBoundingClientRect();
      updateRect(key, {
        x: asPixels(r.x),
        y: asPixels(r.y),
        width: asPixels(r.width),
        height: asPixels(r.height),
      }).catch((err) => console.error("[FocusZone] updateRect failed", err));
    });
    observer.observe(node);

    return () => {
      observer.disconnect();
      unregisterScope(key).catch((err) =>
        console.error("[FocusZone] unregister failed", err),
      );
    };
  }, [
    key,
    moniker,
    layerKey,
    parentZone,
    registerZone,
    unregisterScope,
    updateRect,
  ]);

  // Merge `relative` into the consumer's className so the absolutely-
  // positioned `<FocusIndicator>` child positions itself against this
  // zone's box rather than escaping to the nearest ancestor with a
  // containing block. The merge keeps consumer styles intact and adds
  // the positioning hint without forcing every call site to remember it.
  const { className: consumerClassName, ...restWithoutClassName } = rest;
  const mergedClassName = cn(consumerClassName, "relative");

  return (
    <FocusZoneContext.Provider value={key}>
      <div
        ref={setRef}
        data-moniker={moniker}
        data-focused={focused || undefined}
        onClick={(e) => {
          // Match `<Focusable>` semantics: clicking the zone (when nothing
          // inside it absorbed the event) sends focus to the zone itself.
          // Inputs/textareas/contenteditable are spared so editing is not
          // hijacked by spatial focus changes.
          const target = e.target as HTMLElement;
          const tag = target.tagName;
          if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
          if (target.closest("[contenteditable]")) return;
          // Stop here so a click on this zone does not bubble into an
          // enclosing zone (or `<FocusScope>`) and fire `spatial_focus`
          // again with the outer key. Each level handles its own click
          // exactly once. Mirrors the <FocusScope> convention.
          e.stopPropagation();
          focus(key).catch((err) =>
            console.error("[FocusZone] focus failed", err),
          );
        }}
        {...restWithoutClassName}
        className={mergedClassName}
      >
        {showFocusBar && <FocusIndicator focused={focused} />}
        {children}
      </div>
    </FocusZoneContext.Provider>
  );
}
