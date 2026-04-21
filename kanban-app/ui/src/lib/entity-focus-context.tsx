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

/** Callback type for the spatial focus claim registry. */
export type ClaimCallback = (focused: boolean) => void;

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
   * Dispatch a navigation command to the Rust spatial navigation engine.
   *
   * Maps the command id (e.g. `"nav.right"`) to a direction string,
   * looks up the focused entry's spatial key, and invokes `spatial_navigate`.
   * Returns `true` if the command was dispatched, `false` if unmapped or
   * no focused entry exists.
   */
  broadcastNavCommand: (commandId: string) => boolean;
  /**
   * Register a spatial focus claim. The callback is called with `true` when
   * this key gains focus and `false` when it loses focus, driven by the
   * Rust `focus-changed` event.
   */
  registerClaim: (
    key: string,
    moniker: string,
    callback: ClaimCallback,
  ) => void;
  /** Unregister a spatial focus claim by key. */
  unregisterClaim: (key: string) => void;
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

/** Spatial focus claim registry: three ref-based maps with register/unregister. */
function useClaimRegistry() {
  const claimRegistryRef = useRef<Map<string, ClaimCallback>>(new Map());
  const keyToMonikerRef = useRef<Map<string, string>>(new Map());
  const monikerToKeysRef = useRef<Map<string, Set<string>>>(new Map());

  const registerClaim = useCallback(
    (key: string, moniker: string, callback: ClaimCallback) => {
      claimRegistryRef.current.set(key, callback);
      keyToMonikerRef.current.set(key, moniker);
      const keys = monikerToKeysRef.current.get(moniker) ?? new Set<string>();
      keys.add(key);
      monikerToKeysRef.current.set(moniker, keys);
    },
    [],
  );

  const unregisterClaim = useCallback((key: string) => {
    claimRegistryRef.current.delete(key);
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
    claimRegistryRef,
    keyToMonikerRef,
    monikerToKeysRef,
    registerClaim,
    unregisterClaim,
  };
}

/**
 * Returns a `setFocus` callback that writes the focused moniker, dispatches
 * `ui.setFocus` to the backend, and syncs with Rust spatial state.
 *
 * The moniker is stored only in the ref-backed focus store — there is no
 * parallel React state. Consumers that need to re-render on focus change
 * subscribe via `useFocusedMoniker()` / `useIsFocused()` / `useFocusedScope()`.
 *
 * Claim callbacks for the outgoing and incoming keys are fired synchronously
 * (optimistic UI) so the focus highlight responds immediately without waiting
 * for the Rust `focus-changed` round trip. Rust remains the authoritative
 * owner — its event can override or confirm this local update.
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

function useFocusSetter(
  getFocusedMoniker: () => string | null,
  setFocusedMoniker: (m: string | null) => void,
  dispatch: (opts: { args: Record<string, unknown> }) => Promise<unknown>,
  registryRef: React.RefObject<Map<string, CommandScope>>,
  monikerToKeysRef: React.RefObject<Map<string, Set<string>>>,
  claimRegistryRef: React.RefObject<Map<string, ClaimCallback>>,
) {
  return useCallback(
    (moniker: string | null) => {
      const prev = getFocusedMoniker();
      setFocusedMoniker(moniker);
      if (import.meta.env.DEV) {
        console.warn(`[FocusScope] focus → ${moniker ?? "(none)"}`);
      }
      // Optimistic claim update — flip the un-focus callback for the previous
      // moniker and the focus callback for the new one so the highlight
      // responds immediately. Both transitions are idempotent with the later
      // Rust `focus-changed` event.
      notifyClaim(
        prev,
        false,
        monikerToKeysRef.current,
        claimRegistryRef.current,
      );
      notifyClaim(
        moniker,
        true,
        monikerToKeysRef.current,
        claimRegistryRef.current,
      );
      const chain = moniker
        ? buildScopeChain(moniker, registryRef.current)
        : [];
      dispatch({ args: { scope_chain: chain } }).catch((error) =>
        console.error("ui.setFocus failed:", error),
      );
      syncSpatialFocus(moniker, monikerToKeysRef.current);
    },
    [
      getFocusedMoniker,
      setFocusedMoniker,
      dispatch,
      registryRef,
      monikerToKeysRef,
      claimRegistryRef,
    ],
  );
}

/**
 * Fire the claim callback for every key registered against `moniker`, with
 * the given focus state. Used by `setFocus` to drive the focus highlight
 * immediately on user-initiated focus changes.
 *
 * A single moniker can have multiple spatial keys (one per mounted FocusScope
 * instance sharing that moniker) — all of them receive the notification so
 * every visible highlight stays in sync.
 */
function notifyClaim(
  moniker: string | null,
  focused: boolean,
  monikerToKeys: Map<string, Set<string>>,
  claimRegistry: Map<string, ClaimCallback>,
): void {
  if (!moniker) return;
  const keys = monikerToKeys.get(moniker);
  if (!keys) return;
  for (const key of keys) {
    claimRegistry.get(key)?.(focused);
  }
}

/**
 * Listen for `focus-changed` events from Rust. Drives claim callbacks and
 * updates the focused-moniker store so subscribers (`useFocusedMoniker` etc.)
 * re-render on focus transitions.
 *
 * The event listener is the sole authority for "current focused moniker" —
 * Rust owns focus, React only mirrors the scalar via the ref + listener
 * pattern below.
 *
 * The listener is registered on the current webview window (not the app-wide
 * event bus) so it only fires for `focus-changed` emissions scoped to this
 * window via `window.emit_to(window.label(), ...)` on the Rust side. An
 * app-wide `listen()` would fire in every window regardless of the emit
 * target, causing cross-window focus thrash.
 */
function useFocusChangedEffect(
  claimRegistryRef: React.RefObject<Map<string, ClaimCallback>>,
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
      const { prev_key, next_key } = event.payload;
      if (prev_key) claimRegistryRef.current.get(prev_key)?.(false);
      if (next_key) claimRegistryRef.current.get(next_key)?.(true);
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

/** Map from nav command id to Rust Direction string. */
const NAV_DIRECTION_MAP: Record<string, string> = {
  "nav.up": "Up",
  "nav.down": "Down",
  "nav.left": "Left",
  "nav.right": "Right",
  "nav.first": "First",
  "nav.last": "Last",
  "nav.rowStart": "RowStart",
  "nav.rowEnd": "RowEnd",
};

/**
 * Returns a `broadcastNavCommand` callback that delegates to the Rust
 * spatial navigation engine via `spatial_navigate`.
 *
 * Maps the command id to a direction, looks up the focused moniker's
 * spatial key (if any), and fires an async invoke. Returns `true` if
 * dispatched.
 *
 * When no moniker is focused or no spatial key is registered for it,
 * `key` is `null`. Rust's `spatial_navigate` treats a null/unknown key
 * as "no source" and falls back to the top-left entry in the active
 * layer — the safety net that keeps the "something is always focused"
 * invariant recoverable after a view swap or a React/Rust desync. Do
 * NOT short-circuit here on a null moniker; let Rust pick a successor.
 */
function useBroadcastNav(
  getFocusedMoniker: () => string | null,
  monikerToKeysRef: React.RefObject<Map<string, Set<string>>>,
) {
  return useCallback(
    (commandId: string): boolean => {
      const direction = NAV_DIRECTION_MAP[commandId];
      if (!direction) return false;
      const focusedMk = getFocusedMoniker();
      // Pick the first registered spatial key for the focused moniker,
      // or `null` when the moniker is missing or has no keys. Both cases
      // tell Rust "no source" and trigger the fallback-to-first path.
      let key: string | null = null;
      if (focusedMk) {
        const keys = monikerToKeysRef.current.get(focusedMk);
        if (keys && keys.size > 0) {
          key = keys.values().next().value ?? null;
        }
      }
      invoke("spatial_navigate", { key, direction }).catch(() => {});
      return true;
    },
    [getFocusedMoniker, monikerToKeysRef],
  );
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
 * Provides entity focus state and a scope registry to the component tree.
 * Should be provided once at the App level.
 *
 * Focus state is owned by Rust — this provider holds no `useState` mirror of
 * the focused moniker. Instead, it exposes a subscribe/getSnapshot pair and
 * a set of hooks (`useFocusedMoniker`, `useIsFocused`, `useFocusedScope`)
 * that re-render via `useSyncExternalStore` when focus changes.
 */
function useFocusContextValue(deps: {
  setFocus: EntityFocusContextValue["setFocus"];
  registerScope: EntityFocusContextValue["registerScope"];
  unregisterScope: EntityFocusContextValue["unregisterScope"];
  getScope: EntityFocusContextValue["getScope"];
  broadcastNavCommand: EntityFocusContextValue["broadcastNavCommand"];
  registerClaim: EntityFocusContextValue["registerClaim"];
  unregisterClaim: EntityFocusContextValue["unregisterClaim"];
  getFocusedMoniker: EntityFocusContextValue["getFocusedMoniker"];
  subscribeFocus: EntityFocusContextValue["subscribeFocus"];
}) {
  const {
    setFocus,
    registerScope,
    unregisterScope,
    getScope,
    broadcastNavCommand,
    registerClaim,
    unregisterClaim,
    getFocusedMoniker,
    subscribeFocus,
  } = deps;
  return useMemo<EntityFocusContextValue>(
    () => ({
      setFocus,
      registerScope,
      unregisterScope,
      getScope,
      broadcastNavCommand,
      registerClaim,
      unregisterClaim,
      getFocusedMoniker,
      subscribeFocus,
    }),
    [
      setFocus,
      registerScope,
      unregisterScope,
      getScope,
      broadcastNavCommand,
      registerClaim,
      unregisterClaim,
      getFocusedMoniker,
      subscribeFocus,
    ],
  );
}

export function EntityFocusProvider({ children }: { children: ReactNode }) {
  const dispatch = useDispatchCommand("ui.setFocus");
  const dispatchRef = useRef(dispatch);
  dispatchRef.current = dispatch;

  const { getFocusedMoniker, subscribeFocus, setFocusedMoniker } =
    useFocusedMonikerStore();
  const { registryRef, registerScope, unregisterScope, getScope } =
    useScopeRegistry();
  const {
    claimRegistryRef,
    keyToMonikerRef,
    monikerToKeysRef,
    registerClaim,
    unregisterClaim,
  } = useClaimRegistry();
  const setFocus = useFocusSetter(
    getFocusedMoniker,
    setFocusedMoniker,
    dispatch,
    registryRef,
    monikerToKeysRef,
    claimRegistryRef,
  );
  useFocusChangedEffect(
    claimRegistryRef,
    keyToMonikerRef,
    getFocusedMoniker,
    setFocusedMoniker,
    registryRef,
    dispatchRef,
  );
  const broadcastNavCommand = useBroadcastNav(
    getFocusedMoniker,
    monikerToKeysRef,
  );
  useWindowFocusEffect(getFocusedMoniker, registryRef, dispatchRef);

  const value = useFocusContextValue({
    setFocus,
    registerScope,
    unregisterScope,
    getScope,
    broadcastNavCommand,
    registerClaim,
    unregisterClaim,
    getFocusedMoniker,
    subscribeFocus,
  });

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
