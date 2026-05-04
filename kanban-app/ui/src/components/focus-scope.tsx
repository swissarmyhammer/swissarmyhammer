/**
 * `<FocusScope>` — the leaf primitive in the spatial-nav graph and the
 * entity-aware focus point most call sites use.
 *
 * `<FocusScope>` is a **pure spatial primitive**. It does NOT know about
 * inspectable entities and does NOT dispatch `ui.inspect`. Inspector
 * dispatch lives in `<Inspectable>` — see `inspectable.tsx`.
 *
 * # Path-monikers identity model
 *
 * After card `01KQD6064G1C1RAXDFPJVT1F46` the spatial graph uses one
 * identifier shape per primitive: `FullyQualifiedMoniker`. The FQM is
 * the spatial key — there is no separate UUID. The scope reads its
 * parent FQM from `FullyQualifiedMonikerContext`, composes its own
 * FQM as `<parentFq>/<segment>`, and registers with the kernel via
 * `spatial_register_scope(fq, segment, rect, layerFq, parentZone,
 * overrides)`. There is no `crypto.randomUUID()` on the React side.
 *
 * Three peers, not four: the spatial-nav kernel exposes `<FocusLayer>`
 * (modal boundary), `<FocusZone>` (navigable container), and
 * `<FocusScope>` (leaf). This component is the leaf:
 *
 *   - Composes its FQM via `useFullyQualifiedMoniker()` + the consumer's
 *     `moniker` segment, then registers with Rust via
 *     `spatial_register_scope`.
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
 *     nearest enclosing scope without walking the command-scope chain.
 *   - Registers with the entity-focus scope registry so `useFocusedScope`
 *     and the dispatcher can compute scope chains.
 *   - Optional `navOverride` per-direction directives forwarded into the
 *     Rust-side registry.
 *   - `scrollIntoView` when the entity-focus store reports this scope as
 *     directly focused — preserves the legacy "follow the focus bar"
 *     scroll behavior.
 *
 * For containers (board, column, grid, perspective, view, nav-bar,
 * toolbar group), use `<FocusZone>` directly. For modal boundaries
 * (window root, inspector, dialog), use `<FocusLayer>` directly.
 *
 * # Scope-is-leaf invariant (enforced by the kernel)
 *
 * `<FocusScope>` MUST be a **leaf** in the spatial graph — its subtree
 * may contain DOM elements but MUST NOT contain further `<FocusScope>`
 * or `<FocusZone>` registrations. Registering a `<FocusScope>` whose
 * subtree contains further `<FocusScope>` or `<FocusZone>` is a kernel
 * error and is logged as `scope-not-leaf` to `just logs`. Grep for the
 * literal token to find the offending wrapper:
 *
 * ```bash
 * just logs | grep scope-not-leaf
 * ```
 *
 * The fix is always to promote the misused `<FocusScope>` to a
 * `<FocusZone>` and add inner `<FocusScope>` leaves around the actual
 * interactive elements. Mirror the navbar's
 * `<FocusZone moniker="ui:navbar">`-with-leaves pattern, or the
 * perspective-tab-bar's `<FocusZone moniker="ui:perspective-bar">`
 * with per-tab leaf scopes.
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
import {
  FullyQualifiedMonikerContext,
  useFullyQualifiedMoniker,
} from "@/components/fully-qualified-moniker-context";
import { useEnclosingLayerFq } from "@/components/layer-fq-context";
import { useParentZoneFq } from "@/components/focus-zone";
import { FocusDebugOverlay } from "@/components/focus-debug-overlay";
import { FocusIndicator } from "@/components/focus-indicator";
import {
  FocusScopeContext,
  useParentFocusScope,
} from "@/components/focus-scope-context";
import { useTrackRectOnAncestorScroll } from "@/components/use-track-rect-on-ancestor-scroll";
import {
  asPixels,
  composeFq,
  type FocusOverrides,
  type FullyQualifiedMoniker,
  type SegmentMoniker,
} from "@/types/spatial";

// `FocusScopeContext` is shared with `<FocusZone>` via `./focus-scope-context`.
// Both push the nearest entity moniker so descendants resolve without walking
// the command-scope chain.

/** Own props for `<FocusScope>`; standard HTML attributes (className, style, data-*) pass through. */
export interface FocusScopeOwnProps {
  /**
   * Relative `SegmentMoniker` for the scope this represents — e.g.
   * `"task:01ABC"`, `"toolbar.button:new"`. The scope's full FQM is
   * composed by appending this segment to the parent FQM read from
   * `FullyQualifiedMonikerContext`.
   */
  moniker: SegmentMoniker;
  /**
   * Per-direction navigation overrides forwarded into the Rust-side
   * registry. Missing keys mean "fall through to beam search"; an
   * explicit `null` blocks navigation in that direction; an FQM value
   * redirects.
   */
  navOverride?: FocusOverrides;
  /**
   * Commands to register in this scope. Optional — defaults to the shared
   * `EMPTY_COMMANDS` constant from command-scope.
   */
  commands?: readonly CommandDef[];
  children: ReactNode;
  /**
   * When false, suppresses both the visible `<FocusIndicator>` and the
   * entity-focus-driven `scrollIntoView` effect. Defaults to true.
   */
  showFocusBar?: boolean;
  /**
   * When false, suppresses click / right-click / double-click event
   * handling. Independent of `showFocusBar`. Defaults to true.
   */
  handleEvents?: boolean;
  /**
   * When false, omits the wrapping primitive — children render directly
   * under the CommandScopeContext + FocusScopeContext providers.
   *
   * Use for table rows where a wrapping div breaks HTML structure.
   */
  renderContainer?: boolean;
  /** Optional ref to the rendered `<div>` element. */
  ref?: Ref<HTMLDivElement>;
}

/**
 * Full props for `<FocusScope>` — `FocusScopeOwnProps` + passthrough HTML attrs.
 *
 * `onClick` is intentionally omitted from the passthrough: the primitive owns
 * the click handler so it can call `spatial_focus`.
 */
export type FocusScopeProps = FocusScopeOwnProps &
  Omit<HTMLAttributes<HTMLDivElement>, keyof FocusScopeOwnProps | "onClick">;

/**
 * Mounts a leaf focus scope in the Rust-side spatial graph and layers
 * the entity-focus / command-scope / context-menu chrome on top.
 *
 * The FQM is composed deterministically from the parent FQM context plus
 * the consumer's `moniker` segment — no UUID minting. A ResizeObserver
 * attached to the root element keeps the Rust-side rect in sync. The
 * component re-renders only when its own focus claim flips.
 */
export function FocusScope({
  moniker: segment,
  navOverride,
  commands = EMPTY_COMMANDS,
  children,
  showFocusBar = true,
  handleEvents = true,
  renderContainer = true,
  ref: externalRef,
  ...rest
}: FocusScopeProps) {
  // Compose this scope's FQM from the ancestor FQM + the segment. The
  // throwing hook variant enforces that every `<FocusScope>` lives inside
  // a `<FocusLayer>` — mounting a scope outside the spatial provider
  // stack is a setup bug and surfaces as a clear error rather than
  // silently degrading to a plain `<div>`.
  const parentFq = useFullyQualifiedMoniker();
  const fq = useMemo<FullyQualifiedMoniker>(
    () => composeFq(parentFq, segment),
    [parentFq, segment],
  );

  // Selective subscription: this scope re-renders only when *its own*
  // focus slot flips — not on every focus move elsewhere in the tree.
  const isFocused = useOptionalIsDirectFocus(fq);

  // Build the scope ourselves so we can register it in the entity-focus
  // registry. Same shape as CommandScopeProvider produces, but we control
  // the lifecycle so the registry stays in lockstep.
  const parent = useContext(CommandScopeContext);
  const scope = useMemo<CommandScope>(() => {
    const map = new Map<string, CommandDef>();
    for (const cmd of commands) {
      map.set(cmd.id, cmd);
    }
    return { commands: map, parent, moniker: segment };
  }, [commands, parent, segment]);

  const isDirectFocus = showFocusBar && isFocused;

  // Register the scope in the entity-focus registry.
  useEntityScopeRegistration(fq, scope);

  return (
    <FocusScopeContext.Provider value={fq}>
      <CommandScopeContext.Provider value={scope}>
        {!renderContainer ? (
          children
        ) : (
          <SpatialFocusScopeBody
            fq={fq}
            segment={segment}
            navOverride={navOverride}
            showFocusBar={showFocusBar}
            isDirectFocus={isDirectFocus}
            handleEvents={handleEvents}
            ref={externalRef}
            {...rest}
          >
            {children}
          </SpatialFocusScopeBody>
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
  fq: FullyQualifiedMoniker;
  segment: SegmentMoniker;
  navOverride?: FocusOverrides;
  showFocusBar: boolean;
  isDirectFocus: boolean;
  handleEvents: boolean;
  children: ReactNode;
  ref?: Ref<HTMLDivElement>;
}

/**
 * Body branch when a `<FocusLayer>` ancestor IS present.
 *
 * Registers with the Rust-side spatial registry via
 * `spatial_register_scope(fq, segment, ...)`, subscribes to per-FQM
 * focus claims, and renders a single `<div>` that carries the
 * consumer's className plus the `data-moniker` / `data-focused`
 * debugging attributes.
 */
function SpatialFocusScopeBody({
  fq,
  segment,
  navOverride,
  showFocusBar,
  isDirectFocus,
  handleEvents,
  children,
  ref: externalRef,
  ...htmlProps
}: SpatialFocusScopeBodyProps) {
  const contextMenuHandler = useContextMenu();
  const focusActions = useOptionalFocusActions();
  const setFocus = focusActions?.setFocus;

  const layerFq = useEnclosingLayerFq();
  const parentZone = useParentZoneFq();

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

  const [focused, setFocused] = useState(false);
  useFocusClaim(fq, setFocused);

  const {
    registerScope: registerSpatialScope,
    unregisterScope,
    updateRect,
    focus,
  } = useSpatialFocusActions();

  // ---------------------------------------------------------------------
  // navOverride contract
  // ---------------------------------------------------------------------
  // `navOverride` is read from a ref and snapshotted into the Rust-side
  // registry **only when the registration effect runs**. Mid-life changes
  // are intentionally ignored.
  const navOverrideRef = useRef<FocusOverrides | undefined>(navOverride);
  navOverrideRef.current = navOverride;

  useEffect(() => {
    const node = ref.current;
    if (!node) return;

    const overrides: FocusOverrides = navOverrideRef.current ?? {};
    const initialRect = node.getBoundingClientRect();
    // Capture `performance.now()` adjacent to the rect read so the
    // dev-mode staleness check (`rect-validation.ts`) can compare
    // against the validator's `nowMs` and surface rects that age
    // between sample and IPC dispatch.
    const initialSampledAtMs = performance.now();
    registerSpatialScope(
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
      initialSampledAtMs,
    ).catch((err) => console.error("[FocusScope] register failed", err));

    const observer = new ResizeObserver(() => {
      const node = ref.current;
      if (!node) return;
      const r = node.getBoundingClientRect();
      const sampledAtMs = performance.now();
      updateRect(
        fq,
        {
          x: asPixels(r.x),
          y: asPixels(r.y),
          width: asPixels(r.width),
          height: asPixels(r.height),
        },
        sampledAtMs,
      ).catch((err) => console.error("[FocusScope] updateRect failed", err));
    });
    observer.observe(node);

    return () => {
      observer.disconnect();
      unregisterScope(fq).catch((err) =>
        console.error("[FocusScope] unregister failed", err),
      );
    };
  }, [
    fq,
    segment,
    layerFq,
    parentZone,
    registerSpatialScope,
    unregisterScope,
    updateRect,
  ]);

  useTrackRectOnAncestorScroll(ref, fq, updateRect);

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
      if (setFocus) setFocus(fq);
      contextMenuHandler(e);
    },
    [fq, setFocus, contextMenuHandler, handleEvents],
  );

  const handleClick = useCallback(
    (e: React.MouseEvent) => {
      const target = e.target as HTMLElement;
      const tag = target.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
      if (target.closest("[contenteditable]")) return;
      e.stopPropagation();
      focus(fq).catch((err) => console.error("[FocusScope] focus failed", err));
    },
    [focus, fq],
  );

  const { className: consumerClassName, ...restWithoutClassName } = htmlProps;
  const mergedClassName = cn(consumerClassName, "relative");

  const debugEnabled = useFocusDebug();

  return (
    <FullyQualifiedMonikerContext.Provider value={fq}>
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
          <FocusDebugOverlay kind="scope" label={segment} hostRef={ref} />
        )}
        {children}
      </div>
    </FullyQualifiedMonikerContext.Provider>
  );
}

// `useParentFocusScope` is exported as a re-export so existing import
// paths (`@/components/focus-scope`) keep resolving.
export { useParentFocusScope };
