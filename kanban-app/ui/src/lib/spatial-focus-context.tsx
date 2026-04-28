/**
 * Spatial focus claim registry — per-window, event-driven.
 *
 * Mirrors the Rust-side `SpatialState` in `swissarmyhammer-kanban/src/focus/state.rs`.
 * Rust owns the focused-key map (per `WindowLabel`); the React side keeps a
 * `Map<SpatialKey, (focused: boolean) => void>` and a single global
 * `focus-changed` event listener that dispatches `false` to the previously
 * focused key's callback and `true` to the newly focused key's.
 *
 * Each Tauri window has its own React tree and therefore its own claim
 * registry, so a `focus-changed` event for another window's key is a
 * silent no-op here — the lookup misses and nothing fires. We do not
 * filter on `window_label` because Tauri's emit-to-all behavior is
 * symmetric: every window receives every event, but only the window that
 * actually mounted the matching `<FocusScope>` will have a callback to
 * dispatch to.
 *
 * This file does **not** replace `entity-focus-context.tsx` — that
 * context still drives the moniker-keyed scope registry, command-scope
 * chain, and the legacy `setFocus` dispatch path. The claim registry is
 * an additional, opt-in layer that lets a `<FocusScope>` subscribe to its
 * own focus state by `SpatialKey` without re-rendering the whole tree.
 */

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  type ReactNode,
} from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import type {
  Direction,
  FocusChangedPayload,
  FocusOverrides,
  LayerKey,
  LayerName,
  Moniker,
  Rect,
  SpatialKey,
} from "@/types/spatial";

// ---------------------------------------------------------------------------
// Claim registry — per-key callbacks
// ---------------------------------------------------------------------------

/**
 * Callback signature for a claim listener.
 *
 * Receives `true` when the keyed scope just gained focus and `false` when
 * it just lost it. Implementations should make their state update
 * cheap — the listener fires on the React render path of the event
 * dispatch, and a slow callback delays the corresponding visual update.
 */
export type FocusClaimListener = (focused: boolean) => void;

/**
 * Callback signature for a broad `focus-changed` subscriber.
 *
 * Unlike `FocusClaimListener` (which fires only when a specific
 * `SpatialKey` gains or loses focus), this listener observes every
 * `focus-changed` payload in full. Used by integrations that need to
 * bridge spatial-focus into another store keyed by a different
 * identity — most importantly, `EntityFocusProvider`, which mirrors
 * `next_moniker` into its legacy moniker-keyed `FocusStore` so the
 * `focusedMonikerRef` API stays in sync with spatial moves.
 *
 * Subscribers run synchronously on the same dispatch tick as
 * per-key claim listeners, so the work they do should be cheap. Calling
 * back into Tauri (e.g. dispatching `ui.setFocus` to forward the new
 * scope chain) is acceptable — the bridge already does it.
 */
export type FocusChangedSubscriber = (payload: FocusChangedPayload) => void;

/**
 * The set of imperative actions exposed by `SpatialFocusProvider`.
 *
 * Stored in a context whose value is set once and never changes — every
 * closure reads from refs internally, so consumers that only need to
 * register/unregister listeners or invoke spatial commands never re-render
 * on focus moves.
 */
export interface SpatialFocusActions {
  /**
   * Register a focus-claim listener for `key`. Returns the unsubscribe
   * function — call it on component unmount to remove the entry from the
   * registry. Replacing an existing entry with the same key is allowed
   * but rare in practice (each `<FocusScope>` mounts exactly one).
   */
  registerClaim: (key: SpatialKey, listener: FocusClaimListener) => () => void;
  /** Read the listener for a key, primarily for tests. */
  hasClaim: (key: SpatialKey) => boolean;
  /** Invoke `spatial_focus` for the given key in the current window. */
  focus: (key: SpatialKey) => Promise<void>;
  /**
   * Invoke `spatial_register_scope` with the full kernel-types record:
   * stable key, entity moniker, viewport rect, owning layer, optional
   * enclosing zone, and per-direction overrides.
   *
   * Mirrors [`FocusScope`] on the Rust side — the leaf primitive. Pass
   * `null` for `parentZone` when the leaf is registered directly under
   * the layer root, and an empty object for `overrides` when the leaf
   * has no per-direction special cases.
   */
  registerScope: (
    key: SpatialKey,
    moniker: Moniker,
    rect: Rect,
    layerKey: LayerKey,
    parentZone: SpatialKey | null,
    overrides: FocusOverrides,
  ) => Promise<void>;
  /**
   * Invoke `spatial_register_zone` with the full kernel-types record.
   *
   * Mirrors `FocusZone` on the Rust side. Same parameter shape as
   * `registerScope`; the difference is the `Zone` variant in the
   * registry, which owns a `last_focused` slot for drill-out / fallback
   * memory. The slot is always initialized to `None` on register — the
   * navigator populates it as focus moves through the zone.
   */
  registerZone: (
    key: SpatialKey,
    moniker: Moniker,
    rect: Rect,
    layerKey: LayerKey,
    parentZone: SpatialKey | null,
    overrides: FocusOverrides,
  ) => Promise<void>;
  /** Invoke `spatial_unregister_scope` for the given key. */
  unregisterScope: (key: SpatialKey) => Promise<void>;
  /**
   * Invoke `spatial_update_rect` to refresh the bounding rect of a
   * registered scope. Call from a ResizeObserver on the underlying DOM
   * node; no-op on the Rust side if the key is unknown.
   */
  updateRect: (key: SpatialKey, rect: Rect) => Promise<void>;
  /** Invoke `spatial_navigate` from `key` in `direction`. */
  navigate: (key: SpatialKey, direction: Direction) => Promise<void>;
  /** Invoke `spatial_push_layer` for the given key/name/parent. */
  pushLayer: (
    key: LayerKey,
    name: LayerName,
    parent: LayerKey | null,
  ) => Promise<void>;
  /** Invoke `spatial_pop_layer` for the given key. */
  popLayer: (key: LayerKey) => Promise<void>;
  /**
   * Invoke `spatial_drill_in` to compute the [`Moniker`] to focus when
   * the user drills *into* the scope at `key`.
   *
   * Returns the new target's moniker, or `null` when the registry has
   * nothing to descend into (drill-in on a leaf, an empty zone, or an
   * unknown key). The caller then dispatches `setFocus(moniker)` on a
   * `Moniker` result, or falls through to the next command in the
   * chain on `null` (e.g. inline edit on a leaf with an editor).
   *
   * Mirrors `SpatialRegistry::drill_in` on the Rust side — purely a
   * registry query, no focus state mutation.
   */
  drillIn: (key: SpatialKey) => Promise<Moniker | null>;
  /**
   * Invoke `spatial_drill_out` to compute the [`Moniker`] to focus when
   * the user drills *out of* the scope at `key`.
   *
   * Returns the moniker of the scope's enclosing zone, or `null` when
   * there is no enclosing zone (the scope sits at the layer root) or
   * the key is unknown. On `null`, the React Escape chain falls
   * through to `app.dismiss` (close the topmost modal layer).
   *
   * Mirrors `SpatialRegistry::drill_out` on the Rust side — purely a
   * registry query, no focus state mutation.
   */
  drillOut: (key: SpatialKey) => Promise<Moniker | null>;
  /**
   * Read the [`SpatialKey`] currently focused in the active window, or
   * `null` if the window has no focus yet.
   *
   * Read on demand from the latest `focus-changed` event the provider
   * has observed; safe to call from event handlers without
   * re-rendering. Used by the global keybinding handler to thread the
   * focused key into `drillIn` / `drillOut` without round-tripping
   * through the entity-focus moniker store.
   */
  focusedKey: () => SpatialKey | null;
  /**
   * Subscribe to every `focus-changed` payload the provider observes.
   *
   * Returns an unsubscribe function — call it on unmount to remove the
   * entry. Used by integrations that need to bridge spatial focus into
   * a peer store: most notably `EntityFocusProvider`, which mirrors
   * `payload.next_moniker` into its moniker-keyed `FocusStore` so
   * `useFocusedMonikerRef` and `useFocusedScope` stay in sync with
   * spatial moves — keeping `extractScopeBindings` (the keymap
   * handler's scope-binding source) and downstream consumers honest.
   *
   * Subscribers fire synchronously alongside per-key claim listeners on
   * the same dispatch tick, so the callback should keep its work cheap.
   * The provider runs every registered subscriber regardless of whether
   * `next_key` matches any `SpatialKey` in the local claim registry —
   * the broadcast is unconditional, mirroring the all-windows reach of
   * Tauri's `emit`.
   */
  subscribeFocusChanged: (
    subscriber: FocusChangedSubscriber,
  ) => () => void;
}

const SpatialFocusContext = createContext<SpatialFocusActions | null>(null);

// ---------------------------------------------------------------------------
// Provider
// ---------------------------------------------------------------------------

/**
 * Provider for the spatial focus claim registry.
 *
 * Mounts a single global `focus-changed` listener; on every event, looks
 * up `payload.prev_key` and `payload.next_key` in the local registry and
 * dispatches `false` / `true` to whichever callbacks are registered.
 * Unmount removes the listener so a hot-reloaded provider does not leak
 * subscriptions.
 *
 * Callers should mount this once at the root of every Tauri window — each
 * window has its own React tree and therefore needs its own provider.
 */
export function SpatialFocusProvider({ children }: { children: ReactNode }) {
  // Registry of per-`SpatialKey` callbacks. Held in a ref so registrations
  // do not cause re-renders — the only thing that re-renders on a focus
  // change is the `<FocusScope>` whose listener fires.
  const registryRef = useRef<Map<SpatialKey, FocusClaimListener>>(new Map());

  // Set of broad `focus-changed` subscribers. Kept in a `Set` (not an
  // array) so unsubscribe is O(1) and accidental double-registration of
  // the same listener identity is a no-op. Held in a ref for the same
  // reason as `registryRef`: subscriber churn must not re-render the
  // provider tree.
  const subscribersRef = useRef<Set<FocusChangedSubscriber>>(new Set());

  // Latest focused `SpatialKey` from `focus-changed` events. Tracked in a
  // ref because the global keybinding handler needs to read it on every
  // keystroke without re-registering. Mirrors what `SpatialState` holds
  // on the Rust side, scoped to this window — `focus-changed` events for
  // other windows arrive here too but their `next_key` is not registered
  // in the local claim map, so the misroute is silent (and we still record
  // the latest key, since Tauri's emit-to-all guarantees only the matching
  // window's React tree mounts the corresponding scope).
  const focusedKeyRef = useRef<SpatialKey | null>(null);

  // Subscribe to the global `focus-changed` event exactly once for the
  // provider's lifetime. The cleanup is critical: an unmounted provider
  // that left its listener live would receive every focus-changed event
  // for the rest of the process and call into the now-empty registry,
  // leaking memory holding the closure references.
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    let cancelled = false;

    listen<FocusChangedPayload>("focus-changed", ({ payload }) => {
      const registry = registryRef.current;
      if (payload.prev_key !== null) {
        registry.get(payload.prev_key)?.(false);
      }
      if (payload.next_key !== null) {
        registry.get(payload.next_key)?.(true);
      }
      // Record the latest focused key so `drillIn` / `drillOut` callers
      // can thread it through without consulting the entity-focus
      // moniker store.
      focusedKeyRef.current = payload.next_key;
      // Fan out the full payload to broad subscribers (e.g. the
      // entity-focus bridge in `EntityFocusProvider`). Iteration walks a
      // snapshot so subscriber callbacks that unsubscribe themselves
      // (or each other) mid-fire don't perturb the visit order.
      const snapshot = Array.from(subscribersRef.current);
      for (const sub of snapshot) sub(payload);
    }).then((fn) => {
      if (cancelled) {
        // Provider unmounted before `listen` resolved — fire the unlisten
        // immediately so we don't leak a stranded listener.
        fn();
      } else {
        unlisten = fn;
      }
    });

    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, []);

  // Stable actions bag. Built once via a lazy-init ref so consumers that
  // only need actions never re-render — every closure reads from the
  // registry ref, not React state.
  const actionsRef = useRef<SpatialFocusActions | null>(null);
  if (actionsRef.current === null) {
    actionsRef.current = buildSpatialFocusActions(
      registryRef,
      subscribersRef,
      focusedKeyRef,
    );
  }

  return (
    <SpatialFocusContext.Provider value={actionsRef.current}>
      {children}
    </SpatialFocusContext.Provider>
  );
}

/**
 * Build the identity-stable actions bag for the spatial focus provider.
 *
 * Pulled out of the provider body so the component stays small and the
 * action implementations sit in one place — matching the
 * `buildFocusActions` split in `entity-focus-context.tsx`.
 */
function buildSpatialFocusActions(
  registryRef: React.MutableRefObject<Map<SpatialKey, FocusClaimListener>>,
  subscribersRef: React.MutableRefObject<Set<FocusChangedSubscriber>>,
  focusedKeyRef: React.MutableRefObject<SpatialKey | null>,
): SpatialFocusActions {
  const registerClaim: SpatialFocusActions["registerClaim"] = (
    key,
    listener,
  ) => {
    registryRef.current.set(key, listener);
    return () => {
      // Compare against the listener identity so we don't accidentally
      // remove a successor that registered under the same key after this
      // entry was replaced. The map only stores the latest listener per
      // key; a stale unsubscribe should be a no-op.
      const current = registryRef.current.get(key);
      if (current === listener) {
        registryRef.current.delete(key);
      }
    };
  };

  const hasClaim: SpatialFocusActions["hasClaim"] = (key) =>
    registryRef.current.has(key);

  const focus: SpatialFocusActions["focus"] = async (key) => {
    await invoke("spatial_focus", { key });
  };

  const registerScope: SpatialFocusActions["registerScope"] = async (
    key,
    moniker,
    rect,
    layerKey,
    parentZone,
    overrides,
  ) => {
    // Tauri serializes argument names verbatim — they must match the
    // Rust command signature, which uses snake_case. The TS callers use
    // camelCase locally; the conversion happens here so each consumer
    // stays in idiomatic JS land.
    await invoke("spatial_register_scope", {
      key,
      moniker,
      rect,
      layerKey,
      parentZone,
      overrides,
    });
  };

  const registerZone: SpatialFocusActions["registerZone"] = async (
    key,
    moniker,
    rect,
    layerKey,
    parentZone,
    overrides,
  ) => {
    await invoke("spatial_register_zone", {
      key,
      moniker,
      rect,
      layerKey,
      parentZone,
      overrides,
    });
  };

  const unregisterScope: SpatialFocusActions["unregisterScope"] = async (
    key,
  ) => {
    await invoke("spatial_unregister_scope", { key });
  };

  const updateRect: SpatialFocusActions["updateRect"] = async (key, rect) => {
    await invoke("spatial_update_rect", { key, rect });
  };

  const navigate: SpatialFocusActions["navigate"] = async (key, direction) => {
    await invoke("spatial_navigate", { key, direction });
  };

  const pushLayer: SpatialFocusActions["pushLayer"] = async (
    key,
    name,
    parent,
  ) => {
    await invoke("spatial_push_layer", { key, name, parent });
  };

  const popLayer: SpatialFocusActions["popLayer"] = async (key) => {
    await invoke("spatial_pop_layer", { key });
  };

  const drillIn: SpatialFocusActions["drillIn"] = async (key) => {
    return await invoke<Moniker | null>("spatial_drill_in", { key });
  };

  const drillOut: SpatialFocusActions["drillOut"] = async (key) => {
    return await invoke<Moniker | null>("spatial_drill_out", { key });
  };

  const focusedKey: SpatialFocusActions["focusedKey"] = () =>
    focusedKeyRef.current;

  const subscribeFocusChanged: SpatialFocusActions["subscribeFocusChanged"] = (
    subscriber,
  ) => {
    subscribersRef.current.add(subscriber);
    return () => {
      subscribersRef.current.delete(subscriber);
    };
  };

  return {
    registerClaim,
    hasClaim,
    focus,
    registerScope,
    registerZone,
    unregisterScope,
    updateRect,
    navigate,
    pushLayer,
    popLayer,
    drillIn,
    drillOut,
    focusedKey,
    subscribeFocusChanged,
  };
}

// ---------------------------------------------------------------------------
// Consumer hooks
// ---------------------------------------------------------------------------

/**
 * Read the spatial focus actions bag.
 *
 * Returns the same identity-stable object for the provider's lifetime —
 * a destructured `const { focus } = useSpatialFocusActions()` keeps `focus`
 * stable across renders and is safe to use in a `useEffect` dep list.
 *
 * Throws if called outside `SpatialFocusProvider`.
 */
export function useSpatialFocusActions(): SpatialFocusActions {
  const ctx = useContext(SpatialFocusContext);
  if (!ctx)
    throw new Error(
      "useSpatialFocusActions must be used within a SpatialFocusProvider",
    );
  return ctx;
}

/**
 * Read the spatial focus actions bag, or `null` when no provider wraps
 * the caller.
 *
 * Use from primitives that should silently degrade outside the spatial-nav
 * stack (e.g. `<FocusZone>` mounted in a unit test without a
 * `<SpatialFocusProvider>` wrapper). The strict variant
 * `useSpatialFocusActions` is still the right choice anywhere the absence
 * of the provider is a contract violation rather than a tolerated state.
 */
export function useOptionalSpatialFocusActions(): SpatialFocusActions | null {
  return useContext(SpatialFocusContext);
}

/**
 * Subscribe to focus changes for a single `SpatialKey`.
 *
 * Calls `listener(true)` when the key gains focus and `listener(false)`
 * when it loses focus. The listener is registered on mount and cleaned up
 * on unmount; subsequent listener identities replace the previous one (the
 * registry stores at most one callback per key).
 *
 * `listener` is intentionally read through a ref, so callers can pass an
 * inline arrow function without paying for re-registration on every
 * render.
 */
export function useFocusClaim(
  key: SpatialKey,
  listener: FocusClaimListener,
): void {
  const { registerClaim } = useSpatialFocusActions();
  const listenerRef = useRef(listener);
  listenerRef.current = listener;

  // Stable shim that delegates to whatever listenerRef points at. This
  // means we register exactly once per (provider, key) pair — re-renders
  // that change the listener identity do not trigger re-registration.
  const stableListener = useCallback(
    (focused: boolean) => listenerRef.current(focused),
    [],
  );

  useEffect(() => {
    return registerClaim(key, stableListener);
  }, [registerClaim, key, stableListener]);
}
