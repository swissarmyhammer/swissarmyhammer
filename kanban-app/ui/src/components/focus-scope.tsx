import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import {
  CommandScopeContext,
  useDispatchCommand,
  type CommandDef,
  type CommandScope,
} from "@/lib/command-scope";
import {
  useEntityFocus,
} from "@/lib/entity-focus-context";
import { useContextMenu } from "@/lib/context-menu";
import { FocusHighlight } from "@/components/ui/focus-highlight";
import { useFocusLayerKey } from "@/components/focus-layer";
import { ulid } from "ulid";
import { invoke } from "@tauri-apps/api/core";

/**
 * React context that carries the moniker of the nearest ancestor FocusScope.
 * Used by useParentFocusScope() to let children discover their enclosing scope
 * without walking the command scope chain.
 */
const FocusScopeContext = createContext<string | null>(null);

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
 * Register a spatial focus claim and Tauri spatial entry for this key.
 * Measures the element rect via ResizeObserver and reports updates to Rust.
 * When `layerKey` is `null` (no FocusLayer ancestor), spatial registration
 * is skipped — the claim callback still works for focus events.
 *
 * The optional `navOverride` map is forwarded to Rust as `overrides` on each
 * spatial registration call, enabling per-scope navigation redirection or blocking.
 *
 * Returns `{ isClaimed, elementRef }` — attach `elementRef` to the DOM node.
 */
function useSpatialClaim(
  spatialKey: string,
  moniker: string,
  layerKey: string | null,
  navOverride?: Record<string, string | null>,
) {
  const { registerClaim, unregisterClaim } = useEntityFocus();
  const [isClaimed, setIsClaimed] = useState(false);
  const elementRef = useRef<HTMLElement | null>(null);

  // Register claim callback (always — works even without FocusLayer).
  useEffect(() => {
    registerClaim(spatialKey, moniker, setIsClaimed);
    return () => {
      unregisterClaim(spatialKey);
      if (layerKey) invoke("spatial_unregister", { key: spatialKey }).catch(() => {});
    };
  }, [spatialKey, moniker, layerKey, registerClaim, unregisterClaim]);

  // ResizeObserver: measure DOM rect and report to Rust on mount + resize.
  // Skipped when no FocusLayer is present (layerKey is null).
  useEffect(() => {
    if (!layerKey) return;
    const el = elementRef.current;
    if (!el) return;
    const report = () => {
      const r = el.getBoundingClientRect();
      invoke("spatial_register", {
        key: spatialKey,
        moniker,
        x: r.x,
        y: r.y,
        w: r.width,
        h: r.height,
        layer_key: layerKey,
        parent_scope: null,
        overrides: navOverride ?? null,
      }).catch(() => {});
    };
    report();
    const observer = new ResizeObserver(report);
    observer.observe(el);
    return () => observer.disconnect();
  }, [spatialKey, moniker, layerKey, navOverride]);

  return { isClaimed, elementRef };
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
  /** When false, omits the wrapping FocusHighlight div — children render directly.
   *  Use for table rows where a wrapping div breaks HTML structure.
   *  The scope, moniker registration, and context still work; the caller
   *  must attach onContextMenu etc. to their own element. */
  renderContainer?: boolean;
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
  ...rest
}: FocusScopeProps) {
  const spatialKey = useSpatialKey();
  const layerKey = useFocusLayerKey();
  const { isClaimed, elementRef } = useSpatialClaim(spatialKey, moniker, layerKey, navOverride);
  const scope = useScopeRegistration(moniker, commands);
  const handleClick = useScopeClickHandler(moniker, handleEvents);
  const isDirectFocus = showFocusBar && isClaimed;

  return (
    <FocusScopeContext.Provider value={moniker}>
      <CommandScopeContext.Provider value={scope}>
        {renderContainer ? (
          <FocusScopeInner
            moniker={moniker}
            isDirectFocus={isDirectFocus}
            handleEvents={handleEvents}
            onClick={handleClick}
            elementRef={elementRef}
            {...rest}
          >
            {children}
          </FocusScopeInner>
        ) : (
          children
        )}
      </CommandScopeContext.Provider>
    </FocusScopeContext.Provider>
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
  isDirectFocus: boolean;
  handleEvents: boolean;
  onClick: React.MouseEventHandler<HTMLElement>;
  /** Ref for the DOM element — used by ResizeObserver for rect measurement. */
  elementRef: React.RefObject<HTMLElement | null>;
  children: ReactNode;
}

/** Inner component rendered inside CommandScopeContext so useContextMenu sees the scope. */
function FocusScopeInner({
  moniker,
  isDirectFocus,
  handleEvents,
  onClick,
  elementRef,
  children,
  ...htmlProps
}: FocusScopeInnerProps) {
  const { handleContextMenu, handleDoubleClick } =
    useScopeEventHandlers(moniker, handleEvents);

  return (
    <FocusHighlight
      ref={elementRef}
      focused={isDirectFocus}
      data-moniker={moniker}
      onClick={onClick}
      onDoubleClick={handleDoubleClick}
      onContextMenu={handleContextMenu}
      {...htmlProps}
    >
      {children}
    </FocusHighlight>
  );
}

/**
 * Returns the moniker of the nearest ancestor FocusScope, or null.
 * Uses React context so it skips CommandScopeProviders that aren't FocusScopes.
 */
export function useParentFocusScope(): string | null {
  return useContext(FocusScopeContext);
}
