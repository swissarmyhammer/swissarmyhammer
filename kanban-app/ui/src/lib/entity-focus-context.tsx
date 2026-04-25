import {
  createContext,
  useContext,
  useCallback,
  useEffect,
  useRef,
  useSyncExternalStore,
  type ReactNode,
  type MutableRefObject,
} from "react";
import type { CommandScope, DispatchOptions } from "./command-scope";
import { useDispatchCommand, FocusedScopeContext } from "./command-scope";

/** Pre-bound dispatch callable for a specific command — the shape returned by `useDispatchCommand(presetCmd)`. */
type PreboundDispatch = (opts?: DispatchOptions) => Promise<unknown>;

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

// ---------------------------------------------------------------------------
// Focus store: moniker-keyed subscriptions
// ---------------------------------------------------------------------------

type FocusSubscriber = () => void;

/**
 * Manages focus subscriptions keyed by moniker.
 *
 * Direct structural port of `FieldSubscriptions` from `entity-store-context.tsx`:
 * a `Map<key, Set<cb>>` plus broad `anyListeners`. When focus moves from A to B,
 * only the subscribers for A, the subscribers for B, and the broad listeners
 * fire — every other moniker's slot is untouched. This is what takes per-arrow-
 * key re-renders in a 12k-cell grid from 12k down to exactly 2 (losing cell +
 * gaining cell) plus a handful of broad listeners for grid-nav bookkeeping.
 *
 * The store is allocated once per `EntityFocusProvider` via `useRef` and lives
 * for the provider's lifetime. Its mutations must go through `set()` so that
 * notifications stay consistent with the stored snapshot.
 */
export class FocusStore {
  private current: string | null = null;
  private perMoniker = new Map<string, Set<FocusSubscriber>>();
  private anyListeners = new Set<FocusSubscriber>();

  /** Snapshot getter used by `useSyncExternalStore`. */
  getSnapshot = (): string | null => this.current;

  /**
   * Subscribe to a single moniker's focus slot.
   *
   * Notified only when the given moniker gains or loses focus; unrelated
   * focus moves do not wake this subscriber.
   *
   * @returns an unsubscribe function.
   */
  subscribe(moniker: string, cb: FocusSubscriber): () => void {
    let set = this.perMoniker.get(moniker);
    if (!set) {
      set = new Set();
      this.perMoniker.set(moniker, set);
    }
    set.add(cb);
    return () => {
      set!.delete(cb);
      if (set!.size === 0) this.perMoniker.delete(moniker);
    };
  }

  /**
   * Subscribe to every focus change. Use only for consumers that truly need
   * to observe every move (e.g. grid cursor derivation, command-scope bookkeeping).
   *
   * Defined as an arrow-function instance property (matching `getSnapshot`)
   * so call sites can pass `store.subscribeAll` directly to
   * `useSyncExternalStore` without `.bind(store)` — a bound function would
   * have a fresh identity every render and cause `useSyncExternalStore` to
   * unsubscribe and re-subscribe on every re-render.
   *
   * @returns an unsubscribe function.
   */
  subscribeAll = (cb: FocusSubscriber): (() => void) => {
    this.anyListeners.add(cb);
    return () => {
      this.anyListeners.delete(cb);
    };
  };

  /**
   * Update the focused moniker and notify affected subscribers.
   *
   * No-op when `next` equals the current value. Otherwise notifies the
   * previous moniker's slot (lost focus), the next moniker's slot (gained
   * focus), and every broad listener. Broad-listener notification happens
   * last so its order-of-effect matches the pre-refactor `useState` flow.
   */
  set(next: string | null): void {
    const prev = this.current;
    if (prev === next) return;
    this.current = next;
    if (prev !== null) this.perMoniker.get(prev)?.forEach((cb) => cb());
    if (next !== null) this.perMoniker.get(next)?.forEach((cb) => cb());
    this.anyListeners.forEach((cb) => cb());
  }
}

// ---------------------------------------------------------------------------
// Actions context
// ---------------------------------------------------------------------------

/**
 * The stable callbacks exposed by the focus provider.
 *
 * This value is created once per provider mount and never changes identity,
 * so components that only need actions never re-render on focus moves.
 */
export interface FocusActions {
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

/** Combined legacy shape returned by `useEntityFocus` (the deprecated shim). */
interface EntityFocusContextValue extends FocusActions {
  /** The moniker ("type:id") of the currently focused entity, or null. */
  focusedMoniker: string | null;
}

const FocusActionsContext = createContext<FocusActions | null>(null);
const FocusStoreContext = createContext<FocusStore | null>(null);

// ---------------------------------------------------------------------------
// Provider
// ---------------------------------------------------------------------------

/** Build scope chain by walking the registry from a moniker to root. */
function buildScopeChain(
  mk: string,
  registry: Map<string, CommandScope>,
): string[] {
  const chain: string[] = [mk];
  let current = registry.get(mk)?.parent ?? null;
  while (current !== null) {
    if (current.moniker) chain.push(current.moniker);
    current = current.parent;
  }
  return chain;
}

/**
 * Provides entity focus state and a scope registry to the component tree.
 * Should be provided once at the App level.
 *
 * Internally owns a `FocusStore` and publishes two contexts:
 *
 * - `FocusActionsContext`: stable callbacks. Value is created once and never
 *   changes identity, so components that read only actions never re-render
 *   on focus moves.
 * - `FocusStoreContext`: the store itself. Components subscribe to specific
 *   monikers (via `useIsDirectFocus`) or broadly (via `useFocusedMoniker`).
 *
 * The provider keeps `focusedScopeRef.current` in sync with the store via
 * `subscribeAll` — imperatively, without re-rendering. Consumers of
 * `FocusedScopeRefContext` (notably `useDispatchCommand`) never wake on
 * focus moves because the context VALUE (the ref object) is identity-stable
 * for the provider's lifetime. On a 12k-cell grid this is what takes
 * per-nav re-renders from ~12k down to exactly two (the losing + gaining
 * `useIsDirectFocus` subscribers).
 */
export function EntityFocusProvider({ children }: { children: ReactNode }) {
  const storeRef = useRef<FocusStore | null>(null);
  if (storeRef.current === null) storeRef.current = new FocusStore();
  const store = storeRef.current;

  const dispatch = useDispatchCommand("ui.setFocus");

  // Scope registry: ref so registrations don't cause re-renders
  const registryRef = useRef<Map<string, CommandScope>>(new Map());

  // Claim predicate registry: ref so registrations don't cause re-renders
  const claimPredicatesRef = useRef<Map<string, ClaimPredicate[]>>(new Map());

  // Keep the latest dispatch in a ref so the actions bag below can stay
  // identity-stable across re-renders (matches the pre-refactor behavior
  // where setFocus was captured via useCallback with [dispatch]).
  const dispatchRef = useRef(dispatch);
  dispatchRef.current = dispatch;

  // Actions bag — created once via a lazy-init ref, identity-stable forever.
  // Every callback reads from refs so they don't need to be recreated.
  const actionsRef = useRef<FocusActions | null>(null);
  if (actionsRef.current === null) {
    actionsRef.current = buildFocusActions({
      store,
      registryRef,
      claimPredicatesRef,
      dispatchRef,
    });
  }
  const actions = actionsRef.current;

  // Re-dispatch the current scope chain when the OS window gains focus.
  // This ensures the backend always has the scope chain of the active window
  // after Alt-Tab or clicking between windows, not just the last-clicked one.
  useEffect(() => {
    const handleWindowFocus = () => {
      const moniker = store.getSnapshot();
      const chain = moniker
        ? buildScopeChain(moniker, registryRef.current)
        : [];
      dispatchRef
        .current({ args: { scope_chain: chain } })
        .catch((error) =>
          console.error("ui.setFocus (window focus) failed:", error),
        );
    };
    window.addEventListener("focus", handleWindowFocus);
    return () => window.removeEventListener("focus", handleWindowFocus);
  }, [store]);

  // Stable ref tracking the focused CommandScope. Published via
  // `FocusedScopeRefContext` so `useDispatchCommand` can read it at dispatch
  // time WITHOUT subscribing its host component to focus moves. The ref
  // object itself never changes identity — the context value is stable for
  // the provider's lifetime, so no `useDispatchCommand` call site wakes on
  // focus change. On a 12k-cell grid, this collapses per-nav re-renders
  // from ~12k (every `FocusScopeInner` re-rendering via the old
  // `useContext(FocusedScopeContext)` subscription) down to exactly two
  // (the losing + gaining `useIsDirectFocus` subscribers) plus the one
  // `subscribeAll` callback in this provider — which is a plain imperative
  // write, not a React re-render.
  // Subscribe the provider itself to the store so FocusedScopeContext
  // updates when focus moves. Uses useSyncExternalStore with the broad
  // (subscribeAll) channel — the provider is a single component, so one
  // re-render per focus change is exactly what we want.
  const focusedMoniker = useSyncExternalStore(
    store.subscribeAll,
    store.getSnapshot,
  );
  const focusedScope = focusedMoniker
    ? (registryRef.current.get(focusedMoniker) ?? null)
    : null;

  return (
    <FocusActionsContext.Provider value={actions}>
      <FocusStoreContext.Provider value={store}>
        <FocusedScopeContext.Provider value={focusedScope}>
          {children}
        </FocusedScopeContext.Provider>
      </FocusStoreContext.Provider>
    </FocusActionsContext.Provider>
  );
}

/** Inputs for `buildFocusActions`. Grouped to keep the helper signature small. */
interface FocusActionsDeps {
  store: FocusStore;
  registryRef: MutableRefObject<Map<string, CommandScope>>;
  claimPredicatesRef: MutableRefObject<Map<string, ClaimPredicate[]>>;
  dispatchRef: MutableRefObject<PreboundDispatch>;
}

/**
 * Build the identity-stable actions bag used by `FocusActionsContext`.
 *
 * Every closure reads from refs (not state), so this object can be created
 * exactly once per provider mount without staleness. Factoring this out of
 * the provider body keeps the component short and mirrors the split between
 * data and logic in `entity-store-context.tsx`.
 */
function buildFocusActions(deps: FocusActionsDeps): FocusActions {
  const { store, registryRef, claimPredicatesRef, dispatchRef } = deps;

  const setFocus = (moniker: string | null): void => {
    store.set(moniker);
    const chain = moniker ? buildScopeChain(moniker, registryRef.current) : [];
    dispatchRef
      .current({ args: { scope_chain: chain } })
      .catch((error) => console.error("ui.setFocus failed:", error));
  };

  const registerScope = (moniker: string, scope: CommandScope): void => {
    registryRef.current.set(moniker, scope);
  };

  const unregisterScope = (moniker: string): void => {
    registryRef.current.delete(moniker);
  };

  const getScope = (moniker: string): CommandScope | null =>
    registryRef.current.get(moniker) ?? null;

  const registerClaimPredicates = (
    moniker: string,
    predicates: ClaimPredicate[],
  ): void => {
    claimPredicatesRef.current.set(moniker, predicates);
  };

  const unregisterClaimPredicates = (moniker: string): void => {
    claimPredicatesRef.current.delete(moniker);
  };

  /**
   * Broadcast a navigation command to all registered claim predicates.
   *
   * Evaluates each predicate with the current focusedMoniker (read from the
   * store so it's never stale). First matching predicate claims focus via
   * setFocus and evaluation stops (short-circuit).
   *
   * Evaluation order follows Map insertion order (ES6 spec), which corresponds
   * to component mount order (React depth-first). Children register before
   * parents, so more-specific scopes (pills) are checked before less-specific
   * ones (field rows).
   */
  const broadcastNavCommand = (commandId: string): boolean => {
    const currentFocus = store.getSnapshot();

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
  };

  return {
    setFocus,
    registerScope,
    unregisterScope,
    getScope,
    registerClaimPredicates,
    unregisterClaimPredicates,
    broadcastNavCommand,
  };
}

// ---------------------------------------------------------------------------
// Hooks: the narrow surface consumers should prefer
// ---------------------------------------------------------------------------

/**
 * Returns the focus action bag. Value reference is strictly stable across
 * focus moves — consumers that only call actions will never re-render when
 * focus changes.
 *
 * Must be used within an `EntityFocusProvider`.
 */
export function useFocusActions(): FocusActions {
  const actions = useContext(FocusActionsContext);
  if (!actions)
    throw new Error(
      "useFocusActions must be used within an EntityFocusProvider",
    );
  return actions;
}

/**
 * Returns the underlying `FocusStore` handle.
 *
 * Useful when a consumer needs to read the current focus inside an event
 * handler or effect without subscribing to every change — reach for
 * `useFocusedMonikerRef` first; use this hook only when you also need
 * `subscribe` / `subscribeAll` directly.
 *
 * Must be used within an `EntityFocusProvider`.
 */
export function useFocusStore(): FocusStore {
  const store = useContext(FocusStoreContext);
  if (!store)
    throw new Error("useFocusStore must be used within an EntityFocusProvider");
  return store;
}

/**
 * Selective focus subscription.
 *
 * Re-renders its caller **only** when `moniker`'s focus slot flips — i.e.
 * when the given moniker gains or loses direct focus. Unrelated focus moves
 * do not wake this subscriber. This is the hot-path hook used by
 * `FocusScope` to drive the focus-bar highlight; in a 12k-cell grid, a
 * single arrow-key press renders exactly two cells (the one that lost
 * focus and the one that gained it).
 *
 * Note: this is a *direct* focus check, not an ancestor-walking one. For
 * the ancestor variant (true when any descendant of `moniker` is focused),
 * use `useIsFocused` instead.
 */
export function useIsDirectFocus(moniker: string): boolean {
  const store = useFocusStore();

  // `useCallback` so `subscribe` has a stable identity per moniker —
  // useSyncExternalStore resubscribes whenever `subscribe` changes.
  const subscribe = useCallback(
    (cb: () => void) => store.subscribe(moniker, cb),
    [store, moniker],
  );

  const current = useSyncExternalStore(subscribe, store.getSnapshot);
  return current === moniker;
}

/**
 * Broad focus subscription — re-renders on every focus change.
 *
 * Use only for consumers that genuinely need the current moniker on every
 * move (grid cursor derivation, board-view bookkeeping). For everything
 * else, prefer `useIsDirectFocus` or `useFocusedMonikerRef`.
 */
export function useFocusedMoniker(): string | null {
  const store = useFocusStore();
  return useSyncExternalStore(store.subscribeAll, store.getSnapshot);
}

/**
 * Ref-style broad subscription.
 *
 * Returns a ref whose `.current` mirrors the focused moniker. Reads through
 * the ref do **not** trigger re-renders — the ref is updated inside a
 * `subscribeAll` callback. Use this for one-shot captures (e.g. saving
 * the previously focused moniker at inspector mount time) or for refs
 * threaded into action-command factories.
 *
 * The returned ref is stable for the component's lifetime.
 */
export function useFocusedMonikerRef(): MutableRefObject<string | null> {
  const store = useFocusStore();
  const ref = useRef<string | null>(store.getSnapshot());

  useEffect(() => {
    // Initialize eagerly so consumers reading the ref on their first
    // render observe the current focus, not the stale construction-time
    // value (the snapshot could have moved between construction and effect).
    ref.current = store.getSnapshot();
    return store.subscribeAll(() => {
      ref.current = store.getSnapshot();
    });
  }, [store]);

  return ref;
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
  const focusedMoniker = useFocusedMoniker();
  const { getScope } = useFocusActions();
  if (focusedMoniker === null) return null;
  return getScope(focusedMoniker);
}

/**
 * Returns true if the given moniker is the focused moniker or an ancestor
 * of the focused scope in the scope chain.
 *
 * Walk: look up focusedMoniker in the registry, get its scope, walk the
 * `.parent` chain checking `scope.moniker === moniker`.
 *
 * This is the *ancestor-aware* variant. For the direct-only, selector-
 * subscribed hot-path hook used by `FocusScope`, see `useIsDirectFocus`.
 *
 * @param moniker - The moniker to test.
 * @returns true if directly focused or an ancestor of the focused scope.
 */
export function useIsFocused(moniker: string): boolean {
  const focusedMoniker = useFocusedMoniker();
  const { getScope } = useFocusActions();
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
  const { setFocus, getScope } = useFocusActions();
  const focusRef = useFocusedMonikerRef();

  // Capture the focused moniker at mount time only.
  const prevFocusRef = useRef<string | null>(null);
  const mountedRef = useRef(false);
  if (!mountedRef.current) {
    prevFocusRef.current = focusRef.current;
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

// ---------------------------------------------------------------------------
// Compat shim — deprecated
// ---------------------------------------------------------------------------

/**
 * Returns the entity focus state and setter in a single bag.
 *
 * @deprecated Prefer the narrow hooks (`useFocusActions`, `useIsDirectFocus`,
 * `useFocusedMoniker`, `useFocusedMonikerRef`) — this shim re-renders on
 * every focus move because it reads the broad moniker, which defeats the
 * whole point of the moniker-keyed store. Retained so existing call sites
 * and the twelve-file test mock surface keep working during migration.
 */
export function useEntityFocus(): EntityFocusContextValue {
  const actions = useContext(FocusActionsContext);
  const store = useContext(FocusStoreContext);
  if (!actions || !store)
    throw new Error(
      "useEntityFocus must be used within an EntityFocusProvider",
    );
  const focusedMoniker = useSyncExternalStore(
    store.subscribeAll,
    store.getSnapshot,
  );
  return { ...actions, focusedMoniker };
}
