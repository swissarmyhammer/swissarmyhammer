/**
 * `<FocusScope>` — the leaf primitive in the spatial-nav graph and the
 * entity-aware focus point most call sites use.
 *
 * `<FocusScope>` is a **pure spatial primitive**. It does NOT know about
 * inspectable entities and does NOT dispatch `ui.inspect`. Inspector
 * dispatch lives in `<Inspectable>` — see `inspectable.tsx`. Wrap an
 * entity subtree in `<Inspectable>` to make double-click open the
 * inspector; do not look for an `inspectOnDoubleClick` prop here.
 *
 * Three peers, not four: the spatial-nav kernel exposes `<FocusLayer>`
 * (modal boundary), `<FocusZone>` (navigable container), and
 * `<FocusScope>` (leaf). This component is the leaf — it does what the
 * pre-collapse `<Focusable>` and `<FocusScope>` did separately, in one
 * place:
 *
 *   - Mints a stable `SpatialKey` per mount and registers with Rust via
 *     `spatial_register_scope`.
 *   - Subscribes to per-key focus claims through `useFocusClaim` so its
 *     `data-focused` attribute and the visible `<FocusIndicator>` flip
 *     when this key becomes focused.
 *   - Handles click → `spatial_focus`, with editable surfaces (inputs,
 *     contenteditable) spared so caret placement is not stolen.
 *   - Right-click → `setFocus(moniker)` + native context menu via
 *     `useContextMenu`.
 *   - Pushes a `CommandScopeContext.Provider` so descendants participate
 *     in command resolution and the context-menu chain.
 *   - Pushes a `FocusScopeContext.Provider` so descendants discover their
 *     nearest enclosing scope without walking the command-scope chain.
 *   - Registers with the entity-focus scope registry so `useFocusedScope`
 *     and the dispatcher can compute scope chains.
 *   - Optional `navOverride` per-direction directives forwarded into the
 *     Rust-side registry. The kernel runs these as rule 0 of beam search:
 *     a same-layer target moniker redirects, `null` blocks navigation in
 *     that direction, missing keys fall through to beam search.
 *   - `scrollIntoView` when the entity-focus store reports this scope as
 *     directly focused — preserves the legacy "follow the focus bar"
 *     scroll behavior.
 *
 * For containers (board, column, grid, perspective, view, nav-bar,
 * toolbar group), use `<FocusZone>` directly. For modal boundaries
 * (window root, inspector, dialog), use `<FocusLayer>` directly.
 *
 * # Lifecycle
 *
 *   - **Mount**: mints a fresh `SpatialKey`, reads its enclosing
 *     `<FocusLayer>` and (optional) `<FocusZone>` from context, snapshots
 *     the bounding rect, invokes `spatial_register_scope`, and registers
 *     itself in the entity-focus registry.
 *   - **Resize**: a ResizeObserver attached to the root element pushes
 *     rect deltas via `spatial_update_rect`.
 *   - **Ancestor scroll**: a passive `scroll` listener (per-rAF
 *     throttled) on every scrollable ancestor and on `window` pushes
 *     fresh rects via `spatial_update_rect`. Without this, scrolling
 *     a column container would shift the leaf's viewport-y while the
 *     kernel kept its mount-time rect, and beam-search would run on
 *     stale geometry.
 *   - **Click / right-click**: see above.
 *   - **Focus claim**: `useFocusClaim` subscribes to the per-key boolean
 *     stream so the wrapper renders `data-focused` toggling without
 *     re-rendering the entire tree on every focus move elsewhere.
 *   - **Unmount**: invokes `spatial_unregister_scope`, disconnects the
 *     ResizeObserver, and unregisters from the entity-focus registry.
 *
 * # Optional providers
 *
 * The pre-collapse `<Focusable>` was a pure spatial-nav primitive — it
 * required `<SpatialFocusProvider>` + `<FocusLayer>` to mount but never
 * touched `<EntityFocusProvider>`. The collapsed `<FocusScope>` preserves
 * that contract: when an `EntityFocusProvider` ancestor is present, the
 * primitive layers the entity-focus chrome (scope-registry registration,
 * `scrollIntoView` on direct focus) on top; when it is absent, those
 * pieces silently no-op while the spatial-nav and command-scope chrome
 * keep running.
 *
 * Two independent ancestors gate the chrome:
 *
 *   - `<FocusLayer>` — when missing, the component skips the spatial
 *     registration entirely (no `spatial_register_scope`, no focus-claim
 *     subscription, no visible `<FocusIndicator>`) and degrades to a
 *     plain `<div>`. Tests that mount one component at a time without
 *     standing up `<SpatialFocusProvider>` exercise this path.
 *   - `<EntityFocusProvider>` — when missing, the entity-focus
 *     scope-registry registration and the `scrollIntoView` effect are
 *     skipped. The CommandScope provider, FocusScopeContext provider,
 *     and the spatial-nav chrome (when a layer is present) all keep
 *     working. Right-click handlers fall through to no-ops because
 *     their entity-focus targets are unavailable.
 *
 * Production code (`App.tsx` and the quick-capture window) always mounts
 * both providers, so neither degraded branch is ever reached at runtime.
 */

import {
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
import { useParentZoneKey } from "@/components/focus-zone";
import { FocusDebugOverlay } from "@/components/focus-debug-overlay";
import { FocusIndicator } from "@/components/focus-indicator";
import {
  FocusScopeContext,
  useParentFocusScope,
} from "@/components/focus-scope-context";
import { useTrackRectOnAncestorScroll } from "@/components/use-track-rect-on-ancestor-scroll";
import {
  asPixels,
  asSpatialKey,
  type FocusOverrides,
  type Moniker,
  type SpatialKey,
} from "@/types/spatial";

// `FocusScopeContext` is shared with `<FocusZone>` via `./focus-scope-context`.
// Both push the nearest entity moniker so descendants resolve without walking
// the command-scope chain.

/** Own props for `<FocusScope>`; standard HTML attributes (className, style, data-*) pass through. */
export interface FocusScopeOwnProps {
  /** The branded entity moniker for the scope this represents. */
  moniker: Moniker;
  /**
   * Per-direction navigation overrides forwarded into the Rust-side
   * registry. Missing keys mean "fall through to beam search"; an
   * explicit `null` blocks navigation in that direction; a `Moniker`
   * value redirects.
   */
  navOverride?: FocusOverrides;
  /**
   * Commands to register in this scope. Optional — defaults to the shared
   * `EMPTY_COMMANDS` constant from command-scope. Most FocusScopes exist
   * purely to register an entity moniker in the focus/scope chain and have
   * no per-scope commands of their own; those callers should simply omit
   * the prop. Only pass an array when the scope genuinely contributes
   * commands (e.g. `extraCommands` forwarded from a parent card).
   */
  commands?: readonly CommandDef[];
  children: ReactNode;
  /**
   * When false, suppresses both:
   *   1. The visible `<FocusIndicator>` rendered by the primitive (the
   *      `data-focused` attribute and the focus-claim subscription stay
   *      active so tests / e2e selectors keep working).
   *   2. The entity-focus-driven `scrollIntoView` effect (the legacy
   *      "follow the focus bar" scroll).
   *
   * Container scopes whose visible focus bar would clutter the surrounding
   * UI (e.g. an inspector entity scope, where the panel itself is the
   * visual cue) opt out by passing `showFocusBar={false}`.
   *
   * Defaults to true.
   */
  showFocusBar?: boolean;
  /**
   * When false, suppresses click / right-click / double-click event
   * handling. Independent of `showFocusBar` — a scope can handle events
   * without showing the focus bar. Defaults to true.
   */
  handleEvents?: boolean;
  /**
   * When false, omits the wrapping primitive — children render directly
   * under the CommandScopeContext + FocusScopeContext providers.
   *
   * Use for table rows where a wrapping div breaks HTML structure (the row
   * already IS a focusable container in the table layout). The scope,
   * moniker registration, and context still work; the caller must attach
   * onContextMenu etc. to their own element and accepts that the spatial
   * primitive won't register this scope.
   *
   * This is an escape hatch for the data-table edge case and is retained
   * for source compatibility.
   */
  renderContainer?: boolean;
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
 * Full props for `<FocusScope>` — `FocusScopeOwnProps` + passthrough HTML attrs.
 *
 * `onClick` is intentionally omitted from the passthrough: the primitive owns
 * the click handler so it can call `spatial_focus`. Allowing a consumer to
 * spread their own `onClick` would silently replace the spatial handler (the
 * inline handler is set before `{...rest}`), breaking focus-on-click. Wrap
 * the consumer's element instead, or attach click logic at a layer the
 * primitive does not control. This matches the convention `<FocusZone>` uses.
 */
export type FocusScopeProps = FocusScopeOwnProps &
  Omit<HTMLAttributes<HTMLDivElement>, keyof FocusScopeOwnProps | "onClick">;

/**
 * Mounts a leaf focus scope in the Rust-side spatial graph and layers
 * the entity-focus / command-scope / context-menu chrome on top.
 *
 * The key is minted once on mount (held in a ref) so it stays stable
 * across re-renders. A ResizeObserver attached to the root element keeps
 * the Rust-side rect in sync. The component re-renders only when its own
 * focus claim flips — the rest of the tree is unaffected by focus moves
 * elsewhere thanks to the per-key claim registry.
 */
export function FocusScope({
  moniker,
  navOverride,
  commands = EMPTY_COMMANDS,
  children,
  showFocusBar = true,
  handleEvents = true,
  renderContainer = true,
  ref: externalRef,
  ...rest
}: FocusScopeProps) {
  // Selective subscription: this scope re-renders only when *its own*
  // moniker's focus slot flips — not on every focus move elsewhere in the
  // tree. In a 12k-cell grid, one arrow-key press wakes only two of these.
  // Drives the scroll-into-view effect in the body branches so the
  // legacy "follow the focus" behaviour keeps firing when the entity
  // focus store reports this moniker as directly focused. When no
  // `EntityFocusProvider` is mounted, this returns `false` permanently.
  const isFocused = useOptionalIsDirectFocus(moniker);

  // Build the scope ourselves so we can register it in the entity-focus
  // registry. Same shape as CommandScopeProvider produces, but we control
  // the lifecycle so the registry stays in lockstep.
  const parent = useContext(CommandScopeContext);
  const scope = useMemo<CommandScope>(() => {
    const map = new Map<string, CommandDef>();
    for (const cmd of commands) {
      map.set(cmd.id, cmd);
    }
    return { commands: map, parent, moniker };
  }, [commands, parent, moniker]);

  const isDirectFocus = showFocusBar && isFocused;

  // Register the scope in the entity-focus registry. The helper handles the
  // inline-during-render `Map.set` plus the cleanup-only effect, with full
  // tolerance for a missing `EntityFocusProvider`. See its docstring in
  // `entity-focus-context.tsx` for the identity-churn rationale that keeps
  // 12k-cell grids from flooding React's commit phase on every render.
  useEntityScopeRegistration(moniker, scope);

  // Detect whether a `<FocusLayer>` ancestor is mounted. Production code
  // (App.tsx and the quick-capture window) wraps everything in one, but
  // many isolated tests render components without the spatial-focus
  // provider stack. When no layer is present we degrade to a plain
  // `<div>` — the entity-focus chrome (when an `EntityFocusProvider` is
  // mounted) still works, only the spatial registration is skipped.
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
        {!renderContainer ? (
          children
        ) : hasSpatialContext ? (
          <SpatialFocusScopeBody
            moniker={moniker}
            navOverride={navOverride}
            showFocusBar={showFocusBar}
            isDirectFocus={isDirectFocus}
            handleEvents={handleEvents}
            layerKey={layerKey}
            ref={externalRef}
            {...rest}
          >
            {children}
          </SpatialFocusScopeBody>
        ) : (
          <FallbackFocusScopeBody
            moniker={moniker}
            isDirectFocus={isDirectFocus}
            handleEvents={handleEvents}
            ref={externalRef}
            {...rest}
          >
            {children}
          </FallbackFocusScopeBody>
        )}
      </CommandScopeContext.Provider>
    </FocusScopeContext.Provider>
  );
}

// ---------------------------------------------------------------------------
// Body — spatial-context branch
// ---------------------------------------------------------------------------

/** Props for the spatial-context body. */
interface SpatialFocusScopeBodyProps extends Omit<
  HTMLAttributes<HTMLDivElement>,
  "onClick" | "children"
> {
  moniker: Moniker;
  navOverride?: FocusOverrides;
  showFocusBar: boolean;
  isDirectFocus: boolean;
  handleEvents: boolean;
  layerKey: import("@/types/spatial").LayerKey;
  children: ReactNode;
  ref?: Ref<HTMLDivElement>;
}

/**
 * Body branch when a `<FocusLayer>` ancestor IS present.
 *
 * Mints a `SpatialKey`, registers with the Rust-side spatial registry via
 * `spatial_register_scope`, subscribes to per-key focus claims, and
 * renders a single `<div>` that carries the consumer's className plus the
 * `data-moniker` / `data-focused` debugging attributes.
 *
 * The chrome (right-click → context menu, click → spatial focus) lives
 * on the same `<div>` as the spatial primitive's root — there is
 * intentionally no inner wrapper because an earlier revision's
 * `<FocusScopeBody>` div broke the flex chain when consumers passed
 * `<FocusScope className="flex …">`.
 *
 * Inspector dispatch (double-click → `ui.inspect`) is **not** owned
 * here — it lives in `<Inspectable>` (`inspectable.tsx`). Wrap an
 * entity subtree in `<Inspectable>` when its double-click should open
 * the inspector.
 */
function SpatialFocusScopeBody({
  moniker,
  navOverride,
  showFocusBar,
  isDirectFocus,
  handleEvents,
  layerKey,
  children,
  ref: externalRef,
  ...htmlProps
}: SpatialFocusScopeBodyProps) {
  const contextMenuHandler = useContextMenu();
  // Optional: when no `EntityFocusProvider` is mounted, `setFocus` is null
  // and the right-click handler skips the entity-focus side effect (focus
  // still moves spatially via `spatial_focus`). Production always has an
  // `EntityFocusProvider`, so `setFocus` is always available there.
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

  const { registerScope: registerSpatialScope, unregisterScope, updateRect, focus } =
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
    registerSpatialScope(
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
    ).catch((err) => console.error("[FocusScope] register failed", err));

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
      }).catch((err) => console.error("[FocusScope] updateRect failed", err));
    });
    observer.observe(node);

    return () => {
      observer.disconnect();
      unregisterScope(key).catch((err) =>
        console.error("[FocusScope] unregister failed", err),
      );
    };
  }, [
    key,
    moniker,
    layerKey,
    parentZone,
    registerSpatialScope,
    unregisterScope,
    updateRect,
  ]);

  // Ancestor-scroll listener: refresh the kernel's rect whenever any
  // scrollable ancestor (or the document) scrolls. The `ResizeObserver`
  // above only fires on size changes, so without this hook a scrolled
  // column would leave the kernel with mount-time viewport coordinates
  // and beam-search would pick wrong candidates.
  useTrackRectOnAncestorScroll(ref, key, updateRect);

  // Scroll-into-view when the entity-focus store reports this scope as
  // directly focused — preserves the legacy "follow the focus" behaviour.
  useEffect(() => {
    if (isDirectFocus && ref.current?.scrollIntoView) {
      ref.current.scrollIntoView({ block: "nearest" });
    }
  }, [isDirectFocus]);

  const handleContextMenu = useCallback(
    (e: React.MouseEvent) => {
      // When handleEvents is false, let the event propagate to the parent
      // entity scope (e.g. EntityRow).
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
      // Skip when the click landed on an editable surface — letting the
      // editor own the click avoids stealing caret placement from the user.
      const target = e.target as HTMLElement;
      const tag = target.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
      if (target.closest("[contenteditable]")) return;
      // Stop here: a leaf click must not bubble to an enclosing
      // <FocusZone> and fire `spatial_focus` again with the ancestor's
      // key — that would clobber the user's intent.
      e.stopPropagation();
      focus(key).catch((err) =>
        console.error("[FocusScope] focus failed", err),
      );
    },
    [focus, key],
  );

  // Merge `relative` into the consumer's className. `relative` is required
  // so the absolutely-positioned `<FocusIndicator>` child positions itself
  // against the primitive's box rather than escaping to the nearest
  // ancestor with a containing block. The merge keeps consumer styles
  // (e.g. `<FocusScope className="flex flex-col">`) intact and adds the
  // positioning hint without forcing every call site to remember it.
  const { className: consumerClassName, ...restWithoutClassName } = htmlProps;
  const mergedClassName = cn(consumerClassName, "relative");

  // Read the spatial-nav debug flag once per render — the overlay renders
  // a dashed border + coordinate label when on, nothing when off. See
  // `lib/focus-debug-context.tsx` for the toggle path (App.tsx and the
  // quick-capture window mount `<FocusDebugProvider enabled>`).
  const debugEnabled = useFocusDebug();

  // Render the single primitive div. `data-focused` reflects the per-key
  // spatial focus claim. It rides along as an output-only debugging /
  // e2e selector — no CSS rule reads it back as state (the visible bar
  // is rendered from React state by `<FocusIndicator>`).
  //
  // No `onDoubleClick` is attached: inspector dispatch is owned by
  // `<Inspectable>` (`inspectable.tsx`). When an entity subtree should
  // be inspectable, wrap this scope in `<Inspectable>`; the gesture
  // bubbles untouched to that ancestor.
  return (
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
        <FocusDebugOverlay kind="scope" label={moniker} hostRef={ref} />
      )}
      {children}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Body — no-spatial-context fallback
// ---------------------------------------------------------------------------

/** Props for the no-spatial-context fallback body. */
interface FallbackFocusScopeBodyProps extends Omit<
  HTMLAttributes<HTMLDivElement>,
  "onClick" | "children"
> {
  moniker: Moniker;
  isDirectFocus: boolean;
  handleEvents: boolean;
  children: ReactNode;
  ref?: Ref<HTMLDivElement>;
}

/**
 * Body branch for tests that mount a `<FocusScope>` outside the spatial
 * provider stack (no `<SpatialFocusProvider>` / no `<FocusLayer>`).
 *
 * Renders a plain `<div>` with as much chrome as the surrounding providers
 * make available. Skips the spatial registration and the per-key
 * focus-claim subscription that would otherwise throw. Production code
 * never enters this branch — `App.tsx` and the quick-capture window
 * always wrap their tree in a `<FocusLayer>`. The branch exists purely
 * so the long tail of unit tests that mount one component at a time
 * continues to work without standing up the spatial provider stack.
 *
 * What still runs here:
 *
 *   - The plain `<div>` carrying `data-moniker` and `data-focused` (the
 *     latter driven by the optional entity-focus store; falls back to
 *     `false` when no `EntityFocusProvider` is mounted).
 *   - `CommandScopeContext` and `FocusScopeContext` providers — pushed
 *     by the parent `<FocusScope>` body around this branch.
 *   - Right-click → context menu, when a `CommandScopeContext` chain is
 *     reachable (always true here because `<FocusScope>` itself pushes
 *     one); the entity-focus side effect (`setFocus(moniker)`) only
 *     fires when an `EntityFocusProvider` is also mounted.
 *   - Click → `setFocus(moniker)`, when an `EntityFocusProvider` is
 *     mounted; otherwise a no-op (no spatial focus is reachable in this
 *     branch either way).
 *
 * Inspector dispatch (double-click → `ui.inspect`) is **not** owned
 * here either — wrap an entity subtree in `<Inspectable>`
 * (`inspectable.tsx`) when its double-click should open the inspector.
 *
 * Visible focus bar is intentionally NOT rendered here — there is no
 * `useFocusClaim` subscription to drive a `<FocusIndicator>` and no
 * Rust-side focus state to follow. Tests exercising this path assert
 * against the output-only `data-focused` attribute directly, which
 * tracks the entity-focus `isDirectFocus` value computed by the parent
 * `<FocusScope>`.
 */
function FallbackFocusScopeBody({
  moniker,
  isDirectFocus,
  handleEvents,
  children,
  ref: externalRef,
  ...htmlProps
}: FallbackFocusScopeBodyProps) {
  const contextMenuHandler = useContextMenu();
  // Optional: tolerate a missing `EntityFocusProvider`. The legacy
  // `<Focusable>` did not require this provider, and the collapsed
  // `<FocusScope>` preserves that contract — when absent, `setFocus`
  // is null and the click / right-click handlers below skip the
  // entity-focus side effect.
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

  // Scroll-into-view when the entity-focus store reports this scope as
  // directly focused — same legacy behavior the spatial branch has.
  // No-op when no `EntityFocusProvider` is mounted (`isDirectFocus` is
  // permanently `false` in that case).
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
  // positioned debug overlay positions itself against this scope's box
  // rather than escaping to the nearest ancestor with a containing block.
  // Production never enters this branch (no `<FocusLayer>` ancestor), so
  // the merge is only relevant when a test mounts a `<FocusScope>` inside
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
        <FocusDebugOverlay kind="scope" label={moniker} hostRef={ref} />
      )}
      {children}
    </div>
  );
}

// `useParentFocusScope` is exported as a re-export below so existing
// import paths (`@/components/focus-scope`) keep resolving.
export { useParentFocusScope };
