/**
 * `<Focusable>` — React peer of the Rust `swissarmyhammer_focus::Focusable`.
 *
 * A leaf focus point in the spatial-nav graph. Each `<Focusable>` registers
 * exactly one entry on the Rust side via `spatial_register_focusable`,
 * subscribes to focus claims for its key via `useFocusClaim`, and unregisters
 * on unmount.
 *
 * This is a **primitive**: no entity binding, no `CommandScope`, no
 * context-menu plumbing. The composite `<FocusScope>` wrapper layers those
 * concerns on top for entity-bound use cases. Use `<Focusable>` directly for
 * non-entity chrome (toolbar buttons, navigation chevrons, etc.) — anywhere
 * you need a single keystroke target without the rest of the entity stack.
 *
 * Lifecycle:
 *   - Mount: mints a fresh `SpatialKey`, reads its enclosing `<FocusLayer>`
 *     and (optional) `<FocusZone>` from context, snapshots the bounding rect,
 *     and invokes `spatial_register_focusable`.
 *   - Resize: a ResizeObserver attached to the root element pushes rect
 *     deltas via `spatial_update_rect`.
 *   - Click: invokes `spatial_focus` to mark this key as focused. Inputs and
 *     contenteditable subtrees are spared so editing is not hijacked.
 *   - Focus claim: `useFocusClaim` subscribes to the per-key boolean stream
 *     so the wrapper renders `data-focused` toggling without re-rendering
 *     the entire tree on every focus move elsewhere.
 *   - Unmount: invokes `spatial_unregister_scope` and disconnects the
 *     ResizeObserver.
 *
 * Visual decoration: the primitive renders a single `<FocusIndicator>` as
 * its first child. The indicator paints the visible focus bar from the
 * primitive's React `focused` state — there is no CSS rule that reads
 * `[data-focused]` to draw a bar, and no second component anywhere else
 * that renders the same decoration. The `data-focused` attribute is kept
 * as an output-only debugging / e2e hook; nothing reads it back as state.
 */

import {
  useCallback,
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
import { useParentZoneKey } from "./focus-zone";
import { FocusIndicator } from "./focus-indicator";

/** Own props for `<Focusable>`; standard HTML attributes (className, style, data-*) pass through. */
export interface FocusableOwnProps {
  /** Entity moniker for this focusable (e.g. `"task:01ABC"`, `"ui:toolbar.new"`). */
  moniker: Moniker;
  /** Optional per-direction navigation overrides (walls/redirects). */
  navOverride?: FocusOverrides;
  /**
   * When false, suppresses the visible `<FocusIndicator>`. The primitive
   * still subscribes to focus claims and renders `data-focused` so tests
   * and e2e selectors keep working — this prop only hides the visual
   * bar. Useful for entity scopes whose focus bar would clutter the
   * surrounding UI (e.g. the inspector entity scope, where the panel
   * itself is the visual cue).
   *
   * Defaults to true.
   */
  showFocusBar?: boolean;
  /** Children rendered inside the focusable container. */
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
 * Full props for `<Focusable>` — `FocusableOwnProps` + passthrough HTML attrs.
 *
 * `onClick` is intentionally omitted from the passthrough: the primitive owns
 * the click handler so it can call `spatial_focus`. Allowing a consumer to
 * spread their own `onClick` would silently replace the spatial handler (the
 * inline handler is set before `{...rest}`), breaking focus-on-click. Wrap
 * the consumer's element instead, or attach click logic at a layer the
 * primitive does not control. This mirrors the existing `<FocusScope>`
 * convention.
 */
export type FocusableProps = FocusableOwnProps &
  Omit<HTMLAttributes<HTMLDivElement>, keyof FocusableOwnProps | "onClick">;

/**
 * Mounts a leaf focusable in the Rust-side spatial graph.
 *
 * The key is minted once on mount (held in a ref) so it stays stable across
 * re-renders. A ResizeObserver attached to the root element keeps the
 * Rust-side rect in sync. The component re-renders only when its own focus
 * claim flips — the rest of the tree is unaffected by focus moves elsewhere
 * thanks to the per-key claim registry.
 */
export function Focusable({
  moniker,
  navOverride,
  showFocusBar = true,
  children,
  ref: externalRef,
  ...rest
}: FocusableProps) {
  const keyRef = useRef<SpatialKey | null>(null);
  if (keyRef.current === null) {
    keyRef.current = asSpatialKey(crypto.randomUUID());
  }
  const key = keyRef.current;

  const layerKey = useCurrentLayerKey();
  const parentZone = useParentZoneKey();
  const ref = useRef<HTMLDivElement | null>(null);

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

  const [focused, setFocused] = useState(false);
  useFocusClaim(key, setFocused);

  const { registerFocusable, unregisterScope, updateRect, focus } =
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
    registerFocusable(
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
    ).catch((err) => console.error("[Focusable] register failed", err));

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
      }).catch((err) => console.error("[Focusable] updateRect failed", err));
    });
    observer.observe(node);

    return () => {
      observer.disconnect();
      unregisterScope(key).catch((err) =>
        console.error("[Focusable] unregister failed", err),
      );
    };
  }, [
    key,
    moniker,
    layerKey,
    parentZone,
    registerFocusable,
    unregisterScope,
    updateRect,
  ]);

  // Merge `relative` into the consumer's className. `relative` is required
  // so the absolutely-positioned `<FocusIndicator>` child positions itself
  // against the primitive's box rather than escaping to the nearest
  // ancestor with a containing block. The merge keeps consumer styles
  // (e.g. `<Focusable className="flex flex-col">`) intact and adds the
  // positioning hint without forcing every call site to remember it.
  const { className: consumerClassName, ...restWithoutClassName } = rest;
  const mergedClassName = cn(consumerClassName, "relative");

  return (
    <div
      ref={setRef}
      data-moniker={moniker}
      data-focused={focused || undefined}
      onClick={(e) => {
        // Skip when the click landed on an editable surface — letting the
        // editor own the click avoids stealing caret placement from the user.
        const target = e.target as HTMLElement;
        const tag = target.tagName;
        if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
        if (target.closest("[contenteditable]")) return;
        // Stop here: a leaf click must not bubble to an enclosing
        // <FocusZone> (or <FocusScope>) and fire `spatial_focus` again
        // with the ancestor's key — that would clobber the user's
        // intent. Mirrors the long-standing <FocusScope> convention.
        e.stopPropagation();
        focus(key).catch((err) =>
          console.error("[Focusable] focus failed", err),
        );
      }}
      {...restWithoutClassName}
      className={mergedClassName}
    >
      {showFocusBar && <FocusIndicator focused={focused} />}
      {children}
    </div>
  );
}
