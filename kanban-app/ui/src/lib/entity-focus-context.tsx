import {
  createContext,
  useContext,
  useCallback,
  useEffect,
  useRef,
  useMemo,
  useSyncExternalStore,
  type ReactNode,
} from "react";
import type { CommandScope } from "./command-scope";
import { useDispatchCommand, FocusedScopeContext } from "./command-scope";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

/** Listener signature for focused-moniker subscriptions. */
type FocusListener = () => void;

interface EntityFocusContextValue {
  /** Set the focused entity. Pass null to clear focus. */
  setFocus: (moniker: string | null) => void;
  /** Register a scope for a given moniker. Does not trigger re-renders. */
  registerScope: (moniker: string, scope: CommandScope) => void;
  /** Unregister a scope by moniker. Does not trigger re-renders. */
  unregisterScope: (moniker: string) => void;
  /** Look up a registered scope by moniker. */
  getScope: (moniker: string) => CommandScope | null;
  /**
   * Register a spatial key ↔ moniker association.
   *
   * The focused-moniker store is the single source of truth for which
   * scope is visually focused; FocusScope subscribes to the store via
   * `useFocusedMoniker()` and derives `data-focused` by comparing its
   * moniker against the store value. This registration supplies the two
   * maps that Rust-interop code needs:
   *
   * - `key → moniker`, so the `focus-changed` event listener can map the
   *   Rust-chosen spatial key back to the moniker it belongs to.
   * - `moniker → keys`, so `spatial_focus` and `spatial_navigate` can
   *   pick a key to hand to Rust when the React side knows only the
   *   moniker.
   *
   * A single moniker may have multiple registered keys (duplicate mounts
   * of the same FocusScope); all remain registered until individually
   * unregistered.
   */
  registerSpatialKey: (key: string, moniker: string) => void;
  /** Unregister a spatial key ↔ moniker association. */
  unregisterSpatialKey: (key: string) => void;
  /**
   * Read the current focused moniker imperatively.
   *
   * Does NOT subscribe — call this inside event handlers or effects where
   * a snapshot is needed. Use `useFocusedMoniker()` from React render code
   * so the component re-renders when focus changes.
   */
  getFocusedMoniker: () => string | null;
  /**
   * Subscribe to focused-moniker changes. Returns an unsubscribe function.
   *
   * Primarily used by `useFocusedMoniker`, `useIsFocused`, and
   * `useFocusedScope` via `useSyncExternalStore`. Direct callers should
   * typically use those hooks instead of subscribing manually.
   */
  subscribeFocus: (listener: FocusListener) => () => void;
}

const EntityFocusContext = createContext<EntityFocusContextValue | null>(null);

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

// ---------------------------------------------------------------------------
// Custom hooks — extracted to keep EntityFocusProvider under 50 lines
// ---------------------------------------------------------------------------

/**
 * Focused-moniker store: ref-based state with a subscriber list.
 *
 * Replaces the former `useState<focusedMoniker>` so Rust is the sole owner of
 * focus — React state never mirrors it. The ref holds the current value;
 * callers use `setFocusedMoniker` to mutate and fan out, and subscribe via
 * `subscribe()` (consumed by `useSyncExternalStore` in the public hooks).
 */
function useFocusedMonikerStore() {
  const focusedMonikerRef = useRef<string | null>(null);
  const listenersRef = useRef<Set<FocusListener>>(new Set());

  const getFocusedMoniker = useCallback(
    (): string | null => focusedMonikerRef.current,
    [],
  );

  const subscribeFocus = useCallback((listener: FocusListener) => {
    listenersRef.current.add(listener);
    return () => {
      listenersRef.current.delete(listener);
    };
  }, []);

  /**
   * Update the focused moniker and notify subscribers.
   *
   * Skips the notify when the value is unchanged to avoid redundant re-renders
   * in components subscribed via `useSyncExternalStore`.
   */
  const setFocusedMoniker = useCallback((next: string | null) => {
    if (focusedMonikerRef.current === next) return;
    focusedMonikerRef.current = next;
    for (const listener of listenersRef.current) listener();
  }, []);

  return {
    focusedMonikerRef,
    getFocusedMoniker,
    subscribeFocus,
    setFocusedMoniker,
  };
}

/** Scope registry: ref-based Map<moniker, CommandScope> with register/unregister/get. */
function useScopeRegistry() {
  const registryRef = useRef<Map<string, CommandScope>>(new Map());
  const registerScope = useCallback((moniker: string, scope: CommandScope) => {
    registryRef.current.set(moniker, scope);
  }, []);
  const unregisterScope = useCallback((moniker: string) => {
    registryRef.current.delete(moniker);
  }, []);
  const getScope = useCallback((moniker: string): CommandScope | null => {
    return registryRef.current.get(moniker) ?? null;
  }, []);
  return { registryRef, registerScope, unregisterScope, getScope };
}

/**
 * Spatial key registry: two ref-based maps binding `key ↔ moniker`.
 *
 * The registry exists purely to bridge the Rust spatial-nav key space
 * (opaque ULIDs used by the beam-test graph) with the moniker space
 * (React-level identity used by command scopes and focus decoration).
 *
 * Visual focus state is NOT stored here — FocusScope pulls it from the
 * focused-moniker store via `useFocusedMoniker()` and compares against
 * its own moniker on every render. The registry has no subscribers and
 * no per-key callbacks; it is a pure lookup table.
 */
function useSpatialKeyRegistry() {
  const keyToMonikerRef = useRef<Map<string, string>>(new Map());
  const monikerToKeysRef = useRef<Map<string, Set<string>>>(new Map());

  const registerSpatialKey = useCallback((key: string, moniker: string) => {
    keyToMonikerRef.current.set(key, moniker);
    const keys = monikerToKeysRef.current.get(moniker) ?? new Set<string>();
    keys.add(key);
    monikerToKeysRef.current.set(moniker, keys);
  }, []);

  const unregisterSpatialKey = useCallback((key: string) => {
    const moniker = keyToMonikerRef.current.get(key);
    keyToMonikerRef.current.delete(key);
    if (moniker) {
      const keys = monikerToKeysRef.current.get(moniker);
      if (keys) {
        keys.delete(key);
        if (keys.size === 0) monikerToKeysRef.current.delete(moniker);
      }
    }
  }, []);

  return {
    keyToMonikerRef,
    monikerToKeysRef,
    registerSpatialKey,
    unregisterSpatialKey,
  };
}

/**
 * Forward the focus change to Rust's spatial state. Picks the first
 * registered key for `moniker` (if any) and invokes `spatial_focus`;
 * when `moniker` is `null`, invokes `spatial_clear_focus`.
 *
 * Rust is the authoritative owner of spatial focus — its eventual
 * `focus-changed` event re-confirms or overrides this update via the
 * focused-moniker store. FocusScope subscribes to the store and
 * re-derives its visual state, so a later Rust-driven correction
 * (e.g. a successor selected because `moniker` was not a valid target
 * on Rust's side) automatically repaints without any separate push.
 */
function syncSpatialFocus(
  moniker: string | null,
  monikerToKeys: Map<string, Set<string>>,
) {
  if (!moniker) {
    invoke("spatial_clear_focus").catch(() => {});
    return;
  }
  const keys = monikerToKeys.get(moniker);
  if (keys && keys.size > 0) {
    const key = keys.values().next().value;
    invoke("spatial_focus", { key }).catch(() => {});
  }
}

/**
 * Returns a `setFocus` callback that writes the focused moniker, dispatches
 * `ui.setFocus` to the backend, and syncs with Rust spatial state.
 *
 * The moniker is stored only in the ref-backed focus store — there is no
 * parallel React state, and no push-based claim notification fan-out.
 * FocusScope subscribes to the store via `useFocusedMoniker()` and
 * re-derives `data-focused` on every change by comparing its moniker to
 * the new store value. Stale focus decorations are impossible by
 * construction: the store is the single source of truth, and every
 * scope re-evaluates against it.
 */
function useFocusSetter(
  setFocusedMoniker: (m: string | null) => void,
  dispatch: (opts: { args: Record<string, unknown> }) => Promise<unknown>,
  registryRef: React.RefObject<Map<string, CommandScope>>,
  monikerToKeysRef: React.RefObject<Map<string, Set<string>>>,
) {
  return useCallback(
    (moniker: string | null) => {
      setFocusedMoniker(moniker);
      if (import.meta.env.DEV) {
        console.warn(`[FocusScope] focus → ${moniker ?? "(none)"}`);
      }
      const chain = moniker
        ? buildScopeChain(moniker, registryRef.current)
        : [];
      dispatch({ args: { scope_chain: chain } }).catch((error) =>
        console.error("ui.setFocus failed:", error),
      );
      syncSpatialFocus(moniker, monikerToKeysRef.current);
    },
    [setFocusedMoniker, dispatch, registryRef, monikerToKeysRef],
  );
}

/**
 * Listen for `focus-changed` events from Rust and update the focused-moniker
 * store. FocusScope subscribers re-render via `useSyncExternalStore` and
 * derive `data-focused` by comparing their moniker to the new store value.
 *
 * The event listener is the sole authority for "current focused moniker" —
 * Rust owns focus, React only mirrors the scalar via the ref + listener
 * pattern below. No per-key push callbacks fan out from here; every
 * subscribed scope re-evaluates against the store on its own.
 *
 * The listener is registered on the current webview window (not the app-wide
 * event bus) so it only fires for `focus-changed` emissions scoped to this
 * window via `window.emit_to(window.label(), ...)` on the Rust side. An
 * app-wide `listen()` would fire in every window regardless of the emit
 * target, causing cross-window focus thrash.
 */
function useFocusChangedEffect(
  keyToMonikerRef: React.RefObject<Map<string, string>>,
  getFocusedMoniker: () => string | null,
  setFocusedMoniker: (m: string | null) => void,
  registryRef: React.RefObject<Map<string, CommandScope>>,
  dispatchRef: React.RefObject<
    (opts: { args: Record<string, unknown> }) => Promise<unknown>
  >,
) {
  useEffect(() => {
    const webview = getCurrentWebviewWindow();
    const unlisten = webview.listen<{
      prev_key: string | null;
      next_key: string | null;
    }>("focus-changed", (event) => {
      const { next_key } = event.payload;
      const newMoniker = next_key
        ? (keyToMonikerRef.current.get(next_key) ?? null)
        : null;
      if (getFocusedMoniker() !== newMoniker) {
        setFocusedMoniker(newMoniker);
        const chain = newMoniker
          ? buildScopeChain(newMoniker, registryRef.current)
          : [];
        dispatchRef
          .current({ args: { scope_chain: chain } })
          .catch((e: unknown) =>
            console.error("ui.setFocus (focus-changed) failed:", e),
          );
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
    // Refs and setters are stable — effect runs once on mount.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
}

/** Re-dispatch the scope chain when the OS window gains focus (Alt-Tab). */
function useWindowFocusEffect(
  getFocusedMoniker: () => string | null,
  registryRef: React.RefObject<Map<string, CommandScope>>,
  dispatchRef: React.RefObject<
    (opts: { args: Record<string, unknown> }) => Promise<unknown>
  >,
) {
  useEffect(() => {
    const handler = () => {
      const moniker = getFocusedMoniker();
      const chain = moniker
        ? buildScopeChain(moniker, registryRef.current)
        : [];
      dispatchRef
        .current({ args: { scope_chain: chain } })
        .catch((e) => console.error("ui.setFocus (window focus) failed:", e));
    };
    window.addEventListener("focus", handler);
    return () => window.removeEventListener("focus", handler);
    // Refs and getters are stable — effect runs once on mount.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
}

// ---------------------------------------------------------------------------
// EntityFocusProvider — orchestrates the hooks above
// ---------------------------------------------------------------------------

/**
 * Memoize the `EntityFocusContextValue` exposed by `EntityFocusProvider`.
 *
 * All deps are stable identities from the hooks above; this hook just
 * packages them into a single context value and stabilizes it across
 * re-renders so consumers don't thrash.
 */
function useFocusContextValue(deps: {
  setFocus: EntityFocusContextValue["setFocus"];
  registerScope: EntityFocusContextValue["registerScope"];
  unregisterScope: EntityFocusContextValue["unregisterScope"];
  getScope: EntityFocusContextValue["getScope"];
  registerSpatialKey: EntityFocusContextValue["registerSpatialKey"];
  unregisterSpatialKey: EntityFocusContextValue["unregisterSpatialKey"];
  getFocusedMoniker: EntityFocusContextValue["getFocusedMoniker"];
  subscribeFocus: EntityFocusContextValue["subscribeFocus"];
}) {
  const {
    setFocus,
    registerScope,
    unregisterScope,
    getScope,
    registerSpatialKey,
    unregisterSpatialKey,
    getFocusedMoniker,
    subscribeFocus,
  } = deps;
  return useMemo<EntityFocusContextValue>(
    () => ({
      setFocus,
      registerScope,
      unregisterScope,
      getScope,
      registerSpatialKey,
      unregisterSpatialKey,
      getFocusedMoniker,
      subscribeFocus,
    }),
    [
      setFocus,
      registerScope,
      unregisterScope,
      getScope,
      registerSpatialKey,
      unregisterSpatialKey,
      getFocusedMoniker,
      subscribeFocus,
    ],
  );
}

/**
 * Provides entity focus state and a scope registry to the component tree.
 * Should be provided once at the App level.
 *
 * Focus state is owned by Rust — this provider holds no `useState` mirror of
 * the focused moniker. Instead, it exposes a subscribe/getSnapshot pair and
 * a set of hooks (`useFocusedMoniker`, `useIsFocused`, `useFocusedScope`)
 * that re-render via `useSyncExternalStore` when focus changes.
 */
/**
 * Wire together all focus-provider subsystems: dispatch ref, focused-moniker
 * store, scope registry, claim registry, setFocus, focus-changed effect,
 * nav broadcaster, window-focus effect.
 *
 * Returns everything `EntityFocusProvider` needs to assemble its context
 * value and its `FocusedScopeContext`.
 */
function useFocusProviderInternals() {
  const dispatch = useDispatchCommand("ui.setFocus");
  const dispatchRef = useRef(dispatch);
  dispatchRef.current = dispatch;
  const { getFocusedMoniker, subscribeFocus, setFocusedMoniker } =
    useFocusedMonikerStore();
  const { registryRef, registerScope, unregisterScope, getScope } =
    useScopeRegistry();
  const {
    keyToMonikerRef,
    monikerToKeysRef,
    registerSpatialKey,
    unregisterSpatialKey,
  } = useSpatialKeyRegistry();
  const setFocus = useFocusSetter(
    setFocusedMoniker,
    dispatch,
    registryRef,
    monikerToKeysRef,
  );
  useFocusChangedEffect(
    keyToMonikerRef,
    getFocusedMoniker,
    setFocusedMoniker,
    registryRef,
    dispatchRef,
  );
  useWindowFocusEffect(getFocusedMoniker, registryRef, dispatchRef);
  return {
    getFocusedMoniker,
    subscribeFocus,
    registryRef,
    registerScope,
    unregisterScope,
    getScope,
    registerSpatialKey,
    unregisterSpatialKey,
    setFocus,
  };
}

/**
 * Provides entity focus state and a scope registry to the component tree.
 * Should be provided once at the App level.
 *
 * Focus state is owned by Rust — this provider holds no `useState` mirror of
 * the focused moniker. Instead, it exposes a subscribe/getSnapshot pair and
 * a set of hooks (`useFocusedMoniker`, `useIsFocused`, `useFocusedScope`)
 * that re-render via `useSyncExternalStore` when focus changes.
 */
export function EntityFocusProvider({ children }: { children: ReactNode }) {
  const internals = useFocusProviderInternals();
  const { getFocusedMoniker, subscribeFocus, registryRef } = internals;
  const value = useFocusContextValue(internals);

  // Subscribe the provider itself so FocusedScopeContext re-renders when focus
  // changes — useDispatchCommand depends on this context to compute scope chains.
  const focusedMoniker = useSyncExternalStore(
    subscribeFocus,
    getFocusedMoniker,
  );
  const focusedScope = focusedMoniker
    ? (registryRef.current.get(focusedMoniker) ?? null)
    : null;

  return (
    <EntityFocusContext.Provider value={value}>
      <FocusedScopeContext.Provider value={focusedScope}>
        {children}
      </FocusedScopeContext.Provider>
    </EntityFocusContext.Provider>
  );
}

/**
 * Returns the entity focus context (setters and registries).
 * Must be used within an EntityFocusProvider.
 *
 * Does NOT subscribe to focus changes. To read the focused moniker reactively,
 * use `useFocusedMoniker()`; for imperative snapshot reads from inside an
 * effect or event handler, call `getFocusedMoniker()` on the returned object.
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
 * Subscribe to the focused moniker and re-render when it changes.
 *
 * Returns the current focused moniker, or `null` when nothing is focused.
 * This is the idiomatic hook for React code that needs to render differently
 * based on which entity has focus. For imperative reads (e.g. inside
 * event handlers), call `useEntityFocus().getFocusedMoniker()` instead.
 */
export function useFocusedMoniker(): string | null {
  const { subscribeFocus, getFocusedMoniker } = useEntityFocus();
  return useSyncExternalStore(subscribeFocus, getFocusedMoniker);
}

/**
 * Returns the CommandScope of the currently focused entity, or null.
 *
 * Subscribes to focus changes so the component re-renders when focus moves.
 * The registry itself is stored in a ref (scope registrations don't trigger
 * re-renders) — the returned value reflects the registry state at the time
 * focus changed. In practice this is safe because `setFocus` is called from
 * click handlers (after mount) while scopes register during mount effects.
 */
export function useFocusedScope(): CommandScope | null {
  const focusedMoniker = useFocusedMoniker();
  const { getScope } = useEntityFocus();
  if (focusedMoniker === null) return null;
  return getScope(focusedMoniker);
}

/**
 * Returns true if the given moniker is the focused moniker or an ancestor
 * of the focused scope in the scope chain.
 *
 * Subscribes to focus changes so consumers re-render when focus moves into
 * or out of the subtree. Walks the scope-registry ancestor chain starting
 * from the focused scope, matching by `scope.moniker`.
 *
 * @param moniker - The moniker to test.
 * @returns true if directly focused or an ancestor of the focused scope.
 */
export function useIsFocused(moniker: string): boolean {
  const focusedMoniker = useFocusedMoniker();
  const { getScope } = useEntityFocus();
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
