import { createContext, useContext, useState, useCallback, useRef, useMemo, type ReactNode } from "react";
import type { CommandScope } from "./command-scope";

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
}

const EntityFocusContext = createContext<EntityFocusContextValue | null>(null);

/**
 * Provides entity focus state and a scope registry to the component tree.
 * Should be provided once at the App level.
 */
export function EntityFocusProvider({ children }: { children: ReactNode }) {
  const [focusedMoniker, setFocusedMoniker] = useState<string | null>(null);

  // Scope registry: ref so registrations don't cause re-renders
  const registryRef = useRef<Map<string, CommandScope>>(new Map());

  const setFocus = useCallback((moniker: string | null) => {
    setFocusedMoniker(moniker);
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

  const value = useMemo<EntityFocusContextValue>(
    () => ({ focusedMoniker, setFocus, registerScope, unregisterScope, getScope }),
    [focusedMoniker, setFocus, registerScope, unregisterScope, getScope],
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
  if (!ctx) throw new Error("useEntityFocus must be used within an EntityFocusProvider");
  return ctx;
}

/**
 * Returns the CommandScope of the currently focused entity, or null.
 *
 * Uses the scope registry to look up the focused moniker.
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
