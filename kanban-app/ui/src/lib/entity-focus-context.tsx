import {
  createContext,
  useContext,
  useState,
  useCallback,
  useEffect,
  useRef,
  useMemo,
  type ReactNode,
} from "react";
import type { CommandScope } from "./command-scope";
import { backendDispatch } from "./command-scope";

/** A predicate that a FocusScope uses to claim focus when a nav command fires. */
export interface ClaimPredicate {
  /** The command ID to match (e.g. "nav.right"). */
  command: string;
  /**
   * Returns true if this scope should claim focus.
   * @param focusedMoniker - The currently focused moniker
   * @param isDescendantOf - Returns true if the focused element is a descendant
   *   of the given ancestor moniker (walks the scope chain). Use this when a
   *   field should respond to nav commands even when a child (e.g. a pill) is focused.
   */
  when: (
    focusedMoniker: string | null,
    isDescendantOf: (ancestor: string) => boolean,
  ) => boolean;
}

interface EntityFocusContextValue {
  /** The moniker ("type:id") of the currently focused entity, or null. */
  focusedMoniker: string | null;
  /** Set the focused entity. Pass null to clear focus. */
  setFocus: (moniker: string | null) => void;
  /** Register a scope for a given moniker. Does not trigger re-renders. */
  registerScope: (moniker: string, scope: CommandScope) => void;
  /** Unregister a scope by moniker. Does not trigger re-renders. */
  unregisterScope: (moniker: string) => void;
  /** Look up a registered scope by moniker. */
  getScope: (moniker: string) => CommandScope | null;
  /** Register claim predicates for a FocusScope moniker. */
  registerClaimPredicates: (
    moniker: string,
    predicates: ClaimPredicate[],
  ) => void;
  /** Unregister claim predicates for a FocusScope moniker. */
  unregisterClaimPredicates: (moniker: string) => void;
  /**
   * Broadcast a navigation command to all registered claim predicates.
   * Evaluates each predicate with the current focusedMoniker.
   * First match wins -- calls setFocus(claimantMoniker) and stops.
   * Returns true if a claim was made, false otherwise.
   */
  broadcastNavCommand: (commandId: string) => boolean;
}

const EntityFocusContext = createContext<EntityFocusContextValue | null>(null);

/**
 * Build a scope chain from the registry and send the focus change to Rust.
 * Defined outside the component body so it never closes over stale state.
 */
function invokeFocusChange(
  mk: string | null,
  registry: React.MutableRefObject<Map<string, CommandScope>>,
) {
  console.warn(`[FocusScope] focus → ${mk ?? "(none)"}`);
  if (mk) {
    const scope = registry.current.get(mk);
    const chain: string[] = [mk];
    let current = scope?.parent ?? null;
    while (current !== null) {
      if (current.moniker) {
        chain.push(current.moniker);
      }
      current = current.parent;
    }
    backendDispatch({
      cmd: "ui.setFocus",
      args: { scope_chain: chain },
    }).catch((error) => console.error("ui.setFocus failed:", error));
  } else {
    backendDispatch({
      cmd: "ui.setFocus",
      args: { scope_chain: [] },
    }).catch((error) => console.error("ui.setFocus failed:", error));
  }
}

/**
 * Provides entity focus state and a scope registry to the component tree.
 * Should be provided once at the App level.
 */
export function EntityFocusProvider({ children }: { children: ReactNode }) {
  const [focusedMoniker, setFocusedMoniker] = useState<string | null>(null);

  // Ref that shadows focusedMoniker state so callbacks can read the current
  // value without depending on render-time state.
  const focusedMonikerRef = useRef<string | null>(null);

  // Scope registry: ref so registrations don't cause re-renders
  const registryRef = useRef<Map<string, CommandScope>>(new Map());

  const setFocus = useCallback((moniker: string | null) => {
    focusedMonikerRef.current = moniker;
    setFocusedMoniker(moniker);
    invokeFocusChange(moniker, registryRef);
  }, []);

  const registerScope = useCallback((moniker: string, scope: CommandScope) => {
    registryRef.current.set(moniker, scope);
  }, []);

  const unregisterScope = useCallback((moniker: string) => {
    registryRef.current.delete(moniker);
  }, []);

  const getScope = useCallback((moniker: string): CommandScope | null => {
    return registryRef.current.get(moniker) ?? null;
  }, []);

  // --- Claim predicate registry (ref-based, no re-renders) ---
  const claimPredicatesRef = useRef<Map<string, ClaimPredicate[]>>(new Map());

  const registerClaimPredicates = useCallback(
    (moniker: string, predicates: ClaimPredicate[]) => {
      claimPredicatesRef.current.set(moniker, predicates);
    },
    [],
  );

  const unregisterClaimPredicates = useCallback((moniker: string) => {
    claimPredicatesRef.current.delete(moniker);
  }, []);

  /**
   * Broadcast a navigation command to all registered claim predicates.
   *
   * Evaluates each predicate with the current focusedMoniker (read from a ref
   * so it's never stale). First matching predicate claims focus via setFocus
   * and evaluation stops (short-circuit).
   *
   * Evaluation order follows Map insertion order (ES6 spec), which corresponds
   * to component mount order (React depth-first). Children register before
   * parents, so more-specific scopes (pills) are checked before less-specific
   * ones (field rows).
   *
   * @returns true if a predicate claimed focus, false if none matched.
   */
  const broadcastNavCommand = useCallback(
    (commandId: string): boolean => {
      const currentFocus = focusedMonikerRef.current;

      // Build isDescendantOf helper — walks the focused scope's parent chain
      const isDescendantOf = (ancestor: string): boolean => {
        if (!currentFocus) return false;
        const scope = registryRef.current.get(currentFocus);
        if (!scope) return false;
        let current = scope.parent;
        while (current !== null) {
          if (current.moniker === ancestor) return true;
          current = current.parent;
        }
        return false;
      };

      for (const [moniker, predicates] of claimPredicatesRef.current) {
        for (const pred of predicates) {
          if (
            pred.command === commandId &&
            pred.when(currentFocus, isDescendantOf)
          ) {
            setFocus(moniker);
            return true;
          }
        }
      }
      return false;
    },
    [setFocus],
  );

  // Re-dispatch the current scope chain when the OS window gains focus.
  // This ensures the backend always has the scope chain of the active window
  // after Alt-Tab or clicking between windows, not just the last-clicked one.
  useEffect(() => {
    const handleWindowFocus = () => {
      invokeFocusChange(focusedMonikerRef.current, registryRef);
    };
    window.addEventListener("focus", handleWindowFocus);
    return () => window.removeEventListener("focus", handleWindowFocus);
  }, []);

  const value = useMemo<EntityFocusContextValue>(
    () => ({
      focusedMoniker,
      setFocus,
      registerScope,
      unregisterScope,
      getScope,
      registerClaimPredicates,
      unregisterClaimPredicates,
      broadcastNavCommand,
    }),
    [
      focusedMoniker,
      setFocus,
      registerScope,
      unregisterScope,
      getScope,
      registerClaimPredicates,
      unregisterClaimPredicates,
      broadcastNavCommand,
    ],
  );

  return (
    <EntityFocusContext.Provider value={value}>
      {children}
    </EntityFocusContext.Provider>
  );
}

/**
 * Returns the entity focus state and setter.
 * Must be used within an EntityFocusProvider.
 */
export function useEntityFocus(): EntityFocusContextValue {
  const ctx = useContext(EntityFocusContext);
  if (!ctx)
    throw new Error(
      "useEntityFocus must be used within an EntityFocusProvider",
    );
  return ctx;
}

/**
 * Returns the CommandScope of the currently focused entity, or null.
 *
 * Uses the scope registry (a ref) to look up the focused moniker.
 * Note: the registry is stored in a ref for performance (scope
 * registrations don't trigger re-renders). This means the returned
 * value may be stale if a FocusScope registers in the same render
 * cycle as a focus change. In practice this is safe because setFocus
 * is called from click handlers (after mount) while scopes register
 * during mount effects.
 */
export function useFocusedScope(): CommandScope | null {
  const { focusedMoniker, getScope } = useEntityFocus();
  if (focusedMoniker === null) return null;
  return getScope(focusedMoniker);
}

/**
 * Returns true if the given moniker is the focused moniker or an ancestor
 * of the focused scope in the scope chain.
 *
 * Walk: look up focusedMoniker in registry, get scope, walk .parent chain
 * checking scope.moniker === moniker.
 *
 * @param moniker - The moniker to test.
 * @returns true if directly focused or an ancestor of the focused scope.
 */
export function useIsFocused(moniker: string): boolean {
  const { focusedMoniker, getScope } = useEntityFocus();
  if (focusedMoniker === null) return false;
  if (focusedMoniker === moniker) return true;

  // Walk the ancestor chain of the focused scope
  const scope = getScope(focusedMoniker);
  if (!scope) return false;

  let current = scope.parent;
  while (current !== null) {
    if (current.moniker === moniker) return true;
    current = current.parent;
  }
  return false;
}

/**
 * Saves the currently focused moniker on mount and restores it on unmount,
 * but only if the saved moniker still has a registered scope. If the
 * previously focused entity was deleted while this component was mounted,
 * focus is cleared to null instead of restoring a stale moniker.
 *
 * Use this in inspector panels that temporarily steal focus from the board.
 */
export function useRestoreFocus(): void {
  const { focusedMoniker, setFocus, getScope } = useEntityFocus();

  // Capture the focused moniker at mount time only.
  const prevFocusRef = useRef<string | null>(focusedMoniker);
  const mountedRef = useRef(false);
  if (!mountedRef.current) {
    prevFocusRef.current = focusedMoniker;
    mountedRef.current = true;
  }

  // On unmount, restore focus — but only if the saved moniker still exists
  // in the scope registry. If it was removed (e.g. entity deleted), clear
  // focus to null to avoid pointing at a nonexistent entity.
  useEffect(() => {
    return () => {
      const saved = prevFocusRef.current;
      if (saved === null) {
        setFocus(null);
      } else if (getScope(saved) !== null) {
        setFocus(saved);
      } else {
        setFocus(null);
      }
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- cleanup-only effect, must not re-run
  }, []);
}
