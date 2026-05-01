/**
 * Entity-focus provider — moniker-keyed scope registry, kernel-projected
 * focus state, and the `setFocus` action that routes focus mutations to
 * the spatial-nav kernel.
 *
 * # Path-monikers identity model
 *
 * Card `01KQD6064G1C1RAXDFPJVT1F46` collapsed the legacy UUID-based
 * `SpatialKey` and the flat string `Moniker` into a single fully-qualified
 * path: `FullyQualifiedMoniker`. The store's "focused moniker" is now the
 * FQM. The bridge subscribes to the kernel's `focus-changed` events and
 * writes `payload.next_fq` directly into the store.
 *
 * `setFocus` takes a `FullyQualifiedMoniker | null` strictly — passing a
 * `SegmentMoniker` is a TypeScript compile error. The bridge is the only
 * upstream of `store.set` in production; the action dispatches
 * `spatial_focus(fq)` (or `spatial_clear_focus` for `null`) and waits for
 * the kernel emit to flow back.
 *
 * The scope registry is still keyed by string (the entity-scope chain
 * uses `parent.moniker` walks), but the value used at every callsite is
 * the FQM string — `<FocusZone moniker="card:T1">` mounted under
 * `/window/board/column:todo` registers the FQM
 * `/window/board/column:todo/card:T1` in both the kernel and the
 * entity-focus registry.
 */

import {
  createContext,
  useContext,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useSyncExternalStore,
  type ReactNode,
  type MutableRefObject,
} from "react";
import type { CommandScope, DispatchOptions } from "./command-scope";
import { useDispatchCommand, FocusedScopeContext } from "./command-scope";
import {
  useOptionalSpatialFocusActions,
  type SpatialFocusActions,
} from "./spatial-focus-context";
import {
  composeFq,
  fqLastSegment,
  type FullyQualifiedMoniker,
  type SegmentMoniker,
} from "@/types/spatial";
import { useOptionalFullyQualifiedMoniker } from "@/components/fully-qualified-moniker-context";

/** Pre-bound dispatch callable for a specific command — the shape returned by `useDispatchCommand(presetCmd)`. */
type PreboundDispatch = (opts?: DispatchOptions) => Promise<unknown>;

// ---------------------------------------------------------------------------
// Focus store: FQM-keyed subscriptions
// ---------------------------------------------------------------------------

type FocusSubscriber = () => void;

/**
 * Manages focus subscriptions keyed by moniker (FQM in production,
 * segment-form in test fallbacks).
 *
 * Direct structural port of `FieldSubscriptions` from
 * `entity-store-context.tsx`: a `Map<key, Set<cb>>` plus broad
 * `anyListeners`. When focus moves from A to B, only the subscribers
 * for A, the subscribers for B, and the broad listeners fire — every
 * other moniker's slot is untouched. This is what takes per-arrow-key
 * re-renders in a 12k-cell grid from 12k down to exactly 2 (losing cell
 * + gaining cell) plus a handful of broad listeners.
 *
 * The store is allocated once per `EntityFocusProvider` via `useRef`
 * and lives for the provider's lifetime. Its mutations must go through
 * `set()` so notifications stay consistent with the stored snapshot.
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
   * Subscribe to every focus change. Use only for consumers that truly
   * need to observe every move.
   *
   * Defined as an arrow-function instance property so call sites can
   * pass `store.subscribeAll` directly to `useSyncExternalStore` without
   * `.bind(store)`.
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
   * previous moniker's slot (lost focus), the next moniker's slot
   * (gained focus), and every broad listener.
   *
   * **Architectural note** (card `01KQD0WK54G0FRD7SZVZASA9ST`): in
   * production this method is called only from the spatial-focus →
   * entity-focus bridge inside `EntityFocusProvider`. The kernel's
   * `focus-changed` event is the sole upstream of focus state.
   * `FocusActions.setFocus(fq)` does NOT call this method directly; it
   * dispatches a kernel command and waits for the kernel's
   * `focus-changed` event to flow back through the bridge.
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
 * This value is created once per provider mount and never changes
 * identity, so components that only need actions never re-render on
 * focus moves.
 */
export interface FocusActions {
  /**
   * Set the focused entity by FQM. Pass `null` to clear focus.
   *
   * Routes to the spatial-nav kernel via `spatial_focus(fq)` (or
   * `spatial_clear_focus` for `null`); the store update flows back
   * through the bridge after the kernel emits `focus-changed`.
   */
  setFocus: (fq: FullyQualifiedMoniker | null) => void;
  /** Register a scope for a given moniker key. Does not trigger re-renders. */
  registerScope: (moniker: string, scope: CommandScope) => void;
  /** Unregister a scope by moniker key. Does not trigger re-renders. */
  unregisterScope: (moniker: string) => void;
  /** Look up a registered scope by moniker key. */
  getScope: (moniker: string) => CommandScope | null;
  /**
   * Broadcast a navigation command. Retained as a stable callback in
   * the actions bag so existing call sites (board-view, grid-view,
   * app-shell) compile without churn while the spatial-nav migration
   * completes — but the function is a no-op that always returns `false`.
   * Real navigation lives in the Rust spatial-nav kernel.
   */
  broadcastNavCommand: (commandId: string) => boolean;
}

/** Combined legacy shape returned by `useEntityFocus` (the deprecated shim). */
interface EntityFocusContextValue extends FocusActions {
  /** The FQM of the currently focused entity, or null. */
  focusedFq: FullyQualifiedMoniker | null;
}

const FocusActionsContext = createContext<FocusActions | null>(null);
const FocusStoreContext = createContext<FocusStore | null>(null);

// ---------------------------------------------------------------------------
// Provider
// ---------------------------------------------------------------------------

/** Build scope chain by walking the registry from a moniker to root.
 *
 * Each entry pushed is the registered scope's segment moniker (e.g.
 * `"task:abc"`), not the registry key (which is the FQM in production).
 * The Rust side parses each entry with `split_once(':')` to extract an
 * entity type, so chain entries must be segment-shaped.
 */
function buildScopeChain(
  mk: string,
  registry: Map<string, CommandScope>,
): string[] {
  const chain: string[] = [];
  let current = registry.get(mk) ?? null;
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
 * - `FocusActionsContext`: stable callbacks. Value is created once and
 *   never changes identity.
 * - `FocusStoreContext`: the store itself. Components subscribe to
 *   specific monikers (via `useIsDirectFocus`) or broadly (via
 *   `useFocusedFq`).
 *
 * The provider keeps `focusedScopeRef.current` in sync with the store
 * via `subscribeAll` — imperatively, without re-rendering.
 */
export function EntityFocusProvider({ children }: { children: ReactNode }) {
  const storeRef = useRef<FocusStore | null>(null);
  if (storeRef.current === null) storeRef.current = new FocusStore();
  const store = storeRef.current;

  const dispatch = useDispatchCommand("ui.setFocus");

  // Scope registry: ref so registrations don't cause re-renders
  const registryRef = useRef<Map<string, CommandScope>>(new Map());

  // Keep the latest dispatch in a ref so the actions bag below can stay
  // identity-stable across re-renders.
  const dispatchRef = useRef(dispatch);
  dispatchRef.current = dispatch;

  // Hold the latest spatial-focus actions in a ref so `setFocus` reads
  // through the kernel pathway in production and falls back to direct
  // store mutation in test harnesses that skip it.
  const spatialFocus = useOptionalSpatialFocusActions();
  const spatialActionsRef = useRef<SpatialFocusActions | null>(spatialFocus);
  spatialActionsRef.current = spatialFocus;

  // Actions bag — created once via a lazy-init ref, identity-stable forever.
  const actionsRef = useRef<FocusActions | null>(null);
  if (actionsRef.current === null) {
    actionsRef.current = buildFocusActions({
      store,
      registryRef,
      dispatchRef,
      spatialActionsRef,
    });
  }
  const actions = actionsRef.current;

  // Re-dispatch the current scope chain when the OS window gains focus.
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

  // Bridge spatial-focus → entity-focus: the kernel is the single
  // source of truth for focus state.
  //
  // The Rust spatial-nav kernel owns the focused-FQM map. Every change
  // — leaf click, arrow-key cascade, drill-out, fallback resolution
  // after unregister, AND `setFocus(fq)` itself — flows through
  // `spatial_focus*` / `spatial_navigate` and emits a `focus-changed`
  // event with the new FQM. The bridge below translates each event
  // into a `store.set(payload.next_fq)` write so the React-side store
  // stays in lockstep with the kernel.
  //
  // **Architectural invariant** (card `01KQD0WK54G0FRD7SZVZASA9ST`):
  // this bridge is the ONLY upstream of `store.set` in production.
  // `FocusActions.setFocus(fq)` no longer mutates the store directly
  // — it dispatches a kernel command and waits for the kernel's emit
  // to flow back through this bridge.
  //
  // Degrades silently when no `<SpatialFocusProvider>` ancestor is
  // mounted (older tests that wrap only in `<EntityFocusProvider>`
  // keep their pre-bridge behavior, where `setFocus`'s test-harness
  // fallback writes the store directly).
  useEffect(() => {
    if (!spatialFocus) return;
    return spatialFocus.subscribeFocusChanged((payload) => {
      // `next_fq` is null when focus clears (window lost focus,
      // focused scope unregistered without a fallback, or
      // `setFocus(null)` dispatched `spatial_clear_focus`).
      store.set(payload.next_fq);
      const chain = payload.next_fq
        ? buildScopeChain(payload.next_fq, registryRef.current)
        : [];
      dispatchRef
        .current({ args: { scope_chain: chain } })
        .catch((error) =>
          console.error("ui.setFocus (kernel bridge) failed:", error),
        );
    });
  }, [spatialFocus, store]);

  // Subscribe the provider itself to the store so FocusedScopeContext
  // updates when focus moves. Uses useSyncExternalStore with the broad
  // (subscribeAll) channel.
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
  dispatchRef: MutableRefObject<PreboundDispatch>;
  spatialActionsRef: MutableRefObject<SpatialFocusActions | null>;
}

/**
 * Build the identity-stable actions bag used by `FocusActionsContext`.
 *
 * Every closure reads from refs (not state), so this object can be
 * created exactly once per provider mount without staleness.
 */
function buildFocusActions(deps: FocusActionsDeps): FocusActions {
  const { store, registryRef, dispatchRef, spatialActionsRef } = deps;

  /**
   * Dispatch a "set focus" request to the Rust kernel — the single
   * source of truth for focus state.
   *
   * For a non-null FQM, invokes `spatial_focus(fq)`: the kernel
   * updates the per-window focus map and emits a `focus-changed`
   * event.
   *
   * For `null`, invokes `spatial_clear_focus`: the kernel removes the
   * window's focus slot and emits a `Some(prev) → None`
   * `focus-changed` event.
   *
   * In both cases the provider's bridge effect subscribes to those
   * events and writes them into the local `FocusStore` via
   * `store.set` — that is the ONLY upstream of the store in
   * production.
   */
  const setFocus = (fq: FullyQualifiedMoniker | null): void => {
    const spatial = spatialActionsRef.current;
    if (spatial) {
      // Production pathway: kernel-keyed write.
      if (fq === null) {
        void spatial.clearFocus().catch((error) => {
          console.error("spatial_clear_focus failed:", error);
        });
        return;
      }
      void spatial.focus(fq).catch((error) => {
        // The kernel rejected the FQM — log so dev mode catches drift
        // between the React mount tree and the kernel registry. The
        // store stays at its previous value because no `focus-changed`
        // event fires.
        console.error("spatial_focus failed:", error);
      });
      return;
    }

    // Test-harness fallback: no kernel available, so write the store
    // directly and dispatch `ui.setFocus` like the pre-refactor flow.
    store.set(fq);
    const chain = fq ? buildScopeChain(fq, registryRef.current) : [];
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

  /**
   * No-op stand-in for the legacy predicate-broadcast entry point.
   * Always returns `false`.
   */
  const broadcastNavCommand = (_commandId: string): boolean => false;

  return {
    setFocus,
    registerScope,
    unregisterScope,
    getScope,
    broadcastNavCommand,
  };
}

// ---------------------------------------------------------------------------
// Hooks: the narrow surface consumers should prefer
// ---------------------------------------------------------------------------

/**
 * Returns the focus action bag. Value reference is strictly stable
 * across focus moves.
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
 * Returns the focus action bag, or `null` when no `EntityFocusProvider`
 * ancestor is mounted.
 */
export function useOptionalFocusActions(): FocusActions | null {
  return useContext(FocusActionsContext);
}

/**
 * Registers an entity-focus scope for the given moniker key.
 *
 * The registration runs both inline during render *and* inside a
 * cleanup-only `useEffect`. See the architectural rationale for
 * the dual registration in the entity-focus refactor card.
 */
export function useEntityScopeRegistration(
  moniker: string,
  scope: CommandScope,
): void {
  const focusActions = useOptionalFocusActions();
  const scopeRef = useRef(scope);
  scopeRef.current = scope;
  if (focusActions) {
    focusActions.registerScope(moniker, scope);
  }
  useEffect(() => {
    if (!focusActions) return;
    const { registerScope, unregisterScope } = focusActions;
    registerScope(moniker, scopeRef.current);
    return () => unregisterScope(moniker);
  }, [moniker, focusActions]);
}

/**
 * Returns the underlying `FocusStore` handle.
 *
 * Useful when a consumer needs to read the current focus inside an
 * event handler or effect without subscribing to every change.
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
 * Re-renders its caller **only** when `moniker`'s focus slot flips —
 * i.e. when the given moniker (FQM in production) gains or loses
 * direct focus.
 */
export function useIsDirectFocus(moniker: string): boolean {
  const store = useFocusStore();

  const subscribe = useCallback(
    (cb: () => void) => store.subscribe(moniker, cb),
    [store, moniker],
  );

  const current = useSyncExternalStore(subscribe, store.getSnapshot);
  return current === moniker;
}

/**
 * Selective focus subscription that tolerates a missing
 * `EntityFocusProvider`.
 *
 * Behaves identically to `useIsDirectFocus` when an
 * `EntityFocusProvider` ancestor is mounted; returns `false`
 * permanently otherwise.
 */
export function useOptionalIsDirectFocus(moniker: string): boolean {
  const store = useContext(FocusStoreContext);

  const noop = useMemo(
    () => ({
      subscribe: (_cb: () => void) => () => {},
      getSnapshot: () => null as string | null,
    }),
    [],
  );

  const subscribe = useCallback(
    (cb: () => void) =>
      store ? store.subscribe(moniker, cb) : noop.subscribe(cb),
    [store, moniker, noop],
  );

  const getSnapshot = useCallback(
    () => (store ? store.getSnapshot() : noop.getSnapshot()),
    [store, noop],
  );

  const current = useSyncExternalStore(subscribe, getSnapshot);
  return current === moniker;
}

/**
 * Broad focus subscription — re-renders on every focus change.
 *
 * Returns the focused FQM (or `null`) under the path-monikers refactor.
 * Use only for consumers that genuinely need the current FQM on every
 * move (grid cursor derivation, board-view bookkeeping). For everything
 * else, prefer `useIsDirectFocus` or `useFocusedMonikerRef`.
 */
export function useFocusedFq(): FullyQualifiedMoniker | null {
  const store = useFocusStore();
  const value = useSyncExternalStore(store.subscribeAll, store.getSnapshot);
  return value as FullyQualifiedMoniker | null;
}

/**
 * Derived view of the focused FQM's trailing segment.
 *
 * Re-renders on every focus change (it reads through the broad
 * subscription). Returns `null` when no entity is focused.
 *
 * Provided for legacy display callers that want the leaf segment
 * (`field:T1.title`) rather than the full path
 * (`/window/inspector/field:T1.title`). Most call sites should prefer
 * the FQM directly via `useFocusedFq()`.
 */
export function useFocusedSegmentMoniker(): SegmentMoniker | null {
  const fq = useFocusedFq();
  if (fq === null) return null;
  return fqLastSegment(fq);
}

/**
 * Ref-style broad subscription.
 *
 * Returns a ref whose `.current` mirrors the focused FQM. Reads through
 * the ref do **not** trigger re-renders.
 */
export function useFocusedMonikerRef(): MutableRefObject<FullyQualifiedMoniker | null> {
  const store = useFocusStore();
  const ref = useRef<FullyQualifiedMoniker | null>(
    store.getSnapshot() as FullyQualifiedMoniker | null,
  );

  useEffect(() => {
    ref.current = store.getSnapshot() as FullyQualifiedMoniker | null;
    return store.subscribeAll(() => {
      ref.current = store.getSnapshot() as FullyQualifiedMoniker | null;
    });
  }, [store]);

  return ref;
}

/**
 * Returns the CommandScope of the currently focused entity, or null.
 */
export function useFocusedScope(): CommandScope | null {
  const focusedFq = useFocusedFq();
  const { getScope } = useFocusActions();
  if (focusedFq === null) return null;
  return getScope(focusedFq);
}

/**
 * Returns true if the given moniker key is the focused moniker or an
 * ancestor of the focused scope in the scope chain.
 *
 * Walk: look up `focusedFq` in the registry, get its scope, walk the
 * `.parent` chain checking `scope.moniker === moniker`.
 */
export function useIsFocused(moniker: string): boolean {
  const focusedFq = useFocusedFq();
  const { getScope } = useFocusActions();
  if (focusedFq === null) return false;
  if (focusedFq === moniker) return true;

  // Walk the focused scope chain (leaf included). The leaf's `moniker`
  // is its segment (e.g. `field:task:abc.title`); the registry key is the
  // FQM. Callers pass a segment, so the leaf segment must be checked too.
  const scope = getScope(focusedFq);
  if (!scope) return false;

  let current: CommandScope | null = scope;
  while (current !== null) {
    if (current.moniker === moniker) return true;
    current = current.parent;
  }
  return false;
}

// ---------------------------------------------------------------------------
// Compat shim — deprecated
// ---------------------------------------------------------------------------

/**
 * Returns the entity focus state and setter in a single bag.
 *
 * @deprecated Prefer the narrow hooks (`useFocusActions`,
 * `useIsDirectFocus`, `useFocusedFq`, `useFocusedMonikerRef`) — this
 * shim re-renders on every focus move because it reads the broad FQM,
 * which defeats the whole point of the moniker-keyed store.
 */
export function useEntityFocus(): EntityFocusContextValue {
  const actions = useContext(FocusActionsContext);
  const store = useContext(FocusStoreContext);
  if (!actions || !store)
    throw new Error(
      "useEntityFocus must be used within an EntityFocusProvider",
    );
  const focusedFq = useSyncExternalStore(store.subscribeAll, store.getSnapshot);
  return { ...actions, focusedFq: focusedFq as FullyQualifiedMoniker | null };
}

/**
 * Legacy alias for `useFocusedFq` — returns the broad-subscribed
 * focused moniker as a plain string. Provided so existing call sites
 * that read `useFocusedMoniker()` (which used to return a string)
 * continue to compile during the migration.
 *
 * @deprecated Prefer `useFocusedFq()` (returns `FullyQualifiedMoniker`)
 * or `useFocusedSegmentMoniker()` (returns the trailing segment).
 */
export function useFocusedMoniker(): FullyQualifiedMoniker | null {
  return useFocusedFq();
}

/**
 * Build a `setFocus`-compatible callback that composes a tail of
 * `SegmentMoniker` segments under the enclosing spatial primitive's
 * FQM, then dispatches the focus mutation through the kernel.
 *
 * Use when the caller knows the relative path of the target relative
 * to the current primitive's FQM. Single-segment callers (immediate
 * child) pass one segment; multi-segment callers (e.g. board-zone
 * targeting a not-yet-mounted card under a known column) pass the
 * full chain.
 *
 * Falls back to a `console.error` no-op when called outside any
 * spatial primitive — production trees always wrap everything in a
 * window-root layer, so the no-context branch is reachable only in
 * pre-spatial-nav unit tests.
 */
export function useFocusBySegmentPath(): (
  ...segments: SegmentMoniker[]
) => void {
  const parent = useOptionalFullyQualifiedMoniker();
  const { setFocus } = useFocusActions();
  return useCallback(
    (...segments: SegmentMoniker[]) => {
      if (parent === null) {
        console.error(
          "useFocusBySegmentPath called outside any spatial primitive",
        );
        return;
      }
      let fq = parent;
      for (const seg of segments) {
        fq = composeFq(fq, seg);
      }
      setFocus(fq);
    },
    [parent, setFocus],
  );
}
