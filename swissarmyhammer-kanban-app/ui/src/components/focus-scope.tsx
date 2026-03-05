import { useCallback, useContext, useEffect, useMemo, type ReactNode } from "react";
import { CommandScopeContext, type CommandDef, type CommandScope } from "@/lib/command-scope";
import { useEntityFocus, useIsFocused } from "@/lib/entity-focus-context";
import { useContextMenu } from "@/lib/context-menu";

interface FocusScopeProps {
  /** The moniker ("type:id") for the entity this scope represents. */
  moniker: string;
  /** Commands to register in this scope. */
  commands: CommandDef[];
  children: ReactNode;
  /** Additional CSS class names. */
  className?: string;
  /** Additional inline styles. */
  style?: React.CSSProperties;
}

/**
 * Combines CommandScopeProvider + entity focus + context menu into one wrapper.
 *
 * - Wraps children in a CommandScopeProvider with the given commands
 * - Sets entity focus on click (but not when clicking inputs/textareas)
 * - Shows native context menu on right-click using commands from the scope chain
 * - Adds data-moniker and data-focused attributes for CSS targeting
 * - Registers/deregisters the scope in the EntityFocus scope registry
 */
export function FocusScope({ moniker, commands, children, className, style }: FocusScopeProps) {
  const { focusedMoniker, setFocus, registerScope, unregisterScope } = useEntityFocus();
  const isFocused = useIsFocused(moniker);
  const isDirectFocus = focusedMoniker === moniker;

  // Build the scope ourselves so we can register it
  const parent = useContext(CommandScopeContext);
  const scope = useMemo<CommandScope>(() => {
    const map = new Map<string, CommandDef>();
    for (const cmd of commands) {
      map.set(cmd.id, cmd);
    }
    return { commands: map, parent, moniker };
  }, [commands, parent, moniker]);

  // Register/deregister scope in the EntityFocus registry
  useEffect(() => {
    registerScope(moniker, scope);
    return () => unregisterScope(moniker);
  }, [moniker, scope, registerScope, unregisterScope]);

  const handleClick = useCallback(
    (e: React.MouseEvent) => {
      // Don't change entity focus when clicking inputs/textareas/selects
      const target = e.target as HTMLElement;
      const tag = target.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
      // Don't change if the target is inside a contenteditable
      if (target.closest("[contenteditable]")) return;

      e.stopPropagation();
      setFocus(moniker);
    },
    [moniker, setFocus],
  );

  // Provide the scope via CommandScopeContext directly (not CommandScopeProvider)
  // so we have access to the scope object for registry
  return (
    <CommandScopeContext.Provider value={scope}>
      <FocusScopeInner
        moniker={moniker}
        isFocused={isFocused}
        isDirectFocus={isDirectFocus}
        onClick={handleClick}
        className={className}
        style={style}
      >
        {children}
      </FocusScopeInner>
    </CommandScopeContext.Provider>
  );
}

/** Inner component rendered inside CommandScopeContext so useContextMenu sees the scope. */
function FocusScopeInner({
  moniker,
  isFocused,
  isDirectFocus,
  onClick,
  children,
  className,
  style,
}: {
  moniker: string;
  isFocused: boolean;
  isDirectFocus: boolean;
  onClick: (e: React.MouseEvent) => void;
  children: ReactNode;
  className?: string;
  style?: React.CSSProperties;
}) {
  const contextMenuHandler = useContextMenu();
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

  return (
    <div
      data-moniker={moniker}
      data-focused={isDirectFocus || undefined}
      onClick={onClick}
      onContextMenu={handleContextMenu}
      className={className}
      style={style}
    >
      {children}
    </div>
  );
}
