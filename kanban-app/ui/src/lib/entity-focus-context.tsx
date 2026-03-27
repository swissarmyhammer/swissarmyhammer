import {
  createContext,
  useContext,
  useState,
  useCallback,
  useRef,
  useMemo,
  type ReactNode,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import type { CommandScope } from "./command-scope";

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
  /**
   * Push a new programmatic focus claim onto the LIFO stack.
   * The most recently pushed (highest ID) claim wins entity focus.
   * Returns a numeric claim ID used for updateClaim/popClaim.
   */
  pushClaim: (moniker: string, scope: CommandScope) => number;
  /**
   * Update an existing claim's moniker and scope.
   * Only changes entity focus if this claim is the active (topmost) one.
   */
  updateClaim: (id: number, moniker: string, scope: CommandScope) => void;
  /**
   * Remove a claim from the stack. If it was the active claim, focus
   * falls back to the next claim (or null if the stack is empty).
   */
  popClaim: (id: number) => void;
  /** Register claim predicates for a FocusScope moniker. */
  registerClaimPredicates: (moniker: string, predicates: ClaimPredicate[]) => void;
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
    invoke("dispatch_command", {
      cmd: "ui.setFocus",
      args: { scope_chain: chain },
    }).catch((error) => console.error("ui.setFocus failed:", error));
  } else {
    invoke("dispatch_command", {
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

  // --- Claim stack (LIFO) for programmatic focus ---
  const nextClaimIdRef = useRef(0);
  /** Map from claim ID to { moniker, scope }. */
  const claimsRef = useRef<Map<number, { moniker: string; scope: CommandScope }>>(new Map());

  /** Find the active (highest ID) claim, or null if the stack is empty. */
  function getActiveClaim(): { id: number; moniker: string; scope: CommandScope } | null {
    let maxId = -1;
    let result: { id: number; moniker: string; scope: CommandScope } | null = null;
    for (const [id, entry] of claimsRef.current) {
      if (id > maxId) { maxId = id; result = { id, ...entry }; }
    }
    return result;
  }

  const pushClaim = useCallback((moniker: string, scope: CommandScope): number => {
    const id = nextClaimIdRef.current++;
    claimsRef.current.set(id, { moniker, scope });
    registryRef.current.set(moniker, scope);
    // This is now the active claim (highest ID)
    focusedMonikerRef.current = moniker;
    setFocusedMoniker(moniker);
    invokeFocusChange(moniker, registryRef);
    return id;
  }, []);

  const updateClaim = useCallback((id: number, moniker: string, scope: CommandScope) => {
    const claims = claimsRef.current;
    const prev = claims.get(id);
    if (!prev) return;

    // Unregister old moniker from scope registry if it changed and no other claim uses it
    if (prev.moniker !== moniker) {
      let oldMonikerStillClaimed = false;
      for (const [otherId, other] of claims) {
        if (otherId !== id && other.moniker === prev.moniker) { oldMonikerStillClaimed = true; break; }
      }
      if (!oldMonikerStillClaimed) {
        registryRef.current.delete(prev.moniker);
      }
    }

    const monikerChanged = prev.moniker !== moniker;

    // Update the claim and register the new scope
    claims.set(id, { moniker, scope });
    registryRef.current.set(moniker, scope);

    // Only change focus if the moniker actually changed AND this is the active claim.
    // Scope-only changes (same moniker, new scope object) update the registry
    // but don't reset focus — that would override the user's navigation position.
    if (monikerChanged) {
      const active = getActiveClaim();
      if (active && active.id === id) {
        focusedMonikerRef.current = moniker;
        setFocusedMoniker(moniker);
        invokeFocusChange(moniker, registryRef);
      }
    }
  }, []);

  const popClaim = useCallback((id: number) => {
    const claims = claimsRef.current;
    const entry = claims.get(id);
    claims.delete(id);

    // Only unregister the scope if no other claim uses the same moniker
    if (entry) {
      let monikerStillClaimed = false;
      for (const [, other] of claims) {
        if (other.moniker === entry.moniker) { monikerStillClaimed = true; break; }
      }
      if (!monikerStillClaimed) {
        registryRef.current.delete(entry.moniker);
      }
    }

    // Only update focus if the active moniker actually changed
    const newMoniker = getActiveClaim()?.moniker ?? null;
    if (newMoniker !== focusedMonikerRef.current) {
      focusedMonikerRef.current = newMoniker;
      setFocusedMoniker(newMoniker);
      invokeFocusChange(newMoniker, registryRef);
    }
  }, []);

  // --- Claim predicate registry (ref-based, no re-renders) ---
  const claimPredicatesRef = useRef<Map<string, ClaimPredicate[]>>(new Map());

  const registerClaimPredicates = useCallback((moniker: string, predicates: ClaimPredicate[]) => {
    claimPredicatesRef.current.set(moniker, predicates);
  }, []);

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
  const broadcastNavCommand = useCallback((commandId: string): boolean => {
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
        if (pred.command === commandId && pred.when(currentFocus, isDescendantOf)) {
          setFocus(moniker);
          return true;
        }
      }
    }
    return false;
  }, [setFocus]);

  const value = useMemo<EntityFocusContextValue>(
    () => ({
      focusedMoniker,
      setFocus,
      registerScope,
      unregisterScope,
      getScope,
      pushClaim,
      updateClaim,
      popClaim,
      registerClaimPredicates,
      unregisterClaimPredicates,
      broadcastNavCommand,
    }),
    [focusedMoniker, setFocus, registerScope, unregisterScope, getScope, pushClaim, updateClaim, popClaim, registerClaimPredicates, unregisterClaimPredicates, broadcastNavCommand],
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
