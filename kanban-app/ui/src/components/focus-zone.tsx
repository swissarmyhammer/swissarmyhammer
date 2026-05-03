/**
 * `<FocusZone>` — the navigable-container peer in the spatial-nav graph and
 * the entity-aware composite that production zone wraps use.
 *
 * `<FocusZone>` is a **pure spatial primitive**. It does NOT know about
 * inspectable entities and does NOT dispatch `ui.inspect`. Inspector
 * dispatch lives in `<Inspectable>` — see `inspectable.tsx`.
 *
 * # Path-monikers identity model
 *
 * After card `01KQD6064G1C1RAXDFPJVT1F46` the spatial graph uses one
 * identifier shape per primitive: `FullyQualifiedMoniker`. The FQM is
 * the spatial key — there is no separate UUID. The zone reads its
 * parent FQM from `FullyQualifiedMonikerContext`, composes its own
 * FQM as `<parentFq>/<segment>`, and provides that FQM downward via
 * `<FullyQualifiedMonikerContext.Provider>` so descendants register
 * with the correct path. The composed FQM also doubles as the kernel
 * registry key sent to `spatial_register_zone` — there is no
 * `crypto.randomUUID()` on the React side.
 *
 * Three peers, not four: the spatial-nav kernel exposes `<FocusLayer>`
 * (modal boundary), `<FocusZone>` (navigable container), and `<FocusScope>`
 * (leaf). This component is the zone — a parent of leaves and nested
 * zones in the spatial graph, and an entity-bound surface in the
 * command-scope / context-menu chain.
 *
 *   - Composes its FQM via `useFullyQualifiedMoniker()` + the consumer's
 *     `moniker` segment, then registers with Rust via
 *     `spatial_register_zone(fq, segment, rect, layerFq, parentZone,
 *     overrides)`.
 *   - Publishes its FQM via `FullyQualifiedMonikerContext.Provider` so
 *     descendant scopes pick it up as their parent.
 *   - Subscribes to per-FQM focus claims through `useFocusClaim` so its
 *     `data-focused` attribute and the visible `<FocusIndicator>` flip
 *     when this FQM becomes focused.
 *   - Handles click → `spatial_focus(fq)`, with editable surfaces (inputs,
 *     contenteditable) spared so caret placement is not stolen.
 *   - Right-click → `setFocus(fq)` + native context menu via
 *     `useContextMenu`.
 *   - Pushes a `CommandScopeContext.Provider` so descendants participate
 *     in command resolution and the context-menu chain.
 *   - Pushes a `FocusScopeContext.Provider` so descendants discover their
 *     nearest enclosing entity scope without walking the command-scope
 *     chain.
 *   - Registers with the entity-focus scope registry so `useFocusedScope`
 *     and the dispatcher can compute scope chains.
 *   - Optional `navOverride` per-direction directives forwarded into the
 *     Rust-side registry.
 *   - `scrollIntoView` when the entity-focus store reports this zone as
 *     directly focused — preserves the legacy "follow the focus bar"
 *     scroll behavior.
 *
 * For leaves (a tag pill, a title field, a toolbar button), use
 * `<FocusScope>` directly. For modal boundaries (window root, inspector,
 * dialog), use `<FocusLayer>` directly.
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
import {
  FullyQualifiedMonikerContext,
  useFullyQualifiedMoniker,
} from "@/components/fully-qualified-moniker-context";
import { useEnclosingLayerFq } from "@/components/layer-fq-context";
import { FocusDebugOverlay } from "@/components/focus-debug-overlay";
import { FocusIndicator } from "@/components/focus-indicator";
import { FocusScopeContext } from "@/components/focus-scope-context";
import { useTrackRectOnAncestorScroll } from "@/components/use-track-rect-on-ancestor-scroll";
import {
  asPixels,
  composeFq,
  type FocusOverrides,
  type FullyQualifiedMoniker,
  type SegmentMoniker,
} from "@/types/spatial";

// ---------------------------------------------------------------------------
// FocusZoneContext — descendants discover their nearest zone ancestor
// ---------------------------------------------------------------------------

/**
 * The `FullyQualifiedMoniker` of the nearest ancestor `<FocusZone>`, or
 * `null` when the descendant is mounted directly under the layer root
 * (i.e. its enclosing primitive is a `<FocusLayer>`, not a `<FocusZone>`).
 *
 * This is distinct from `FullyQualifiedMonikerContext` — the latter
 * carries the FQM of the *immediate* ancestor primitive, which can be a
 * layer or a zone. `FocusZoneContext` carries the FQM of the nearest
 * **zone** ancestor specifically, so descendants can populate the
 * `parentZone` argument of their `spatial_register_*` calls (which the
 * kernel uses for cascade and drill-out fallback).
 */
export const FocusZoneContext = createContext<FullyQualifiedMoniker | null>(
  null,
);

/**
 * Read the FQM of the enclosing `<FocusZone>`, or `null` when no
 * zone wraps the caller.
 *
 * Used by `<FocusScope>` and nested `<FocusZone>` instances to populate
 * the `parentZone` argument of their register calls. A `null` parent is
 * valid: it means the scope is anchored directly at the layer root.
 */
export function useParentZoneFq(): FullyQualifiedMoniker | null {
  return useContext(FocusZoneContext);
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

/** Own props for `<FocusZone>`; standard HTML attributes (className, style, data-*) pass through. */
export interface FocusZoneOwnProps {
  /**
   * Relative `SegmentMoniker` for this zone — e.g. `"toolbar.actions"`,
   * `"column:01ABC"`, `"card:T1"`. The zone's full FQM is composed by
   * appending this segment to the parent FQM read from
   * `FullyQualifiedMonikerContext`.
   */
  moniker: SegmentMoniker;
  /** Optional per-direction navigation overrides (walls/redirects). */
  navOverride?: FocusOverrides;
  /**
   * Commands to register in this zone's `CommandScope`. Optional —
   * defaults to the shared `EMPTY_COMMANDS` constant. Most zones exist
   * purely to register a moniker in the focus / scope chain and have
   * no per-zone commands of their own; those callers should simply omit
   * the prop.
   */
  commands?: readonly CommandDef[];
  /**
   * When false, suppresses both:
   *   1. The visible `<FocusIndicator>` rendered by the primitive (the
   *      `data-focused` attribute and the focus-claim subscription stay
   *      active so tests / e2e selectors keep working).
   *   2. The entity-focus-driven `scrollIntoView` effect (the legacy
   *      "follow the focus bar" scroll).
   */
  showFocusBar?: boolean;
  /**
   * When false, suppresses click / right-click / double-click event
   * handling on the zone's outer `<div>`. Independent of `showFocusBar`
   * — a zone can register and emit `data-focused` without owning clicks.
   *
   * Defaults to true. Mirrors the `<FocusScope>` prop of the same name.
   */
  handleEvents?: boolean;
  /**
   * When false, omits the wrapping `<div>` — children render directly
   * under the CommandScopeContext + FocusZoneContext +
   * FullyQualifiedMonikerContext providers. The kernel registration is
   * skipped (no rect tracking, no focus-claim subscription, no event
   * handlers) because there is no DOM node to attach them to.
   *
   * Use for table rows where a wrapping div would break HTML structure
   * (`<tbody>` requires `<tr>` children, not `<div>` children) but the
   * row still needs to be a Zone in the spatial graph so cell leaves
   * compose their FQMs and `parent_zone` under the row instead of
   * skipping up to the surrounding grid zone. Mirrors the `<FocusScope>`
   * prop of the same name, but pushes the additional `FocusZoneContext`
   * + `FullyQualifiedMonikerContext` providers so descendants resolve
   * the row Zone as their nearest enclosing zone (rather than walking
   * past it).
   */
  renderContainer?: boolean;
  /** Children rendered inside the zone container. */
  children: ReactNode;
  /**
   * Optional ref to the rendered `<div>` element. The primitive holds an
   * internal ref for its ResizeObserver and click handler; if you supply
   * one here it is attached alongside that internal ref so callers can
   * reach the same DOM node (e.g. to call `scrollIntoView`).
   */
  ref?: Ref<HTMLDivElement>;
}

/**
 * Full props for `<FocusZone>` — `FocusZoneOwnProps` + passthrough HTML attrs.
 *
 * `onClick` is intentionally omitted from the passthrough: the primitive owns
 * the click handler so it can call `spatial_focus`. Allowing a consumer to
 * spread their own `onClick` would silently replace the spatial handler.
 */
export type FocusZoneProps = FocusZoneOwnProps &
  Omit<HTMLAttributes<HTMLDivElement>, keyof FocusZoneOwnProps | "onClick">;

/**
 * Mounts an entity-bound zone in the Rust-side spatial graph and publishes
 * its FQM via `FullyQualifiedMonikerContext` so descendants register with
 * the correct parent path.
 *
 * The FQM is composed deterministically from the parent FQM context plus
 * the consumer's `moniker` segment — no UUID minting. A ResizeObserver
 * attached to the zone's root element keeps the Rust-side rect in sync;
 * the initial rect is registered alongside the zone in the same
 * `spatial_register_zone` call.
 */
export function FocusZone({
  moniker: segment,
  navOverride,
  commands = EMPTY_COMMANDS,
  showFocusBar = true,
  handleEvents = true,
  renderContainer = true,
  children,
  ref: externalRef,
  ...rest
}: FocusZoneProps) {
  // Compose this zone's FQM from the ancestor FQM (the layer root or
  // an enclosing zone) and the consumer's declared segment. The throwing
  // hook variant enforces that every `<FocusZone>` lives inside a
  // `<FocusLayer>` — mounting a zone outside the spatial provider stack
  // is a setup bug and surfaces as a clear error rather than silently
  // degrading to a plain `<div>`.
  const parentFq = useFullyQualifiedMoniker();
  const fq = useMemo<FullyQualifiedMoniker>(
    () => composeFq(parentFq, segment),
    [parentFq, segment],
  );

  // Selective subscription: re-renders only when *this FQM's* focus slot
  // flips. Drives the `scrollIntoView` effect in the body. Returns `false`
  // permanently when no `EntityFocusProvider` is mounted. The entity-focus
  // store is keyed by the FQM (the kernel's identifier).
  const isFocused = useOptionalIsDirectFocus(fq);

  // Build the scope ourselves so we can register it in the entity-focus
  // registry. Same shape as `<FocusScope>` produces, with `moniker`
  // anchoring the chain. The entity-focus scope chain is keyed by the
  // segment moniker (entity identity) — that is independent of the
  // spatial path, and the `<Inspectable>` chain still walks
  // `parent.moniker` strings through the registry to resolve scope
  // bindings.
  const parent = useContext(CommandScopeContext);
  const scope = useMemo<CommandScope>(() => {
    const map = new Map<string, CommandDef>();
    for (const cmd of commands) {
      map.set(cmd.id, cmd);
    }
    return { commands: map, parent, moniker: segment };
  }, [commands, parent, segment]);

  const isDirectFocus = showFocusBar && isFocused;

  // Register the scope in the entity-focus registry via the shared helper.
  useEntityScopeRegistration(fq, scope);

  // `renderContainer={false}` short-circuit: skip the body branch
  // entirely. We still publish the zone's identity downward — the
  // FullyQualifiedMonikerContext provider so descendants compose their
  // FQM under this zone, and the FocusZoneContext provider so descendant
  // primitives' `useParentZoneFq()` resolves to this zone — but no DOM
  // is rendered, no rect is tracked, and no kernel registration happens
  // (there is no node to call `getBoundingClientRect()` on). Mirrors the
  // matching short-circuit in `<FocusScope>`, with the additional
  // FocusZone-only providers added so descendants treat us as a Zone.
  //
  // Provider nesting order matches the full-body branch:
  // `FocusScope > CommandScope > FullyQualifiedMoniker > FocusZone`.
  // Functionally identical to any other order (context lookups don't
  // depend on provider nesting) — kept consistent so the two branches
  // read the same.
  if (!renderContainer) {
    return (
      <FocusScopeContext.Provider value={fq}>
        <CommandScopeContext.Provider value={scope}>
          <FullyQualifiedMonikerContext.Provider value={fq}>
            <FocusZoneContext.Provider value={fq}>
              {children}
            </FocusZoneContext.Provider>
          </FullyQualifiedMonikerContext.Provider>
        </CommandScopeContext.Provider>
      </FocusScopeContext.Provider>
    );
  }

  return (
    <FocusScopeContext.Provider value={fq}>
      <CommandScopeContext.Provider value={scope}>
        <SpatialFocusZoneBody
          fq={fq}
          segment={segment}
          navOverride={navOverride}
          showFocusBar={showFocusBar}
          handleEvents={handleEvents}
          isDirectFocus={isDirectFocus}
          ref={externalRef}
          {...rest}
        >
          {children}
        </SpatialFocusZoneBody>
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
  /** The composed FQM — used as the kernel registry key. */
  fq: FullyQualifiedMoniker;
  /** The consumer-declared segment, sent to the kernel for logging. */
  segment: SegmentMoniker;
  navOverride?: FocusOverrides;
  showFocusBar: boolean;
  /** When false, the body skips click / right-click handling. */
  handleEvents: boolean;
  isDirectFocus: boolean;
  children: ReactNode;
  ref?: Ref<HTMLDivElement>;
}

/**
 * Body branch when a `<FocusLayer>` ancestor IS present.
 *
 * Registers with the Rust-side spatial registry via
 * `spatial_register_zone(fq, segment, ...)`, subscribes to per-FQM
 * focus claims, publishes its FQM via `FullyQualifiedMonikerContext`
 * for descendants, and renders a single `<div>` that carries the
 * consumer's className plus the `data-moniker` / `data-focused`
 * debugging attributes.
 */
function SpatialFocusZoneBody({
  fq,
  segment,
  navOverride,
  showFocusBar,
  handleEvents,
  isDirectFocus,
  children,
  ref: externalRef,
  ...htmlProps
}: SpatialFocusZoneBodyProps) {
  const contextMenuHandler = useContextMenu();
  const focusActions = useOptionalFocusActions();
  const setFocus = focusActions?.setFocus;

  // Resolve the layer FQM by walking up — every primitive lives inside
  // a layer, so the topmost ancestor in the FQM context chain whose
  // path is the closest `/<window>` or `/<window>/<layer>` is the
  // layer FQ. We don't currently track the layer separately; the
  // simplest correct answer is to read it from the same context the
  // ancestor `<FocusLayer>` provides. For now, the layer FQM is the
  // root of the FQM chain — derive it lazily by finding the second
  // separator. That matches the kernel's notion of "layer is the
  // top-of-path segment under the window root".
  //
  // Rather than introduce a separate layer-FQM context, we forward
  // the same FQM pattern the kernel expects: the registration's
  // `layerFq` is the path `/window/<layerName>` (or `/window`). We
  // can derive it from the parent FQM chain by walking until we find
  // an ancestor whose FQM segment matches a known layer name — but
  // the kernel doesn't actually require this be exact. The clean
  // approach is a separate context: see `LayerFqContext` below.
  const layerFq = useEnclosingLayerFq();

  // Read the parent zone (when present) so the registration call can
  // populate the Rust-side `parent_zone` field.
  const parentZone = useParentZoneFq();

  // Ref to the rendered div. Drives the `scrollIntoView` effect plus the
  // ResizeObserver below.
  const ref = useRef<HTMLDivElement | null>(null);

  // Callback ref that writes to the internal `ref` (used by the
  // ResizeObserver and click handler) AND forwards to any external ref the
  // caller passed.
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

  const [focused, setFocused] = useState(false);
  useFocusClaim(fq, setFocused);

  const { registerZone, unregisterScope, updateRect, focus } =
    useSpatialFocusActions();

  // ---------------------------------------------------------------------
  // navOverride contract
  // ---------------------------------------------------------------------
  // `navOverride` is read from a ref and snapshotted into the Rust-side
  // registry **only when the registration effect runs** — i.e. on mount
  // and whenever one of (`fq`, `layerFq`, `parentZone`) flips identity.
  // Mid-life changes to `navOverride` while those deps stay stable are
  // intentionally ignored.
  const navOverrideRef = useRef<FocusOverrides | undefined>(navOverride);
  navOverrideRef.current = navOverride;

  useEffect(() => {
    const node = ref.current;
    if (!node) return;

    const overrides: FocusOverrides = navOverrideRef.current ?? {};
    const initialRect = node.getBoundingClientRect();
    registerZone(
      fq,
      segment,
      {
        x: asPixels(initialRect.x),
        y: asPixels(initialRect.y),
        width: asPixels(initialRect.width),
        height: asPixels(initialRect.height),
      },
      layerFq,
      parentZone,
      overrides,
    ).catch((err) => console.error("[FocusZone] register failed", err));

    const observer = new ResizeObserver(() => {
      const node = ref.current;
      if (!node) return;
      const r = node.getBoundingClientRect();
      updateRect(fq, {
        x: asPixels(r.x),
        y: asPixels(r.y),
        width: asPixels(r.width),
        height: asPixels(r.height),
      }).catch((err) => console.error("[FocusZone] updateRect failed", err));
    });
    observer.observe(node);

    return () => {
      observer.disconnect();
      unregisterScope(fq).catch((err) =>
        console.error("[FocusZone] unregister failed", err),
      );
    };
  }, [
    fq,
    segment,
    layerFq,
    parentZone,
    registerZone,
    unregisterScope,
    updateRect,
  ]);

  // Ancestor-scroll listener: refresh the kernel's rect whenever any
  // scrollable ancestor (or the document) scrolls.
  useTrackRectOnAncestorScroll(ref, fq, updateRect);

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
      if (setFocus) setFocus(fq);
      contextMenuHandler(e);
    },
    [fq, setFocus, contextMenuHandler, handleEvents],
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
      focus(fq).catch((err) => console.error("[FocusZone] focus failed", err));
    },
    [focus, fq, handleEvents],
  );

  // Merge `relative` into the consumer's className.
  const { className: consumerClassName, ...restWithoutClassName } = htmlProps;
  const mergedClassName = cn(consumerClassName, "relative");

  const debugEnabled = useFocusDebug();

  return (
    <FullyQualifiedMonikerContext.Provider value={fq}>
      <FocusZoneContext.Provider value={fq}>
        <div
          ref={setRef}
          data-moniker={fq}
          data-segment={segment}
          data-focused={focused || undefined}
          onClick={handleClick}
          onContextMenu={handleContextMenu}
          {...restWithoutClassName}
          className={mergedClassName}
        >
          {showFocusBar && <FocusIndicator focused={focused} />}
          {debugEnabled && (
            <FocusDebugOverlay kind="zone" label={segment} hostRef={ref} />
          )}
          {children}
        </div>
      </FocusZoneContext.Provider>
    </FullyQualifiedMonikerContext.Provider>
  );
}

// ---------------------------------------------------------------------------
// Layer-FQ re-export
// ---------------------------------------------------------------------------

// `LayerFqContext` lives in its own module (`@/components/layer-fq-context`)
// to avoid a focus-zone ↔ focus-layer import cycle. Re-exported here so
// existing imports of `@/components/focus-zone` can still reach it.
export {
  LayerFqContext,
  useEnclosingLayerFq,
} from "@/components/layer-fq-context";
