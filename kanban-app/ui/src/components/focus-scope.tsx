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
  useDispatchCommand,
  type CommandDef,
  type CommandScope,
} from "@/lib/command-scope";
import { useEntityFocus, useFocusedMoniker } from "@/lib/entity-focus-context";
import { useContextMenu } from "@/lib/context-menu";
import { useFocusLayerKey } from "@/components/focus-layer";
import { ulid } from "ulid";
import { invoke } from "@tauri-apps/api/core";

/**
 * React context that carries the moniker of the nearest ancestor FocusScope.
 * Used by useParentFocusScope() to let children discover their enclosing scope
 * without walking the command scope chain.
 */
const FocusScopeContext = createContext<string | null>(null);

/**
 * React context that carries the `elementRef` of a `FocusScope` rendered
 * with `renderContainer={false}`.
 *
 * When the scope's wrapping element is suppressed (table rows, table
 * cells, or any consumer that owns its own DOM element), the scope has
 * no node to observe for `getBoundingClientRect()`. The consumer must
 * attach `elementRef` to its own element so `ResizeObserver` inside
 * `useSpatialClaim` can measure it and push the rect to Rust.
 *
 * The context is only populated when `renderContainer={false}`. The
 * consumer wires it with `useFocusScopeElementRef()` and sets
 * `ref={ref}` on the element that defines this scope's spatial footprint.
 */
const FocusScopeElementRefContext =
  createContext<React.RefObject<HTMLElement | null> | null>(null);

/**
 * Returns the `elementRef` for the nearest ancestor `FocusScope` that
 * uses `renderContainer={false}`, or `null` when no such scope is
 * active.
 *
 * Consumers (e.g. table rows and cells) call this to attach the ref to
 * their own DOM element so the scope can be measured and registered
 * with the Rust spatial state.
 */
export function useFocusScopeElementRef(): React.RefObject<HTMLElement | null> | null {
  return useContext(FocusScopeElementRefContext);
}

// ---------------------------------------------------------------------------
// Custom hooks — extracted to keep FocusScope and FocusScopeInner under 50 lines
// ---------------------------------------------------------------------------

/** Generate a stable ULID spatial key per mount — new on remount. */
function useSpatialKey(): string {
  const ref = useRef<string | null>(null);
  if (ref.current === null) ref.current = ulid();
  return ref.current;
}

/**
 * Bind this scope's spatial key to its moniker in the EntityFocus
 * key↔moniker registry, and tear down the Tauri spatial entry on unmount.
 *
 * The binding exists so the Rust `focus-changed` event listener can map
 * a spatial key back to the moniker that owns it, and so `spatial_focus`
 * / `spatial_navigate` can pick a key for a given moniker. It does NOT
 * drive visual focus — FocusScope subscribes to the focused-moniker
 * store via `useFocusedMoniker()` and derives `data-focused` by
 * comparing its moniker against the store value on every render.
 *
 * When `layerKey` is `null` (no FocusLayer ancestor), the Tauri spatial
 * entry is never registered; the moniker↔key binding still happens so
 * any focus events that still resolve to this key stay coherent.
 *
 * When `spatial` is `false`, the spatial entry (rect) is never
 * registered — the scope still participates in focus/commands/scope
 * chain, but the Rust beam test does not see it as a navigation target.
 * Used for container scopes like table rows that must not shadow their
 * own cells during cardinal-direction searches.
 */
function useSpatialKeyBinding(
  spatialKey: string,
  moniker: string,
  layerKey: string | null,
  spatial: boolean,
) {
  const { registerSpatialKey, unregisterSpatialKey } = useEntityFocus();
  useEffect(() => {
    registerSpatialKey(spatialKey, moniker);
    return () => {
      unregisterSpatialKey(spatialKey);
      if (layerKey && spatial)
        invoke("spatial_unregister", { key: spatialKey }).catch(() => {});
    };
  }, [
    spatialKey,
    moniker,
    layerKey,
    spatial,
    registerSpatialKey,
    unregisterSpatialKey,
  ]);
}

function useRectObserver(
  elementRef: React.RefObject<HTMLElement | null>,
  spatialKey: string,
  moniker: string,
  layerKey: string | null,
  spatial: boolean,
  parentScope: string | null,
  navOverride?: Record<string, string | null>,
) {
  useEffect(() => {
    if (!layerKey || !spatial) return;
    const el = elementRef.current;
    if (!el) return;
    const report = () => {
      const r = el.getBoundingClientRect();
      // The Rust command takes a single `args` struct so serde aliases accept
      // both camelCase and snake_case on the wire — see `SpatialRegisterArgs`.
      invoke("spatial_register", {
        args: {
          key: spatialKey,
          moniker,
          x: r.x,
          y: r.y,
          w: r.width,
          h: r.height,
          layerKey,
          parentScope,
          overrides: navOverride ?? null,
        },
      }).catch(() => {});
    };
    report();
    const observer = new ResizeObserver(report);
    observer.observe(el);
    return () => observer.disconnect();
  }, [
    elementRef,
    spatialKey,
    moniker,
    layerKey,
    spatial,
    parentScope,
    navOverride,
  ]);
}

/**
 * Compose the moniker↔spatial-key binding with the rect observer that
 * feeds `spatial_register` calls. Returns the `elementRef` the caller
 * must attach to the DOM node it wants Rust to measure.
 *
 * Visual focus state is NOT returned from here; FocusScope derives it
 * directly from the focused-moniker store via `useFocusedMoniker()`.
 */
function useSpatialRegistration(
  spatialKey: string,
  moniker: string,
  layerKey: string | null,
  spatial: boolean,
  parentScope: string | null,
  navOverride?: Record<string, string | null>,
) {
  const elementRef = useRef<HTMLElement | null>(null);
  useSpatialKeyBinding(spatialKey, moniker, layerKey, spatial);
  useRectObserver(
    elementRef,
    spatialKey,
    moniker,
    layerKey,
    spatial,
    parentScope,
    navOverride,
  );
  return elementRef;
}

/**
 * Imperatively mirror the scope's focus state onto its attached DOM
 * element. When `active` is `true` (the scope's moniker matches the
 * focused-moniker store AND the scope opts in via `showFocusBar`),
 * sets `data-focused="true"` on `elementRef.current` and scrolls it
 * into view. When `active` flips to `false`, removes the attribute.
 *
 * `active` is computed by the caller as `focusedMoniker === moniker`
 * from a `useFocusedMoniker()` subscription — the pull-based model.
 * Every scope independently re-derives its own `active` from the
 * single source of truth on every focus change; no push notification
 * is ever fanned out. Stale `data-focused` is impossible by
 * construction: if the store says B is focused, only B evaluates to
 * `true`; everyone else evaluates to `false` and clears the attribute
 * in the same render.
 *
 * This is the single canonical driver of the focus decoration — consumers
 * of `renderContainer={false}` do not manage the attribute themselves.
 * The attribute is written directly to the DOM (not via React state) so
 * the element owns exactly one focus signal, regardless of whether the
 * scope renders its own container or shares a DOM node with a consumer.
 *
 * The `scrollIntoView({ block: "nearest" })` effect was previously in
 * `FocusHighlight` and only applied to `renderContainer={true}` scopes;
 * moving it here gives all decorated scopes the same scroll behavior.
 *
 * When `active` is always `false` (e.g. `showFocusBar={false}`), the
 * effect is a no-op from first render — the attribute never appears.
 */
function useFocusDecoration(
  elementRef: React.RefObject<HTMLElement | null>,
  active: boolean,
) {
  useEffect(() => {
    const el = elementRef.current;
    if (!el) return;
    if (active) {
      el.setAttribute("data-focused", "true");
      el.scrollIntoView?.({ block: "nearest" });
      return () => {
        el.removeAttribute("data-focused");
      };
    }
    // `active === false` on first effect pass — ensure no stale
    // attribute lingers (defensive: covers remounts where the DOM node
    // was reused by React but the cleanup never ran).
    el.removeAttribute("data-focused");
  }, [elementRef, active]);
}

/**
 * Build a `CommandScope` and register it in the EntityFocus scope registry.
 * Returns the built scope.
 */
function useScopeRegistration(
  moniker: string,
  commands: CommandDef[],
): CommandScope {
  const { registerScope, unregisterScope } = useEntityFocus();
  const parent = useContext(CommandScopeContext);

  const scope = useMemo<CommandScope>(() => {
    const map = new Map<string, CommandDef>();
    for (const cmd of commands) map.set(cmd.id, cmd);
    return { commands: map, parent, moniker };
  }, [commands, parent, moniker]);

  useEffect(() => {
    registerScope(moniker, scope);
    return () => unregisterScope(moniker);
  }, [moniker, scope, registerScope, unregisterScope]);

  return scope;
}

/**
 * Returns a click handler that sets focus on the scope's moniker, skipping
 * clicks that originate inside editable controls.
 */
function useScopeClickHandler(moniker: string, handleEvents: boolean) {
  const { setFocus } = useEntityFocus();
  return useCallback(
    (e: React.MouseEvent) => {
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
}

/**
 * Returns context-menu and double-click handlers for FocusScopeInner.
 *
 * Must be called inside `CommandScopeContext.Provider` so `useContextMenu`
 * and `useDispatchCommand` see the correct scope chain.
 */
function useScopeEventHandlers(moniker: string, handleEvents: boolean) {
  const contextMenuHandler = useContextMenu();
  const dispatch = useDispatchCommand("ui.inspect");
  const { setFocus } = useEntityFocus();

  const handleContextMenu = useCallback(
    (e: React.MouseEvent) => {
      if (!handleEvents) return;
      e.preventDefault();
      e.stopPropagation();
      setFocus(moniker);
      contextMenuHandler(e);
    },
    [moniker, setFocus, contextMenuHandler, handleEvents],
  );

  const handleDoubleClick = useCallback(
    (e: React.MouseEvent) => {
      if (!handleEvents) return;
      const target = e.target as HTMLElement;
      const tag = target.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
      if (target.closest("[contenteditable]")) return;
      e.stopPropagation();
      dispatch({ target: moniker }).catch(console.error);
    },
    [dispatch, moniker, handleEvents],
  );

  return { handleContextMenu, handleDoubleClick };
}

// ---------------------------------------------------------------------------
// Component props
// ---------------------------------------------------------------------------

/** Own props for FocusScope; HTML attributes (className, style, data-*) pass through. */
type FocusScopeOwnProps = {
  /** The moniker ("type:id") for the entity this scope represents. */
  moniker: string;
  /** Commands to register in this scope. */
  commands: CommandDef[];
  children: ReactNode;
  /**
   * Directional navigation overrides for this scope.
   *
   * Key is a direction string (e.g. `"Right"`, `"Up"`). Value is a target
   * moniker to redirect navigation to, or `null` to block that direction.
   * Missing keys fall through to spatial navigation.
   *
   * Forwarded to Rust via `spatial_register` as the `overrides` parameter.
   */
  navOverride?: Record<string, string | null>;
  /** When false, suppresses the data-focused attribute (hides the focus bar).
   *  The scope still participates in focus/commands — only the visual indicator is hidden. */
  showFocusBar?: boolean;
  /** When false, suppresses click/right-click/double-click event handling.
   *  Independent of showFocusBar — a scope can handle events without showing the focus bar.
   *  Defaults to true. */
  handleEvents?: boolean;
  /** When false, omits the wrapping `<div>` — children render directly.
   *  Use for table rows where a wrapping div breaks HTML structure.
   *  The scope, moniker registration, and context still work; the caller
   *  must attach onContextMenu etc. to their own element. */
  renderContainer?: boolean;
  /**
   * When false, skip spatial registration (the scope has no rect in the
   * Rust beam-test graph). Defaults to true.
   *
   * Use this for container scopes — table rows, list groups — that must
   * remain focus-aware (claim callback, command scope, scope chain) but
   * whose bounding rect would shadow their own children during
   * cardinal-direction searches. Children that are themselves spatial
   * entries (e.g. per-cell `FocusScope`s) become the only navigation
   * targets inside the container.
   */
  spatial?: boolean;
};

type FocusScopeProps = FocusScopeOwnProps &
  Omit<React.HTMLAttributes<HTMLElement>, keyof FocusScopeOwnProps>;

// ---------------------------------------------------------------------------
// FocusScope — outer component that owns scope registration and spatial state
// ---------------------------------------------------------------------------

/**
 * Combines CommandScopeProvider + entity focus + context menu into one wrapper.
 *
 * - Wraps children in a CommandScopeProvider with the given commands
 * - Sets entity focus on click (but not when clicking inputs/textareas)
 * - Shows native context menu on right-click using commands from the scope chain
 * - Adds data-moniker and data-focused attributes for CSS targeting
 * - Registers/deregisters the scope in the EntityFocus scope registry
 */
export function FocusScope({
  moniker,
  commands,
  children,
  navOverride,
  showFocusBar = true,
  handleEvents = true,
  renderContainer = true,
  spatial = true,
  ...rest
}: FocusScopeProps) {
  const spatialKey = useSpatialKey();
  const layerKey = useFocusLayerKey();
  // parentScope: the enclosing FocusScope's moniker, used by Rust's
  // container-first search so h/j/k/l stays within siblings before falling
  // through to the full layer. Null means this scope is at layer root.
  const parentScope = useContext(FocusScopeContext);
  const elementRef = useSpatialRegistration(
    spatialKey,
    moniker,
    layerKey,
    spatial,
    parentScope,
    navOverride,
  );
  // Pull-based focus decoration: subscribe to the focused-moniker store
  // and ask the idempotent question "is the current focus me?". Every
  // scope re-evaluates on every focus change; stale state is impossible
  // because each scope derives its own visual from the same single
  // source of truth.
  const focusedMoniker = useFocusedMoniker();
  const isFocused = focusedMoniker === moniker;
  useFocusDecoration(elementRef, showFocusBar && isFocused);
  const scope = useScopeRegistration(moniker, commands);
  const handleClick = useScopeClickHandler(moniker, handleEvents);
  // elementRefForContext: only populated for renderContainer={false} — then a
  // descendant (<tr>/<td>/consumer) attaches the ref to its own DOM node via
  // useFocusScopeElementRef() so ResizeObserver can measure it.
  const elementRefForContext = renderContainer ? null : elementRef;

  return (
    <FocusScopeContext.Provider value={moniker}>
      <FocusScopeElementRefContext.Provider value={elementRefForContext}>
        <CommandScopeContext.Provider value={scope}>
          <FocusScopeBody
            moniker={moniker}
            handleEvents={handleEvents}
            renderContainer={renderContainer}
            onClick={handleClick}
            elementRef={elementRef}
            htmlProps={rest}
          >
            {children}
          </FocusScopeBody>
        </CommandScopeContext.Provider>
      </FocusScopeElementRefContext.Provider>
    </FocusScopeContext.Provider>
  );
}

interface FocusScopeBodyProps {
  moniker: string;
  handleEvents: boolean;
  renderContainer: boolean;
  onClick: React.MouseEventHandler<HTMLElement>;
  elementRef: React.RefObject<HTMLElement | null>;
  htmlProps: React.HTMLAttributes<HTMLElement>;
  children: ReactNode;
}

function FocusScopeBody({
  renderContainer,
  children,
  htmlProps,
  ...inner
}: FocusScopeBodyProps) {
  if (!renderContainer) return <>{children}</>;
  return (
    <FocusScopeInner {...inner} {...htmlProps}>
      {children}
    </FocusScopeInner>
  );
}

// ---------------------------------------------------------------------------
// FocusScopeInner — rendered inside CommandScopeContext so hooks see the scope
// ---------------------------------------------------------------------------

/** Props for the inner focus-scope wrapper rendered inside CommandScopeContext. */
interface FocusScopeInnerProps extends Omit<
  React.HTMLAttributes<HTMLElement>,
  "onClick" | "children"
> {
  moniker: string;
  handleEvents: boolean;
  onClick: React.MouseEventHandler<HTMLElement>;
  /** Ref for the DOM element — used by ResizeObserver for rect measurement
   *  and by `useFocusDecoration` for the `data-focused` attribute write. */
  elementRef: React.RefObject<HTMLElement | null>;
  children: ReactNode;
}

/**
 * Inner component rendered inside CommandScopeContext so `useContextMenu`
 * sees the scope.
 *
 * Renders a plain `<div>` container — `data-focused` and scroll-into-view
 * are written imperatively by `useFocusDecoration` in the parent scope,
 * not via a prop on this element. Keeping the JSX free of focus state
 * means the `renderContainer={true}` and `renderContainer={false}` paths
 * share a single decoration mechanism.
 */
function FocusScopeInner({
  moniker,
  handleEvents,
  onClick,
  elementRef,
  children,
  ...htmlProps
}: FocusScopeInnerProps) {
  const { handleContextMenu, handleDoubleClick } = useScopeEventHandlers(
    moniker,
    handleEvents,
  );

  return (
    <div
      ref={elementRef as React.RefObject<HTMLDivElement>}
      data-moniker={moniker}
      onClick={onClick}
      onDoubleClick={handleDoubleClick}
      onContextMenu={handleContextMenu}
      {...htmlProps}
    >
      {children}
    </div>
  );
}

/**
 * Returns the moniker of the nearest ancestor FocusScope, or null.
 * Uses React context so it skips CommandScopeProviders that aren't FocusScopes.
 */
export function useParentFocusScope(): string | null {
  return useContext(FocusScopeContext);
}
