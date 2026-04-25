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
import {
  useFocusActions,
  useIsDirectFocus,
  type ClaimPredicate,
} from "@/lib/entity-focus-context";
import { useContextMenu } from "@/lib/context-menu";
import { FocusHighlight } from "@/components/ui/focus-highlight";

/**
 * React context that carries the moniker of the nearest ancestor FocusScope.
 * Used by useParentFocusScope() to let children discover their enclosing scope
 * without walking the command scope chain.
 */
const FocusScopeContext = createContext<string | null>(null);

/** Own props for FocusScope; HTML attributes (className, style, data-*) pass through. */
type FocusScopeOwnProps = {
  /** The moniker ("type:id") for the entity this scope represents. */
  moniker: string;
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
   * Predicates that let this scope claim focus when a nav command is broadcast.
   * Callers must memoize this array (e.g. with useMemo) to avoid unnecessary
   * effect re-runs on every render.
   */
  claimWhen?: ClaimPredicate[];
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
  commands = EMPTY_COMMANDS,
  children,
  claimWhen,
  showFocusBar = true,
  handleEvents = true,
  renderContainer = true,
  ...rest
}: FocusScopeProps) {
  const {
    setFocus,
    registerScope,
    unregisterScope,
    registerClaimPredicates,
    unregisterClaimPredicates,
  } = useFocusActions();

  // Selective subscription: this scope re-renders only when *its own*
  // moniker's focus slot flips — not on every focus move elsewhere in the
  // tree. In a 12k-cell grid, one arrow-key press wakes only two of these.
  const isFocused = useIsDirectFocus(moniker);

  // Build the scope ourselves so we can register it
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
  //
  // This keeps the registry always in sync with the latest scope reference
  // while the effect only fires when the moniker actually changes (mount /
  // unmount / moniker swap).
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

  // Register claim predicates with the same ref-pinned pattern as `scope`
  // above: `claimWhen` is typically a 2D-array slice rebuilt upstream on
  // every grid render, so depending on its identity in the effect deps
  // causes the same 12k unregister/register flood. Hold the latest value
  // in a ref, write through on every render (plain `Map.set`), and keep
  // the effect deps narrow so only the actual unmount path runs cleanup.
  const claimWhenRef = useRef(claimWhen);
  claimWhenRef.current = claimWhen;
  if (claimWhen && claimWhen.length > 0) {
    registerClaimPredicates(moniker, claimWhen);
  }
  useEffect(() => {
    const current = claimWhenRef.current;
    if (current && current.length > 0) {
      registerClaimPredicates(moniker, current);
      return () => unregisterClaimPredicates(moniker);
    }
  }, [moniker, registerClaimPredicates, unregisterClaimPredicates]);

  const handleClick = useCallback(
    (e: React.MouseEvent) => {
      // When handleEvents is false, don't claim focus on click — let the
      // event propagate to the parent FocusScope (e.g. grid cell, card).
      if (!handleEvents) return;

      // Don't change entity focus when clicking inputs/textareas/selects
      const target = e.target as HTMLElement;
      const tag = target.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
      // Don't change if the target is inside a contenteditable
      if (target.closest("[contenteditable]")) return;

      e.stopPropagation();
      setFocus(moniker);
    },
    [moniker, setFocus, handleEvents],
  );

  // Provide the scope via CommandScopeContext directly (not CommandScopeProvider)
  // so we have access to the scope object for registry
  return (
    <FocusScopeContext.Provider value={moniker}>
      <CommandScopeContext.Provider value={scope}>
        {renderContainer ? (
          <FocusScopeInner
            moniker={moniker}
            isDirectFocus={isDirectFocus}
            showFocusBar={showFocusBar}
            handleEvents={handleEvents}
            onClick={handleClick}
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

/** Props for the inner focus-scope wrapper rendered inside CommandScopeContext. */
interface FocusScopeInnerProps extends Omit<
  React.HTMLAttributes<HTMLElement>,
  "onClick" | "children"
> {
  moniker: string;
  isDirectFocus: boolean;
  /** When false, hides the focus bar visual indicator. */
  showFocusBar: boolean;
  /** When false, suppresses click/right-click/double-click event handling. */
  handleEvents: boolean;
  onClick: React.MouseEventHandler<HTMLElement>;
  children: ReactNode;
}

/** Inner component rendered inside CommandScopeContext so useContextMenu sees the scope. */
function FocusScopeInner({
  moniker,
  isDirectFocus,
  showFocusBar,
  handleEvents,
  onClick,
  children,
  ...htmlProps
}: FocusScopeInnerProps) {
  const contextMenuHandler = useContextMenu();
  const dispatch = useDispatchCommand("ui.inspect");
  const { setFocus } = useFocusActions();

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

  return (
    <FocusHighlight
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
