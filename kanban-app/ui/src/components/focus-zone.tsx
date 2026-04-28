/**
 * `<FocusZone>` — the navigable-container peer in the spatial-nav graph and
 * the entity-aware composite that production zone wraps use.
 *
 * `<FocusZone>` is a **pure spatial primitive**. It does NOT know about
 * inspectable entities and does NOT dispatch `ui.inspect`. Inspector
 * dispatch lives in `<Inspectable>` — see `inspectable.tsx`. Wrap an
 * entity subtree in `<Inspectable>` to make double-click open the
 * inspector; do not look for an `inspectOnDoubleClick` prop here.
 *
 * Three peers, not four: the spatial-nav kernel exposes `<FocusLayer>`
 * (modal boundary), `<FocusZone>` (navigable container), and `<FocusScope>`
 * (leaf). This component is the zone — a parent of leaves and nested
 * zones in the spatial graph, and an entity-bound surface in the
 * command-scope / context-menu chain.
 *
 *   - Mints a stable `SpatialKey` per mount and registers with Rust via
 *     `spatial_register_zone`.
 *   - Publishes its `SpatialKey` via `FocusZoneContext.Provider` so
 *     descendant scopes pick it up as their `parent_zone`.
 *   - Subscribes to per-key focus claims through `useFocusClaim` so its
 *     `data-focused` attribute and the visible `<FocusIndicator>` flip
 *     when this key becomes focused.
 *   - Handles click → `spatial_focus`, with editable surfaces (inputs,
 *     contenteditable) spared so caret placement is not stolen.
 *     `handleEvents={false}` opts out of click / right-click ownership
 *     when an enclosing primitive already owns them (e.g. a grid-cell
 *     `<FocusScope>` that is the cursor target).
 *   - Right-click → `setFocus(moniker)` + native context menu via
 *     `useContextMenu`.
 *   - Pushes a `CommandScopeContext.Provider` so descendants participate
 *     in command resolution and the context-menu chain.
 *   - Pushes a `FocusScopeContext.Provider` so descendants discover their
 *     nearest enclosing entity scope without walking the command-scope
 *     chain.
 *   - Registers with the entity-focus scope registry so `useFocusedScope`
 *     and the dispatcher can compute scope chains.
 *   - Optional `navOverride` per-direction directives forwarded into the
 *     Rust-side registry. The kernel runs these as rule 0 of beam search:
 *     a same-layer target moniker redirects, `null` blocks navigation in
 *     that direction, missing keys fall through to beam search.
 *   - `scrollIntoView` when the entity-focus store reports this zone as
 *     directly focused — preserves the legacy "follow the focus bar"
 *     scroll behavior.
 *
 * For leaves (a tag pill, a title field, a toolbar button), use
 * `<FocusScope>` directly. For modal boundaries (window root, inspector,
 * dialog), use `<FocusLayer>` directly.
 *
 * # Lifecycle
 *
 *   - **Mount**: mints a fresh `SpatialKey`, reads its enclosing
 *     `<FocusLayer>` and (optional) parent `<FocusZone>` from context,
 *     snapshots the bounding rect, invokes `spatial_register_zone`, and
 *     registers itself in the entity-focus registry.
 *   - **Resize**: a ResizeObserver attached to the root element pushes
 *     rect deltas via `spatial_update_rect`.
 *   - **Ancestor scroll**: a passive `scroll` listener (per-rAF
 *     throttled) on every scrollable ancestor and on `window` pushes
 *     fresh rects via `spatial_update_rect`. Without this, scrolling
 *     a column container would shift the zone's viewport-y while the
 *     kernel kept its mount-time rect, and beam-search would run on
 *     stale geometry.
 *   - **Click / right-click / double-click**: see above.
 *   - **Focus claim**: `useFocusClaim` subscribes to the per-key boolean
 *     stream so the wrapper renders `data-focused` toggling without
 *     re-rendering the entire tree on every focus move elsewhere.
 *   - **Unmount**: invokes `spatial_unregister_scope`, disconnects the
 *     ResizeObserver, and unregisters from the entity-focus registry.
 *
 * # Optional providers
 *
 * Two independent ancestors gate the chrome:
 *
 *   - `<FocusLayer>` — when missing, the component skips the spatial
 *     registration entirely (no `spatial_register_zone`, no focus-claim
 *     subscription, no visible `<FocusIndicator>`) and degrades to a
 *     plain `<div>` with the entity-focus chrome only. Tests that mount
 *     one component at a time without standing up `<SpatialFocusProvider>`
 *     exercise this path.
 *   - `<EntityFocusProvider>` — when missing, the entity-focus
 *     scope-registry registration and the `scrollIntoView` effect are
 *     skipped. The CommandScope / FocusScopeContext providers and the
 *     spatial-nav chrome (when a layer is present) all keep working.
 *
 * Production code (`App.tsx` and the quick-capture window) always mounts
 * both providers, so neither degraded branch is ever reached at runtime.
 */

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type HTMLAttributes,
  type ReactNode,
  type Ref,
} from "react";
import {
  CommandScopeContext,
  EMPTY_COMMANDS,
  type CommandDef,
  type CommandScope,
} from "@/lib/command-scope";
import {
  useEntityScopeRegistration,
  useOptionalFocusActions,
  useOptionalIsDirectFocus,
} from "@/lib/entity-focus-context";
import { useContextMenu } from "@/lib/context-menu";
import {
  useFocusClaim,
  useSpatialFocusActions,
} from "@/lib/spatial-focus-context";
import { cn } from "@/lib/utils";
import { useFocusDebug } from "@/lib/focus-debug-context";
import { FocusLayerContext } from "@/components/focus-layer";
import { FocusDebugOverlay } from "@/components/focus-debug-overlay";
import { FocusIndicator } from "@/components/focus-indicator";
import { FocusScopeContext } from "@/components/focus-scope-context";
import { useTrackRectOnAncestorScroll } from "@/components/use-track-rect-on-ancestor-scroll";
import {
  asPixels,
  asSpatialKey,
  type FocusOverrides,
  type LayerKey,
  type Moniker,
  type SpatialKey,
} from "@/types/spatial";

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
 * Used by `<FocusScope>` and nested `<FocusZone>` instances to populate the
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
   * Commands to register in this zone's `CommandScope`. Optional —
   * defaults to the shared `EMPTY_COMMANDS` constant. Most zones exist
   * purely to register a moniker in the focus / scope chain and have
   * no per-zone commands of their own; those callers should simply omit
   * the prop. Only pass an array when the zone genuinely contributes
   * commands (e.g. `extraCommands` forwarded from a parent surface).
   */
  commands?: readonly CommandDef[];
  /**
   * When false, suppresses both:
   *   1. The visible `<FocusIndicator>` rendered by the primitive (the
   *      `data-focused` attribute and the focus-claim subscription stay
   *      active so tests / e2e selectors keep working).
   *   2. The entity-focus-driven `scrollIntoView` effect (the legacy
   *      "follow the focus bar" scroll).
   *
   * Most container zones (board, grid, perspective, view, nav-bar) want
   * `showFocusBar={false}` because a focus bar around the whole body is
   * visually noisy. Zones that ARE focusable items in their own right
   * (an inspector field row, a column body that should advertise its
   * focus) keep the default of `true`.
   */
  showFocusBar?: boolean;
  /**
   * When false, suppresses click / right-click / double-click event
   * handling on the zone's outer `<div>`. Independent of `showFocusBar`
   * — a zone can register and emit `data-focused` without owning clicks.
   *
   * Use this when an enclosing primitive already owns the click semantics
   * for this region (e.g. a grid cell `<FocusScope>` whose `grid_cell:R:K`
   * leaf is the cursor target — the inner Field-as-zone must not steal
   * the click away from the cell). Right-click and double-click still
   * propagate because the consumer-owned wrap will dispatch them.
   *
   * Defaults to true. Mirrors the `<FocusScope>` prop of the same name.
   */
  handleEvents?: boolean;
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
 * matches the convention `<FocusScope>` uses.
 */
export type FocusZoneProps = FocusZoneOwnProps &
  Omit<HTMLAttributes<HTMLDivElement>, keyof FocusZoneOwnProps | "onClick">;

/**
 * Mounts an entity-bound zone in the Rust-side spatial graph and publishes
 * its key via `FocusZoneContext` so descendants register with the correct
 * `parent_zone`.
 *
 * The key is minted once on mount (held in a ref) so it stays stable across
 * re-renders. A ResizeObserver attached to the zone's root element keeps
 * the Rust-side rect in sync; the initial rect is registered alongside the
 * zone in the same `spatial_register_zone` call.
 */
export function FocusZone({
  moniker,
  navOverride,
  commands = EMPTY_COMMANDS,
  showFocusBar = true,
  handleEvents = true,
  children,
  ref: externalRef,
  ...rest
}: FocusZoneProps) {
  // Selective subscription: re-renders only when *this zone's* moniker
  // flips focus. Drives the `scrollIntoView` effect in the body branches.
  // Returns `false` permanently when no `EntityFocusProvider` is mounted.
  const isFocused = useOptionalIsDirectFocus(moniker);

  // Build the scope ourselves so we can register it in the entity-focus
  // registry. Same shape as `<FocusScope>` produces, with `moniker`
  // anchoring the chain.
  const parent = useContext(CommandScopeContext);
  const scope = useMemo<CommandScope>(() => {
    const map = new Map<string, CommandDef>();
    for (const cmd of commands) {
      map.set(cmd.id, cmd);
    }
    return { commands: map, parent, moniker };
  }, [commands, parent, moniker]);

  const isDirectFocus = showFocusBar && isFocused;

  // Register the scope in the entity-focus registry via the shared helper —
  // same identity-churn-tolerant pattern `<FocusScope>` uses, with full
  // tolerance for a missing `EntityFocusProvider`.
  useEntityScopeRegistration(moniker, scope);

  // Detect whether a `<FocusLayer>` ancestor is mounted. Production code
  // (App.tsx and the quick-capture window) wraps everything in one, but
  // many isolated tests render zones without the spatial-focus provider
  // stack. When no layer is present we degrade to a plain `<div>` — the
  // entity-focus chrome (CommandScope, claim registry, right-click,
  // double-click) still works, only the spatial registration is skipped.
  //
  // The two body branches are siblings rather than one branch reading the
  // context conditionally because the spatial-context body calls hooks
  // (`useFocusClaim`, `useSpatialFocusActions`, `useParentZoneKey`) that
  // throw when the matching provider is missing. Splitting them keeps the
  // hook count stable per branch while letting the no-spatial-context
  // path skip those hooks entirely.
  const layerKey = useContext(FocusLayerContext);
  const hasSpatialContext = layerKey !== null;

  return (
    <FocusScopeContext.Provider value={moniker}>
      <CommandScopeContext.Provider value={scope}>
        {hasSpatialContext ? (
          <SpatialFocusZoneBody
            moniker={moniker}
            navOverride={navOverride}
            showFocusBar={showFocusBar}
            handleEvents={handleEvents}
            isDirectFocus={isDirectFocus}
            layerKey={layerKey}
            ref={externalRef}
            {...rest}
          >
            {children}
          </SpatialFocusZoneBody>
        ) : (
          <FallbackFocusZoneBody
            moniker={moniker}
            handleEvents={handleEvents}
            isDirectFocus={isDirectFocus}
            ref={externalRef}
            {...rest}
          >
            {children}
          </FallbackFocusZoneBody>
        )}
      </CommandScopeContext.Provider>
    </FocusScopeContext.Provider>
  );
}

// ---------------------------------------------------------------------------
// Body — spatial-context branch
// ---------------------------------------------------------------------------

/** Props for the spatial-context body. */
interface SpatialFocusZoneBodyProps extends Omit<
  HTMLAttributes<HTMLDivElement>,
  "onClick" | "children"
> {
  moniker: Moniker;
  navOverride?: FocusOverrides;
  showFocusBar: boolean;
  /**
   * When false, the body skips click / right-click handling (the
   * spatial primitive still registers and subscribes to claims, but the
   * outer `<div>` carries no event listeners).
   */
  handleEvents: boolean;
  isDirectFocus: boolean;
  layerKey: LayerKey;
  children: ReactNode;
  ref?: Ref<HTMLDivElement>;
}

/**
 * Body branch when a `<FocusLayer>` ancestor IS present.
 *
 * Mints a `SpatialKey`, registers with the Rust-side spatial registry via
 * `spatial_register_zone`, subscribes to per-key focus claims, publishes
 * its key via `FocusZoneContext.Provider` for descendants, and renders a
 * single `<div>` that carries the consumer's className plus the
 * `data-moniker` / `data-focused` debugging attributes.
 *
 * The chrome (right-click → context menu, click → spatial focus) lives
 * on the same `<div>` as the spatial primitive's root. Inspector
 * dispatch (double-click → `ui.inspect`) is **not** owned here — it
 * lives in `<Inspectable>` (`inspectable.tsx`).
 */
function SpatialFocusZoneBody({
  moniker,
  navOverride,
  showFocusBar,
  handleEvents,
  isDirectFocus,
  layerKey,
  children,
  ref: externalRef,
  ...htmlProps
}: SpatialFocusZoneBodyProps) {
  const contextMenuHandler = useContextMenu();
  const focusActions = useOptionalFocusActions();
  const setFocus = focusActions?.setFocus;

  // Mint a stable SpatialKey per mount. Held in a ref so re-renders do
  // not allocate a fresh ULID.
  const keyRef = useRef<SpatialKey | null>(null);
  if (keyRef.current === null) {
    keyRef.current = asSpatialKey(crypto.randomUUID());
  }
  const key = keyRef.current;

  // Read the parent zone (when present) so the registration call can
  // populate the Rust-side `parent_zone` field.
  const parentZone = useParentZoneKey();

  // Ref to the rendered div. Drives the `scrollIntoView` effect plus the
  // ResizeObserver below.
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
        // React 19 typed `RefObject<T>.current` as readonly even though
        // the runtime still allows assignment — cast to the mutable view.
        (externalRef as React.MutableRefObject<HTMLDivElement | null>).current =
          node;
      }
    },
    [externalRef],
  );

  const [focused, setFocused] = useState(false);
  useFocusClaim(key, setFocused);

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
  // that hands us a fresh-identity literal.
  //
  // Callers must therefore treat `navOverride` as effectively-stable for
  // the lifetime of a given (moniker, layerKey, parentZone) tuple.
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

  // Ancestor-scroll listener: refresh the kernel's rect whenever any
  // scrollable ancestor (or the document) scrolls. The `ResizeObserver`
  // above only fires on size changes, so without this hook a scrolled
  // column would leave the kernel with mount-time viewport coordinates
  // and beam-search would pick wrong candidates.
  useTrackRectOnAncestorScroll(ref, key, updateRect);

  // Scroll-into-view when the entity-focus store reports this zone as
  // directly focused — preserves the legacy "follow the focus" behaviour.
  useEffect(() => {
    if (isDirectFocus && ref.current?.scrollIntoView) {
      ref.current.scrollIntoView({ block: "nearest" });
    }
  }, [isDirectFocus]);

  const handleContextMenu = useCallback(
    (e: React.MouseEvent) => {
      // When handleEvents is false, let the event propagate to the parent
      // entity scope (e.g. an enclosing grid-cell `<FocusScope>`).
      if (!handleEvents) return;
      e.preventDefault();
      e.stopPropagation();
      // `setFocus` is only available when an `EntityFocusProvider` is
      // mounted; the context menu still opens via the CommandScopeContext
      // chain, but the entity-focus side effect is skipped when missing.
      if (setFocus) setFocus(moniker);
      contextMenuHandler(e);
    },
    [moniker, setFocus, contextMenuHandler, handleEvents],
  );

  const handleClick = useCallback(
    (e: React.MouseEvent) => {
      if (!handleEvents) return;
      // Skip when the click landed on an editable surface — letting the
      // editor own the click avoids stealing caret placement from the user.
      const target = e.target as HTMLElement;
      const tag = target.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
      if (target.closest("[contenteditable]")) return;
      // Stop here so a click on this zone does not bubble into an
      // enclosing zone (or `<FocusScope>`) and fire `spatial_focus` again
      // with the outer key. Each level handles its own click exactly once.
      e.stopPropagation();
      focus(key).catch((err) =>
        console.error("[FocusZone] focus failed", err),
      );
    },
    [focus, key, handleEvents],
  );

  // Merge `relative` into the consumer's className so the absolutely-
  // positioned `<FocusIndicator>` child positions itself against this
  // zone's box rather than escaping to the nearest ancestor with a
  // containing block. The merge keeps consumer styles intact and adds
  // the positioning hint without forcing every call site to remember it.
  const { className: consumerClassName, ...restWithoutClassName } = htmlProps;
  const mergedClassName = cn(consumerClassName, "relative");

  // Read the spatial-nav debug flag once per render — the overlay renders
  // a dashed border + coordinate label when on, nothing when off. The
  // hook is unconditional; only the rendered overlay element is gated.
  // See `lib/focus-debug-context.tsx` for the toggle path.
  const debugEnabled = useFocusDebug();

  return (
    <FocusZoneContext.Provider value={key}>
      <div
        ref={setRef}
        data-moniker={moniker}
        data-focused={focused || undefined}
        onClick={handleClick}
        onContextMenu={handleContextMenu}
        {...restWithoutClassName}
        className={mergedClassName}
      >
        {showFocusBar && <FocusIndicator focused={focused} />}
        {debugEnabled && (
          <FocusDebugOverlay kind="zone" label={moniker} hostRef={ref} />
        )}
        {children}
      </div>
    </FocusZoneContext.Provider>
  );
}

// ---------------------------------------------------------------------------
// Body — no-spatial-context fallback
// ---------------------------------------------------------------------------

/** Props for the no-spatial-context fallback body. */
interface FallbackFocusZoneBodyProps extends Omit<
  HTMLAttributes<HTMLDivElement>,
  "onClick" | "children"
> {
  moniker: Moniker;
  /** See {@link FocusZoneOwnProps.handleEvents}. */
  handleEvents: boolean;
  isDirectFocus: boolean;
  children: ReactNode;
  ref?: Ref<HTMLDivElement>;
}

/**
 * Body branch for tests that mount a `<FocusZone>` outside the spatial
 * provider stack (no `<SpatialFocusProvider>` / no `<FocusLayer>`).
 *
 * Renders a plain `<div>` with as much chrome as the surrounding providers
 * make available. Skips the spatial registration and the per-key
 * focus-claim subscription that would otherwise throw. Production code
 * never enters this branch.
 *
 * What still runs here:
 *
 *   - The plain `<div>` carrying `data-moniker` and `data-focused` (the
 *     latter driven by the optional entity-focus store).
 *   - `CommandScopeContext` and `FocusScopeContext` providers — pushed
 *     by the parent `<FocusZone>` body around this branch.
 *   - Right-click → context menu via the command-scope chain; the
 *     entity-focus side effect (`setFocus(moniker)`) only fires when an
 *     `EntityFocusProvider` is mounted.
 *   - Click → `setFocus(moniker)`, when an `EntityFocusProvider` is
 *     mounted; otherwise a no-op.
 *
 * Inspector dispatch (double-click → `ui.inspect`) is **not** owned
 * here — it lives in `<Inspectable>` (`inspectable.tsx`).
 *
 * Visible focus bar is intentionally NOT rendered here — there is no
 * `useFocusClaim` subscription to drive a `<FocusIndicator>`.
 */
function FallbackFocusZoneBody({
  moniker,
  handleEvents,
  isDirectFocus,
  children,
  ref: externalRef,
  ...htmlProps
}: FallbackFocusZoneBodyProps) {
  const contextMenuHandler = useContextMenu();
  const focusActions = useOptionalFocusActions();
  const setFocus = focusActions?.setFocus;

  const ref = useRef<HTMLDivElement | null>(null);

  const setRef = useCallback(
    (node: HTMLDivElement | null) => {
      ref.current = node;
      if (typeof externalRef === "function") {
        externalRef(node);
      } else if (externalRef) {
        (externalRef as React.MutableRefObject<HTMLDivElement | null>).current =
          node;
      }
    },
    [externalRef],
  );

  // Scroll-into-view when the entity-focus store reports this zone as
  // directly focused — same legacy behavior the spatial branch has.
  useEffect(() => {
    if (isDirectFocus && ref.current?.scrollIntoView) {
      ref.current.scrollIntoView({ block: "nearest" });
    }
  }, [isDirectFocus]);

  const handleContextMenu = useCallback(
    (e: React.MouseEvent) => {
      if (!handleEvents) return;
      e.preventDefault();
      e.stopPropagation();
      if (setFocus) setFocus(moniker);
      contextMenuHandler(e);
    },
    [moniker, setFocus, contextMenuHandler, handleEvents],
  );

  const handleClick = useCallback(
    (e: React.MouseEvent) => {
      if (!handleEvents) return;
      const target = e.target as HTMLElement;
      const tag = target.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
      if (target.closest("[contenteditable]")) return;
      e.stopPropagation();
      if (setFocus) setFocus(moniker);
    },
    [moniker, setFocus, handleEvents],
  );

  // Merge `relative` into the consumer's className so the absolutely-
  // positioned debug overlay positions itself against this zone's box
  // rather than escaping to the nearest ancestor with a containing block.
  // Production never enters this branch (no `<FocusLayer>` ancestor), so
  // the merge is only relevant when a test mounts a `<FocusZone>` inside
  // `<FocusDebugProvider enabled>` without the spatial provider stack.
  const { className: consumerClassName, ...restWithoutClassName } = htmlProps;
  const mergedClassName = cn(consumerClassName, "relative");

  // Read the spatial-nav debug flag — see `lib/focus-debug-context.tsx`.
  const debugEnabled = useFocusDebug();

  return (
    <div
      ref={setRef}
      data-moniker={moniker}
      data-focused={isDirectFocus || undefined}
      onClick={handleClick}
      onContextMenu={handleContextMenu}
      {...restWithoutClassName}
      className={mergedClassName}
    >
      {debugEnabled && (
        <FocusDebugOverlay kind="zone" label={moniker} hostRef={ref} />
      )}
      {children}
    </div>
  );
}
