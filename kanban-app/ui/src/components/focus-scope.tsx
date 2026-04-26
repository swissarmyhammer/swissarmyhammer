/**
 * `<FocusScope>` — entity-aware composite that layers entity focus, command
 * scope, and context-menu plumbing on top of the spatial primitives.
 *
 * On the Rust side, `enum FocusScope` is a sum over `Focusable`/`FocusZone`/
 * `FocusLayer`. On the React side, `<FocusScope>` is the entity-aware wrapper
 * most call sites use; under the hood it composes one of the spatial
 * primitives (`<Focusable>` for leaves, `<FocusZone>` for navigable
 * containers) and stacks the additional concerns on top:
 *
 *   - `FocusScopeContext.Provider` so descendants can discover their nearest
 *     enclosing scope without walking the command-scope chain.
 *   - `CommandScopeContext.Provider` so child components participate in
 *     command resolution and the context-menu chain.
 *   - Entity-focus scope-registry registration (the moniker-keyed map used
 *     by `useFocusedScope` and the dispatcher to compute scope chains).
 *   - Right-click → `setFocus(moniker)` + native context menu via
 *     `useContextMenu`.
 *   - Double-click → `ui.inspect` command dispatch.
 *   - Optional `navOverride` per-direction directives forwarded to the
 *     underlying primitive. The Rust spatial-nav kernel runs these as
 *     rule 0 of beam search — a same-layer target moniker redirects,
 *     `null` blocks navigation in that direction, missing keys fall
 *     through to beam search. Replaces the legacy `claimWhen`
 *     predicate-broadcast model.
 *   - `scrollIntoView` when the entity-focus store reports this scope as
 *     directly focused — preserves the legacy "follow the focus bar"
 *     scroll behaviour without emitting a duplicate `data-focused`
 *     attribute (the primitive already owns that).
 *
 * The primitive owns the things the spatial graph cares about: the
 * branded `SpatialKey`, ResizeObserver-driven rect updates, the click →
 * `spatial_focus` invoke, and the per-key `data-focused` toggle from the
 * focus-claim registry. `<FocusScope>` does not duplicate any of those —
 * it composes the primitive once.
 *
 * Use `<FocusScope>` when wrapping an entity (task, column, field row);
 * for non-entity chrome (toolbar buttons, navigation chevrons), reach for
 * `<Focusable>` or `<FocusZone>` directly.
 */

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  type ReactNode,
} from "react";
import {
  CommandScopeContext,
  EMPTY_COMMANDS,
  useDispatchCommand,
  type CommandDef,
  type CommandScope,
} from "@/lib/command-scope";
import { useFocusActions, useIsDirectFocus } from "@/lib/entity-focus-context";
import { useContextMenu } from "@/lib/context-menu";
import { Focusable } from "@/components/focusable";
import { FocusZone } from "@/components/focus-zone";
import { FocusLayerContext } from "@/components/focus-layer";
import type { FocusOverrides, Moniker } from "@/types/spatial";

/**
 * React context that carries the moniker of the nearest ancestor FocusScope.
 * Used by useParentFocusScope() to let children discover their enclosing scope
 * without walking the command scope chain.
 */
const FocusScopeContext = createContext<Moniker | null>(null);

/**
 * Discriminator for which primitive the composite wraps.
 *
 *   - `"leaf"` (default): renders `<Focusable>` — a single keystroke
 *     target, registered as a leaf in the spatial graph.
 *   - `"zone"`: renders `<FocusZone>` — a navigable container that groups
 *     descendant focusables and remembers a `last_focused` slot for
 *     drill-out / fallback memory.
 */
export type FocusScopeKind = "leaf" | "zone";

/** Own props for FocusScope; HTML attributes (className, style, data-*) pass through. */
type FocusScopeOwnProps = {
  /** The branded entity moniker for the scope this represents. */
  moniker: Moniker;
  /**
   * Which primitive to compose. Defaults to `"leaf"` so existing call
   * sites that wrap a single entity (a task title, a tag pill) keep their
   * leaf semantics. Pass `"zone"` for containers (a column body, a
   * board, a field row) that group child focusables.
   */
  kind?: FocusScopeKind;
  /**
   * Per-direction navigation overrides forwarded to the underlying
   * primitive. Missing keys mean "fall through to beam search"; an
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
  /** When false, suppresses the entity-focus-driven scroll-into-view trigger.
   *  The scope still participates in focus/commands — only the visual highlight
   *  follow behaviour is hidden. The primitive's own spatial `data-focused`
   *  remains active regardless. */
  showFocusBar?: boolean;
  /** When false, suppresses click/right-click/double-click event handling.
   *  Independent of showFocusBar — a scope can handle events without showing the focus bar.
   *  Defaults to true. */
  handleEvents?: boolean;
  /** When false, omits the wrapping primitive — children render directly under the
   *  CommandScopeContext + FocusScopeContext providers.
   *
   *  Use for table rows where a wrapping div breaks HTML structure (the row already
   *  IS a focusable container in the table layout). The scope, moniker registration,
   *  and context still work; the caller must attach onContextMenu etc. to their own
   *  element and accepts that the spatial primitive won't register this scope.
   *
   *  This is an escape hatch for the data-table edge case and is retained for
   *  source compatibility. */
  renderContainer?: boolean;
};

type FocusScopeProps = FocusScopeOwnProps &
  Omit<React.HTMLAttributes<HTMLElement>, keyof FocusScopeOwnProps>;

/**
 * Entity-aware composite over the spatial primitives.
 *
 * Wraps children in `<Focusable>` (when `kind === "leaf"`, the default) or
 * `<FocusZone>` (when `kind === "zone"`), then layers the entity-focus
 * registry, command scope, focus highlight, and context-menu plumbing on
 * top. Existing call sites that pass only `moniker` continue to work
 * because the default `kind` is `"leaf"`.
 *
 * Click-to-focus goes through the primitive's `spatial_focus` invoke —
 * the legacy `setFocus(moniker)` path on click is gone. Right-click and
 * double-click still drive the entity-focus + context-menu / inspect
 * dispatch flow that lives in this wrapper, so behaviour for those
 * gestures is unchanged.
 */
export function FocusScope({
  moniker,
  kind = "leaf",
  navOverride,
  commands = EMPTY_COMMANDS,
  children,
  showFocusBar = true,
  handleEvents = true,
  renderContainer = true,
  ...rest
}: FocusScopeProps) {
  const { registerScope, unregisterScope } = useFocusActions();

  // Selective subscription: this scope re-renders only when *its own*
  // moniker's focus slot flips — not on every focus move elsewhere in the
  // tree. In a 12k-cell grid, one arrow-key press wakes only two of these.
  // Drives the scroll-into-view effect in `FocusScopeChrome` so the
  // legacy "follow the focus" behaviour keeps firing when the entity
  // focus store reports this moniker as directly focused.
  const isFocused = useIsDirectFocus(moniker);

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

  // Register the scope in the EntityFocus registry.
  //
  // The effect's dep list is deliberately `[moniker]` — NOT `[scope]`. The
  // scope object's identity churns whenever `parent` (from
  // `useContext(CommandScopeContext)`) changes, which happens on every
  // ancestor-scope rebuild. Re-firing the effect on every such churn
  // produces 12k unregister/register pairs per grid render on a 2000-row
  // board, flooding React's commit phase with cleanups and freezing the UI.
  //
  // Instead we hold the latest scope in a ref (updated every render) and
  // re-register on every render inline via `registerScope` — registration
  // is a plain `Map.set`, not a React effect, so it does not pay React's
  // per-effect overhead. Cleanup still runs on real unmount.
  const scopeRef = useRef(scope);
  scopeRef.current = scope;
  registerScope(moniker, scope);
  useEffect(() => {
    // Re-register on mount (and when moniker changes) to cover the initial
    // paint path where the inline call above has already run but React may
    // have discarded the render in StrictMode.
    registerScope(moniker, scopeRef.current);
    return () => unregisterScope(moniker);
  }, [moniker, registerScope, unregisterScope]);

  return (
    <FocusScopeContext.Provider value={moniker}>
      <CommandScopeContext.Provider value={scope}>
        {renderContainer ? (
          <FocusScopeChrome
            moniker={moniker}
            kind={kind}
            navOverride={navOverride}
            showFocusBar={showFocusBar}
            isDirectFocus={isDirectFocus}
            handleEvents={handleEvents}
            {...rest}
          >
            {children}
          </FocusScopeChrome>
        ) : (
          children
        )}
      </CommandScopeContext.Provider>
    </FocusScopeContext.Provider>
  );
}

/** Props for the inner component that owns the primitive + chrome composition. */
interface FocusScopeChromeProps extends Omit<
  React.HTMLAttributes<HTMLElement>,
  "onClick" | "children"
> {
  moniker: Moniker;
  kind: FocusScopeKind;
  navOverride?: FocusOverrides;
  /**
   * When false, suppresses the visible `<FocusIndicator>` rendered by the
   * underlying spatial primitive. The primitive still subscribes to focus
   * claims and emits `data-focused` so tests / e2e selectors keep working;
   * only the visual bar is hidden. Forwarded straight through to the
   * primitive's matching prop.
   *
   * Independently disables the entity-focus-driven `scrollIntoView` effect
   * (the legacy "follow the focus" scroll), so callers that pass
   * `showFocusBar={false}` get both behaviours suppressed in step.
   */
  showFocusBar: boolean;
  /** Whether the entity-focus store reports this scope as directly focused. */
  isDirectFocus: boolean;
  /** When false, suppresses right-click and double-click event handling. */
  handleEvents: boolean;
  children: ReactNode;
}

/**
 * Inner component that composes the spatial primitive with the entity
 * chrome (right-click context menu, double-click → inspect, scroll-into-
 * view). Rendered inside `CommandScopeContext.Provider` so
 * `useContextMenu` and `useDispatchCommand` see the fresh scope.
 *
 * The chrome is attached **directly** to the spatial primitive's root
 * `<div>` — there is intentionally no inner wrapper element. An inner
 * wrapper used to live here (`FocusScopeBody`) but its plain block-display
 * default broke the flex chain for any consumer that wanted
 * `<FocusScope className="flex …">` to lay out children as flex items.
 * Routing the chrome through the primitive keeps the consumer's layout
 * contract: whatever `className` they pass lands on a single element and
 * its children are direct layout children of that element.
 *
 * The primitive owns `data-moniker`, `data-focused` (driven by
 * `useFocusClaim`), the ResizeObserver-driven rect updates, and the
 * click → `spatial_focus` invoke. We forward a ref into the primitive so
 * the `scrollIntoView` effect can target the same element that carries
 * the focus-bar styling.
 */
function FocusScopeChrome({
  moniker,
  kind,
  navOverride,
  showFocusBar,
  isDirectFocus,
  handleEvents,
  children,
  ...htmlProps
}: FocusScopeChromeProps) {
  const contextMenuHandler = useContextMenu();
  const dispatch = useDispatchCommand("ui.inspect");
  const { setFocus } = useFocusActions();

  // Detect whether a `<FocusLayer>` ancestor is mounted. Production code
  // (App.tsx and the quick-capture window) wraps everything in one, but
  // many isolated tests render components without the spatial-focus
  // provider stack. When no layer is present, the spatial primitives
  // (`<Focusable>` / `<FocusZone>`) would throw via `useCurrentLayerKey`,
  // so we degrade to a plain `<div>` here — the entity-focus chrome
  // (CommandScope, claim registry, right-click, double-click) still
  // works, only the spatial registration is skipped. This preserves
  // compatibility with the large set of unit tests that mount one
  // FocusScope-using component at a time without standing up the
  // spatial provider stack.
  const layerKey = useContext(FocusLayerContext);
  const hasSpatialContext = layerKey !== null;

  // Ref to the rendered div (either the no-spatial-context fallback or
  // the primitive's root). Drives the `scrollIntoView` effect below so
  // the focused scope scrolls itself into view in step with the entity-
  // focus bar.
  const ref = useRef<HTMLDivElement | null>(null);

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
      setFocus(moniker);
      contextMenuHandler(e);
    },
    [moniker, setFocus, contextMenuHandler, handleEvents],
  );

  const handleClickFallback = useCallback(
    (e: React.MouseEvent) => {
      // Used only on the fallback (no-spatial-context) path. When the
      // primitive is mounted, it owns the click handler and invokes
      // `spatial_focus` directly; this fallback preserves the legacy
      // entity-focus-on-click behaviour for tests that don't stand up
      // the spatial stack.
      if (!handleEvents) return;
      const target = e.target as HTMLElement;
      const tag = target.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
      if (target.closest("[contenteditable]")) return;
      e.stopPropagation();
      setFocus(moniker);
    },
    [moniker, setFocus, handleEvents],
  );

  const handleDoubleClick = useCallback(
    (e: React.MouseEvent) => {
      // When handleEvents is false, let the event propagate to the parent
      // entity scope (e.g. EntityRow).
      if (!handleEvents) return;

      // Skip if target is an interactive element
      const target = e.target as HTMLElement;
      const tag = target.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
      if (target.closest("[contenteditable]")) return;

      e.stopPropagation();
      dispatch({ target: moniker }).catch(console.error);
    },
    [dispatch, moniker, handleEvents],
  );

  // No-spatial-context fallback: render a plain `<div>` so consumers that
  // mount FocusScope outside a `<FocusLayer>` (typically isolated unit
  // tests) keep working. Click drives the legacy `setFocus(moniker)`
  // path so right-click / double-click flows still see the moniker as
  // focused; this matches the pre-refactor behaviour of FocusScope.
  //
  // Visible focus bar is intentionally NOT rendered here — this branch
  // runs only in unit tests that don't stand up the spatial provider
  // stack, so there is no `useFocusClaim` subscription to drive a
  // `<FocusIndicator>` and no Rust-side focus state to follow. Tests
  // exercising this path assert against the output-only `data-focused`
  // attribute directly. Production never enters this branch — App.tsx
  // and the quick-capture window always wrap their tree in a
  // `<FocusLayer>`.
  if (!hasSpatialContext) {
    return (
      <div
        ref={ref}
        data-moniker={moniker}
        data-focused={isDirectFocus || undefined}
        {...htmlProps}
        onClick={handleClickFallback}
        onDoubleClick={handleDoubleClick}
        onContextMenu={handleContextMenu}
      >
        {children}
      </div>
    );
  }

  // Pick the spatial primitive based on `kind`. Both share the same prop
  // shape (`moniker`, `navOverride`, `showFocusBar`, passthrough HTML
  // attrs minus `onClick`), so we forward to whichever the caller
  // selected.
  const Primitive = kind === "zone" ? FocusZone : Focusable;

  // The primitive renders a div carrying `data-moniker` and `data-focused`
  // (the latter driven by the spatial-focus claim registry). HTML attrs
  // — className, data-testid, style, etc. — flow onto the same element so
  // the existing surface keeps working unchanged. Right-click,
  // double-click, and our scroll-into-view ref attach to that same
  // element, so children render as direct layout children of the element
  // the consumer styled. The `showFocusBar` prop forwards to the primitive
  // so the visible `<FocusIndicator>` is suppressed in step with the
  // entity-focus scrollIntoView effect.
  return (
    <Primitive
      ref={ref}
      moniker={moniker}
      navOverride={navOverride}
      showFocusBar={showFocusBar}
      {...htmlProps}
      onContextMenu={handleContextMenu}
      onDoubleClick={handleDoubleClick}
    >
      {children}
    </Primitive>
  );
}

/**
 * Returns the moniker of the nearest ancestor FocusScope, or null.
 * Uses React context so it skips CommandScopeProviders that aren't FocusScopes.
 */
export function useParentFocusScope(): Moniker | null {
  return useContext(FocusScopeContext);
}
