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
import { useDispatchCommand, FocusedScopeContext } from "./command-scope";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** Callback type for the spatial focus claim registry. */
export type ClaimCallback = (focused: boolean) => void;

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

  return { claimRegistryRef, keyToMonikerRef, monikerToKeysRef, registerClaim, unregisterClaim };
}

/**
 * Returns a `setFocus` callback that updates React state, dispatches
 * `ui.setFocus` to the backend, and syncs with Rust spatial state.
 */
function useFocusSetter(
  focusedMonikerRef: React.RefObject<string | null>,
  setFocusedMoniker: (m: string | null) => void,
  dispatch: (opts: { args: Record<string, unknown> }) => Promise<unknown>,
  registryRef: React.RefObject<Map<string, CommandScope>>,
  monikerToKeysRef: React.RefObject<Map<string, Set<string>>>,
) {
  return useCallback(
    (moniker: string | null) => {
      focusedMonikerRef.current = moniker;
      setFocusedMoniker(moniker);
      if (import.meta.env.DEV) {
        console.warn(`[FocusScope] focus → ${moniker ?? "(none)"}`);
      }
      const chain = moniker ? buildScopeChain(moniker, registryRef.current) : [];
      dispatch({ args: { scope_chain: chain } }).catch((error) =>
        console.error("ui.setFocus failed:", error),
      );
      // Sync with Rust spatial state via fire-and-forget invoke.
      if (moniker) {
        const keys = monikerToKeysRef.current.get(moniker);
        if (keys && keys.size > 0) {
          const key = keys.values().next().value;
          invoke("spatial_focus", { key }).catch(() => {});
        }
      } else {
        invoke("spatial_clear_focus").catch(() => {});
      }
    },
    [focusedMonikerRef, setFocusedMoniker, dispatch, registryRef, monikerToKeysRef],
  );
}

/**
 * Listen for `focus-changed` events from Rust. Drives claim callbacks and
 * updates `focusedMoniker` state for backward compatibility.
 */
function useFocusChangedEffect(
  claimRegistryRef: React.RefObject<Map<string, ClaimCallback>>,
  keyToMonikerRef: React.RefObject<Map<string, string>>,
  focusedMonikerRef: React.RefObject<string | null>,
  setFocusedMoniker: (m: string | null) => void,
  registryRef: React.RefObject<Map<string, CommandScope>>,
  dispatchRef: React.RefObject<(opts: { args: Record<string, unknown> }) => Promise<unknown>>,
) {
  useEffect(() => {
    const unlisten = listen<{ prev_key: string | null; next_key: string | null }>(
      "focus-changed",
      (event) => {
        const { prev_key, next_key } = event.payload;
        if (prev_key) claimRegistryRef.current.get(prev_key)?.(false);
        if (next_key) claimRegistryRef.current.get(next_key)?.(true);
        const newMoniker = next_key ? (keyToMonikerRef.current.get(next_key) ?? null) : null;
        if (focusedMonikerRef.current !== newMoniker) {
          focusedMonikerRef.current = newMoniker;
          setFocusedMoniker(newMoniker);
          const chain = newMoniker ? buildScopeChain(newMoniker, registryRef.current) : [];
          dispatchRef.current({ args: { scope_chain: chain } }).catch((e: unknown) =>
            console.error("ui.setFocus (focus-changed) failed:", e),
          );
        }
      },
    );
    return () => { unlisten.then((fn) => fn()); };
  // Refs are stable — effect runs once on mount.
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
 * spatial key, and fires an async invoke. Returns `true` if dispatched.
 */
function useBroadcastNav(
  focusedMonikerRef: React.RefObject<string | null>,
  monikerToKeysRef: React.RefObject<Map<string, Set<string>>>,
) {
  return useCallback(
    (commandId: string): boolean => {
      const direction = NAV_DIRECTION_MAP[commandId];
      if (!direction) return false;
      const focusedMk = focusedMonikerRef.current;
      if (!focusedMk) return false;
      const keys = monikerToKeysRef.current.get(focusedMk);
      if (!keys || keys.size === 0) return false;
      const key = keys.values().next().value;
      invoke("spatial_navigate", { key, direction }).catch(() => {});
      return true;
    },
    [focusedMonikerRef, monikerToKeysRef],
  );
}

/** Re-dispatch the scope chain when the OS window gains focus (Alt-Tab). */
function useWindowFocusEffect(
  focusedMonikerRef: React.RefObject<string | null>,
  registryRef: React.RefObject<Map<string, CommandScope>>,
  dispatchRef: React.RefObject<(opts: { args: Record<string, unknown> }) => Promise<unknown>>,
) {
  useEffect(() => {
    const handler = () => {
      const moniker = focusedMonikerRef.current;
      const chain = moniker ? buildScopeChain(moniker, registryRef.current) : [];
      dispatchRef.current({ args: { scope_chain: chain } }).catch((e) =>
        console.error("ui.setFocus (window focus) failed:", e),
      );
    };
    window.addEventListener("focus", handler);
    return () => window.removeEventListener("focus", handler);
  // Refs are stable — effect runs once on mount.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
}

// ---------------------------------------------------------------------------
// EntityFocusProvider — orchestrates the hooks above
// ---------------------------------------------------------------------------

/**
 * Provides entity focus state and a scope registry to the component tree.
 * Should be provided once at the App level.
 */
export function EntityFocusProvider({ children }: { children: ReactNode }) {
  const [focusedMoniker, setFocusedMoniker] = useState<string | null>(null);
  const dispatch = useDispatchCommand("ui.setFocus");
  const focusedMonikerRef = useRef<string | null>(null);
  const dispatchRef = useRef(dispatch);
  dispatchRef.current = dispatch;

  const { registryRef, registerScope, unregisterScope, getScope } = useScopeRegistry();
  const { claimRegistryRef, keyToMonikerRef, monikerToKeysRef, registerClaim, unregisterClaim } = useClaimRegistry();
  const setFocus = useFocusSetter(focusedMonikerRef, setFocusedMoniker, dispatch, registryRef, monikerToKeysRef);
  useFocusChangedEffect(claimRegistryRef, keyToMonikerRef, focusedMonikerRef, setFocusedMoniker, registryRef, dispatchRef);
  const broadcastNavCommand = useBroadcastNav(focusedMonikerRef, monikerToKeysRef);
  useWindowFocusEffect(focusedMonikerRef, registryRef, dispatchRef);

  const value = useMemo<EntityFocusContextValue>(
    () => ({
      focusedMoniker, setFocus, registerScope, unregisterScope, getScope,
      broadcastNavCommand,
      registerClaim, unregisterClaim,
    }),
    [focusedMoniker, setFocus, registerScope, unregisterScope, getScope,
      broadcastNavCommand,
      registerClaim, unregisterClaim],
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

