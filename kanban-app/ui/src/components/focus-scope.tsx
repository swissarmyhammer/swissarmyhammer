import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  type ReactNode,
} from "react";
import {
  CommandScopeContext,
  resolveCommand,
  dispatchCommand,
  type CommandDef,
  type CommandScope,
} from "@/lib/command-scope";
import {
  useEntityFocus,
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
  /** Commands to register in this scope. */
  commands: CommandDef[];
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
  commands,
  children,
  claimWhen,
  showFocusBar = true,
  ...rest
}: FocusScopeProps) {
  const {
    focusedMoniker,
    setFocus,
    registerScope,
    unregisterScope,
    registerClaimPredicates,
    unregisterClaimPredicates,
  } = useEntityFocus();

  // Build the scope ourselves so we can register it
  const parent = useContext(CommandScopeContext);
  const scope = useMemo<CommandScope>(() => {
    const map = new Map<string, CommandDef>();
    for (const cmd of commands) {
      map.set(cmd.id, cmd);
    }
    return { commands: map, parent, moniker };
  }, [commands, parent, moniker]);

  const isDirectFocus = showFocusBar && focusedMoniker === moniker;

  // Register/deregister scope in the EntityFocus registry
  useEffect(() => {
    registerScope(moniker, scope);
    return () => unregisterScope(moniker);
  }, [moniker, scope, registerScope, unregisterScope]);

  // Register/deregister claim predicates when claimWhen is provided
  useEffect(() => {
    if (claimWhen && claimWhen.length > 0) {
      registerClaimPredicates(moniker, claimWhen);
      return () => unregisterClaimPredicates(moniker);
    }
  }, [moniker, claimWhen, registerClaimPredicates, unregisterClaimPredicates]);

  const handleClick = useCallback(
    (e: React.MouseEvent) => {
      // When showFocusBar is false, don't claim focus on click — let the
      // event propagate to the parent FocusScope (e.g. grid cell, card).
      if (!showFocusBar) return;

      // Don't change entity focus when clicking inputs/textareas/selects
      const target = e.target as HTMLElement;
      const tag = target.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
      // Don't change if the target is inside a contenteditable
      if (target.closest("[contenteditable]")) return;

      e.stopPropagation();
      setFocus(moniker);
    },
    [moniker, setFocus, showFocusBar],
  );

  // Provide the scope via CommandScopeContext directly (not CommandScopeProvider)
  // so we have access to the scope object for registry
  return (
    <FocusScopeContext.Provider value={moniker}>
      <CommandScopeContext.Provider value={scope}>
        <FocusScopeInner
          moniker={moniker}
          isDirectFocus={isDirectFocus}
          onClick={handleClick}
          {...rest}
        >
          {children}
        </FocusScopeInner>
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
  onClick: React.MouseEventHandler<HTMLElement>;
  children: ReactNode;
}

/** Inner component rendered inside CommandScopeContext so useContextMenu sees the scope. */
function FocusScopeInner({
  moniker,
  isDirectFocus,
  onClick,
  children,
  ...htmlProps
}: FocusScopeInnerProps) {
  const scope = useContext(CommandScopeContext);

  // Build scope chain (moniker walk from this scope to root) for context menu
  const scopeChain = useMemo(() => {
    const chain: string[] = [];
    let current: typeof scope | null = scope;
    while (current) {
      if (current.moniker) chain.push(current.moniker);
      current = current.parent;
    }
    return chain;
  }, [scope]);

  const contextMenuHandler = useContextMenu(scopeChain);
  const { setFocus } = useEntityFocus();

  const handleContextMenu = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      e.stopPropagation();
      setFocus(moniker);
      contextMenuHandler(e);
    },
    [moniker, setFocus, contextMenuHandler],
  );

  const handleDoubleClick = useCallback(
    (e: React.MouseEvent) => {
      // Skip if target is an interactive element
      const target = e.target as HTMLElement;
      const tag = target.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
      if (target.closest("[contenteditable]")) return;

      e.stopPropagation();

      // ui.inspect opens the inspector panel. Double-click dispatches it
      // to the backend which pushes onto the inspector stack.
      const cmd = resolveCommand(scope, "ui.inspect");
      if (cmd) {
        dispatchCommand(cmd);
      }
    },
    [scope],
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
