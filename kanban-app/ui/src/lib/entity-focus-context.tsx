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
import { invoke } from "@tauri-apps/api/core";
import type { CommandScope, DispatchOptions } from "./command-scope";
import { useDispatchCommand, FocusedScopeContext } from "./command-scope";
import {
  useOptionalSpatialFocusActions,
  type SpatialFocusActions,
} from "./spatial-focus-context";

/** Pre-bound dispatch callable for a specific command — the shape returned by `useDispatchCommand(presetCmd)`. */
type PreboundDispatch = (opts?: DispatchOptions) => Promise<unknown>;

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
   *
   * **Architectural note**: in production this method is called only
   * from the spatial-focus → entity-focus bridge inside
   * [`EntityFocusProvider`] — the kernel's `focus-changed` event is the
   * sole upstream of focus state. `FocusActions.setFocus(moniker)` does
   * NOT call this method directly; it dispatches a kernel command and
   * waits for the kernel's `focus-changed` event to flow back through
   * the bridge. See the provider's bridge-effect comment for the full
   * architecture rationale (card `01KQD0WK54G0FRD7SZVZASA9ST`).
   *
   * The `set` name is preserved (rather than renamed to
   * `_setFromKernelEvent`) because direct unit tests of `FocusStore`
   * still drive this method to exercise the subscriber-notification
   * mechanics independently of the kernel pipeline. Production code
   * outside the bridge must not call it.
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
  /**
   * Broadcast a navigation command. Retained as a stable callback in
   * the actions bag so existing call sites (board-view, grid-view,
   * app-shell) compile without churn while the spatial-nav migration
   * completes — but the function no longer walks a predicate registry.
   * All real navigation now lives in the Rust spatial-nav kernel, with
   * React-side directives expressed as `navOverride` props on
   * `<FocusScope>` / `<FocusZone>`.
   * This callback is therefore a no-op that always
   * returns `false`; callers that need to drive navigation should
   * invoke `spatial_navigate` via `useSpatialFocusActions().navigate`.
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

  // Keep the latest dispatch in a ref so the actions bag below can stay
  // identity-stable across re-renders (matches the pre-refactor behavior
  // where setFocus was captured via useCallback with [dispatch]).
  const dispatchRef = useRef(dispatch);
  dispatchRef.current = dispatch;

  // Hold the latest spatial-focus actions in a ref so `setFocus` reads
  // through the kernel pathway in production (where `<SpatialFocusProvider>`
  // is always mounted) and falls back to direct store mutation in test
  // harnesses that skip it. Updating the ref every render keeps the
  // actions bag identity-stable while still picking up provider remounts.
  const spatialFocus = useOptionalSpatialFocusActions();
  const spatialActionsRef = useRef<SpatialFocusActions | null>(spatialFocus);
  spatialActionsRef.current = spatialFocus;

  // Actions bag — created once via a lazy-init ref, identity-stable forever.
  // Every callback reads from refs so they don't need to be recreated.
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

  // Bridge spatial-focus → entity-focus: the kernel is the single
  // source of truth for focus state.
  //
  // The Rust spatial-nav kernel (`SpatialFocusProvider` / `SpatialState`
  // on the Rust side) owns the focused-key map. Every change — leaf
  // click, arrow-key cascade, drill-out, fallback resolution after
  // unregister, AND now `setFocus(moniker)` itself — flows through
  // `spatial_focus*` / `spatial_navigate` and emits a `focus-changed`
  // event with the new `(SpatialKey, Moniker)` pair. The bridge below
  // translates each event into a `store.set(payload.next_moniker)`
  // write so the React-side moniker-keyed store stays in lockstep with
  // the kernel.
  //
  // **Architectural invariant** (card `01KQD0WK54G0FRD7SZVZASA9ST`):
  // this bridge is the ONLY upstream of `store.set` in production.
  // `FocusActions.setFocus(moniker)` no longer mutates the store
  // directly — it dispatches `spatial_focus_by_moniker` and waits for
  // the kernel's emit to flow back through this bridge. That keeps a
  // single source of truth: the Rust kernel.
  //
  // Calling `store.set` directly here (instead of routing back through
  // the `setFocus` action) is critical: under the new contract,
  // `setFocus` dispatches a kernel command, which would emit another
  // `focus-changed`, which would re-enter this bridge — a feedback
  // loop. The store is the projection target; the kernel emit is its
  // authoritative input.
  //
  // The `ui.setFocus` dispatch (scope-chain bookkeeping for the
  // backend's static-scope map) used to live inside `setFocus`; it
  // moves here so every kernel-driven focus move (not just
  // `setFocus(moniker)` calls) keeps the backend's scope chain
  // synchronized. The kernel does not consume `ui.setFocus`, so there
  // is still no feedback loop.
  //
  // Degrades silently when no `<SpatialFocusProvider>` ancestor is
  // mounted (`useOptionalSpatialFocusActions` returns `null`) — older
  // tests that wrap only in `<EntityFocusProvider>` keep their pre-
  // bridge behavior, where `setFocus`'s test-harness fallback writes
  // the store directly.
  useEffect(() => {
    if (!spatialFocus) return;
    return spatialFocus.subscribeFocusChanged((payload) => {
      // `next_moniker` is null when focus clears (window lost focus,
      // focused scope unregistered without a fallback, or
      // `setFocus(null)` dispatched `spatial_clear_focus`). Writing
      // the store directly — not calling the `setFocus` action —
      // preserves the no-feedback-loop invariant: the kernel emit is
      // the input to the projection, never an action that re-enters
      // the kernel.
      store.set(payload.next_moniker);
      const chain = payload.next_moniker
        ? buildScopeChain(payload.next_moniker, registryRef.current)
        : [];
      dispatchRef
        .current({ args: { scope_chain: chain } })
        .catch((error) =>
          console.error("ui.setFocus (kernel bridge) failed:", error),
        );
    });
  }, [spatialFocus, store]);

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
  dispatchRef: MutableRefObject<PreboundDispatch>;
  spatialActionsRef: MutableRefObject<SpatialFocusActions | null>;
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
  const { store, registryRef, dispatchRef, spatialActionsRef } = deps;

  /**
   * Dispatch a "set focus" request to the Rust kernel — the single
   * source of truth for focus state.
   *
   * For a non-null moniker, invokes `spatial_focus_by_moniker`: the
   * kernel resolves the moniker to its registered SpatialKey (via
   * `SpatialRegistry::find_by_moniker`), updates `focus_by_window`,
   * and emits a `focus-changed` event.
   *
   * For `null`, invokes `spatial_clear_focus`: the kernel removes the
   * window's `focus_by_window` slot and emits a
   * `focus-changed { next_key: null, next_moniker: null }` event.
   *
   * In both cases the provider's bridge effect subscribes to those
   * events and writes them into the local `FocusStore` via
   * `store.set` — that is the ONLY upstream of the store. This setter
   * does NOT mutate the store directly, so a synchronous read of
   * `useFocusedMoniker()` immediately after `setFocus(moniker)`
   * returns the OLD value; callers that need the post-write value
   * must await the kernel's emit (which production tests already do
   * via `waitFor`).
   *
   * When the kernel rejects the moniker (unknown to the registry) the
   * Tauri command resolves to `Err(_)`; we surface that as a
   * `console.error` so dev mode catches drift between the React
   * mount tree and the kernel registry. The store stays at its
   * previous value because no `focus-changed` event fires.
   *
   * Falls back to a direct `store.set` ONLY when no
   * `<SpatialFocusProvider>` ancestor is mounted (test harnesses that
   * skip the spatial-nav stack). Production trees always mount both
   * providers, so the kernel pathway is the production one. The
   * fallback also dispatches `ui.setFocus` for scope-chain bookkeeping;
   * the production pathway lets the bridge handle that after the
   * kernel emit.
   *
   * Cards: `01KQD0WK54G0FRD7SZVZASA9ST` (this refactor),
   * `01KQAW97R9XTCNR1PJAWYSKBC7` (no-silent-dropout contract).
   */
  const setFocus = (moniker: string | null): void => {
    const spatial = spatialActionsRef.current;
    if (spatial) {
      // Production pathway: kernel-keyed write. The kernel emits
      // `focus-changed` on success; the bridge effect writes the
      // store and dispatches `ui.setFocus` for scope-chain bookkeeping
      // through the same path the spatial-nav cascade already uses.
      if (moniker === null) {
        // Explicit clear: dispatch `spatial_clear_focus` to the
        // kernel and let the bridge handle the store write when the
        // kernel emits `focus-changed { next_key: null }`. This is
        // the same path every other focus mutation uses — keeping
        // the "store is a pure projection" invariant from card
        // `01KQD0WK54G0FRD7SZVZASA9ST`. Synchronously calling
        // `store.set(null)` here would re-introduce the kernel/React
        // drift the card was filed to eliminate.
        void invoke<void>("spatial_clear_focus").catch((error) => {
          // Adapter-level failure (e.g. lock contention, channel
          // teardown). Log so dev mode catches it; the store stays at
          // its previous value because no `focus-changed` event fires.
          console.error("spatial_clear_focus failed:", error);
        });
        return;
      }
      void invoke<void>("spatial_focus_by_moniker", { moniker }).catch(
        (error) => {
          // The kernel rejected the moniker — log so dev mode catches
          // drift between the React mount tree and the kernel
          // registry. The store stays at its previous value.
          console.error("spatial_focus_by_moniker failed:", error);
        },
      );
      return;
    }

    // Test-harness fallback: no kernel available, so write the store
    // directly and dispatch `ui.setFocus` like the pre-refactor flow.
    // Production trees always mount `<SpatialFocusProvider>`, so this
    // branch never fires in real apps.
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

  /**
   * No-op stand-in for the legacy predicate-broadcast entry point.
   *
   * The pull-based predicate registry (`ClaimPredicate[]` per moniker)
   * has been replaced by the spatial-nav kernel's per-direction
   * `overrides` map plus beam-search resolution, both of which run in
   * Rust. Production code that still calls this method does so as a
   * dead branch — there is nothing to broadcast to. The callable
   * remains in the actions bag so existing call sites compile without
   * churn during the migration.
   *
   * Always returns `false`. Callers that need to drive navigation
   * must invoke `spatial_navigate` via `useSpatialFocusActions`.
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
 * Returns the focus action bag, or `null` when no `EntityFocusProvider`
 * ancestor is mounted.
 *
 * Use this from primitives (`<FocusScope>` / `<FocusZone>`) that must
 * tolerate isolated unit-test harnesses that skip the entity-focus
 * provider stack. Production trees always mount `<EntityFocusProvider>`,
 * so call sites that genuinely require the actions should keep using
 * the throwing `useFocusActions` variant.
 */
export function useOptionalFocusActions(): FocusActions | null {
  return useContext(FocusActionsContext);
}

/**
 * Registers an entity-focus scope for the given moniker, with the same
 * lifecycle the spatial primitives (`<FocusScope>` / `<FocusZone>`) need.
 *
 * The registration runs both inline during render *and* inside a
 * cleanup-only `useEffect`. Why both:
 *
 * - Inline `Map.set` is cheaper than re-firing a `useEffect` whenever the
 *   `scope` object's identity churns. The scope object's identity changes
 *   whenever the parent `CommandScopeContext` rebuilds, which happens on
 *   every ancestor-scope rebuild. Re-firing an effect on every such churn
 *   produces 12k unregister/register pairs per grid render on a 2000-row
 *   board, flooding React's commit phase with cleanups and freezing the UI.
 *   So we hold the latest scope in a ref (updated every render) and
 *   re-register inline — registration is a plain `Map.set`, not a React
 *   effect, so it does not pay React's per-effect overhead.
 *
 * - The effect re-registers on mount (and when `moniker` changes) to cover
 *   the initial paint path where the inline call above has already run but
 *   React may have discarded the render in StrictMode. Cleanup still runs
 *   on real unmount.
 *
 * When no `EntityFocusProvider` ancestor is mounted, both the inline call
 * and the effect become no-ops — the spatial primitives still flow the
 * scope through `CommandScopeContext` for descendants, but the entity-focus
 * dispatcher cannot resolve scope chains through this moniker (which is
 * fine — there is no dispatcher either when the actions bag is null).
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
 * Selective focus subscription that tolerates a missing `EntityFocusProvider`.
 *
 * Behaves identically to `useIsDirectFocus` when an `EntityFocusProvider`
 * ancestor is mounted; returns `false` permanently otherwise. Used by the
 * `<FocusScope>` and `<FocusZone>` primitives so they keep working in
 * isolated unit-test harnesses that skip the entity-focus provider stack
 * — the legacy `<Focusable>` primitive never required `EntityFocusProvider`,
 * and the collapsed primitives must preserve that contract.
 *
 * Hook order is constant per render: when the store is absent, the
 * pseudo-subscribe / pseudo-getSnapshot pair below is identity-stable so
 * `useSyncExternalStore` does not resubscribe spuriously.
 */
export function useOptionalIsDirectFocus(moniker: string): boolean {
  const store = useContext(FocusStoreContext);

  // Identity-stable noop subscribe / getSnapshot for the no-provider branch.
  // Defined inside `useMemo` so the references survive across renders without
  // forcing `useSyncExternalStore` to resubscribe.
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
