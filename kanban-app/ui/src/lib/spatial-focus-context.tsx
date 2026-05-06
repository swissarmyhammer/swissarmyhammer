/**
 * Spatial focus claim registry — per-window, event-driven.
 *
 * Mirrors the Rust-side `SpatialState` in `swissarmyhammer-focus/src/state.rs`.
 * Rust owns the focused-FQM map (per `WindowLabel`); the React side keeps a
 * `Map<FullyQualifiedMoniker, (focused: boolean) => void>` and a single
 * global `focus-changed` event listener that dispatches `false` to the
 * previously focused FQM's callback and `true` to the newly focused one's.
 *
 * Each Tauri window has its own React tree and therefore its own claim
 * registry, so a `focus-changed` event for another window's FQM is a
 * silent no-op here — the lookup misses and nothing fires. We do not
 * filter on `window_label` because Tauri's emit-to-all behavior is
 * symmetric: every window receives every event, but only the window that
 * actually mounted the matching `<FocusScope>` will have a callback to
 * dispatch to.
 *
 * This file does **not** replace `entity-focus-context.tsx` — that
 * context still drives the entity scope registry and command-scope
 * chain. The claim registry is an additional layer that lets a
 * `<FocusScope>` subscribe to its own focus state by FQM without
 * re-rendering the whole tree.
 *
 * # Path-monikers identity model
 *
 * After card `01KQD6064G1C1RAXDFPJVT1F46` the kernel uses one identifier
 * shape per primitive: `FullyQualifiedMoniker`. The FQM is the spatial
 * key — there is no UUID-based `SpatialKey`. Every action below takes a
 * fully-qualified path; the React side composes it from
 * `FullyQualifiedMonikerContext` before invoking. The Tauri command
 * boundary takes the same shape (see `kanban-app/src/commands.rs`).
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
  FullyQualifiedMoniker,
  LayerName,
  NavSnapshot,
  Rect,
  SegmentMoniker,
} from "@/types/spatial";
import { validateAndLogRect } from "@/lib/rect-validation";
import type { LayerScopeRegistry } from "@/lib/layer-scope-registry-context";

// ---------------------------------------------------------------------------
// Claim registry — per-FQM callbacks
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
 * `FullyQualifiedMoniker` gains or loses focus), this listener observes
 * every `focus-changed` payload in full. Used by integrations that need
 * to bridge spatial-focus into a peer store — most importantly,
 * `EntityFocusProvider`, which mirrors `next_fq` into its FQM-keyed
 * `FocusStore` so the legacy `focusedMonikerRef` API stays in sync with
 * spatial moves.
 *
 * Subscribers run synchronously on the same dispatch tick as per-FQM
 * claim listeners, so the work they do should be cheap. Calling back
 * into Tauri (e.g. dispatching `ui.setFocus` to forward the new scope
 * chain) is acceptable — the bridge already does it.
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
   * Register a focus-claim listener for `fq`. Returns the unsubscribe
   * function — call it on component unmount to remove the entry from the
   * registry. Replacing an existing entry with the same FQM is allowed
   * but rare in practice (each `<FocusScope>` mounts exactly one).
   */
  registerClaim: (
    fq: FullyQualifiedMoniker,
    listener: FocusClaimListener,
  ) => () => void;
  /** Read whether a listener exists for the FQM, primarily for tests. */
  hasClaim: (fq: FullyQualifiedMoniker) => boolean;
  /** Invoke `spatial_focus` for the given FQM in the current window. */
  focus: (fq: FullyQualifiedMoniker) => Promise<void>;
  /**
   * Invoke `spatial_focus` with `null` to clear focus in the current
   * window. Maps to `spatial_clear_focus` on the Rust side — the
   * window's focus slot is dropped and a `Some(prev) → None`
   * `focus-changed` event is emitted.
   */
  clearFocus: () => Promise<void>;
  /**
   * Invoke `spatial_register_scope` with the FQM-keyed kernel record:
   * canonical FQM, declared segment, viewport rect, owning layer FQM,
   * optional enclosing scope FQM, and per-direction overrides.
   *
   * Mirrors [`FocusScope`] on the Rust side — the single registered
   * primitive. Whether the scope plays a leaf or a navigable-container
   * role is determined at runtime by whether anything else is
   * registered under it; the wire shape is the same for both. Pass
   * `null` for `parentZone` when the scope is registered directly
   * under the layer root, and an empty object for `overrides` when
   * the scope has no per-direction special cases.
   *
   * `last_focused` is server-owned drill-out memory and is **not**
   * carried on the wire — registration is the React side's "this scope
   * just mounted" signal. The kernel preserves any existing
   * `last_focused` slot when a scope is re-registered (the
   * placeholder/real-mount swap), so the lack of a wire field is
   * correct rather than lossy.
   *
   * `sampledAtMs` is the `performance.now()` timestamp captured at the
   * exact callsite that called `getBoundingClientRect()` to produce
   * `rect`. The dev-mode validator uses it to detect stale rects (a
   * rect sampled before an unobserved scroll). Callers that just
   * sampled the rect should call `performance.now()` immediately
   * after; callers that don't have a meaningful timestamp may pass
   * `performance.now()` at the IPC boundary, but doing so makes the
   * staleness check a no-op. Optional — defaults to `performance.now()`
   * at the IPC boundary for legacy callers that have not yet been
   * threaded through.
   */
  registerScope: (
    fq: FullyQualifiedMoniker,
    segment: SegmentMoniker,
    rect: Rect,
    layerFq: FullyQualifiedMoniker,
    parentZone: FullyQualifiedMoniker | null,
    overrides: FocusOverrides,
    sampledAtMs?: number,
  ) => Promise<void>;
  /** Invoke `spatial_unregister_scope` for the given FQM. */
  unregisterScope: (fq: FullyQualifiedMoniker) => Promise<void>;
  /**
   * Invoke `spatial_update_rect` to refresh the bounding rect of a
   * registered scope. Call from a ResizeObserver on the underlying DOM
   * node; no-op on the Rust side if the FQM is unknown.
   *
   * See `registerScope` for the `sampledAtMs` contract.
   */
  updateRect: (
    fq: FullyQualifiedMoniker,
    rect: Rect,
    sampledAtMs?: number,
  ) => Promise<void>;
  /** Invoke `spatial_navigate` from `focusedFq` in `direction`. */
  navigate: (
    focusedFq: FullyQualifiedMoniker,
    direction: Direction,
  ) => Promise<void>;
  /**
   * Read the FQM currently focused in the active window, or `null` if
   * the window has no focus yet.
   *
   * Read on demand from the latest `focus-changed` event the provider
   * has observed. Safe to call from event handlers without re-rendering.
   */
  focusedFq: () => FullyQualifiedMoniker | null;
  /** Invoke `spatial_push_layer` for the given (fq, segment, name, parent). */
  pushLayer: (
    fq: FullyQualifiedMoniker,
    segment: SegmentMoniker,
    name: LayerName,
    parent: FullyQualifiedMoniker | null,
  ) => Promise<void>;
  /** Invoke `spatial_pop_layer` for the given FQM. */
  popLayer: (fq: FullyQualifiedMoniker) => Promise<void>;
  /**
   * Invoke `spatial_drill_in` to compute the FQM to focus when the
   * user drills *into* the scope at `fq`.
   *
   * Under the no-silent-dropout contract the kernel always returns an
   * FQM; the caller detects "no descent happened" by comparing the
   * result against `focusedFq`. Equality means the kernel had nothing
   * to descend into (leaf, empty zone, unknown FQM) and the caller
   * should fall through to the next behavior (e.g. inline edit on a
   * leaf with an editor). Inequality means focus should move to the
   * returned FQM.
   *
   * Mirrors `SpatialRegistry::drill_in` on the Rust side — purely a
   * registry query, no focus state mutation.
   */
  drillIn: (
    fq: FullyQualifiedMoniker,
    focusedFq: FullyQualifiedMoniker,
  ) => Promise<FullyQualifiedMoniker>;
  /**
   * Invoke `spatial_drill_out` to compute the FQM to focus when the
   * user drills *out of* the scope at `fq`.
   *
   * Under the no-silent-dropout contract the kernel always returns an
   * FQM; the caller compares the result against `focusedFq` to detect
   * "no zone-level drill happened" and falls through to `app.dismiss`
   * (close the topmost modal layer).
   *
   * Mirrors `SpatialRegistry::drill_out` on the Rust side.
   */
  drillOut: (
    fq: FullyQualifiedMoniker,
    focusedFq: FullyQualifiedMoniker,
  ) => Promise<FullyQualifiedMoniker>;
  /**
   * Register a per-layer scope registry under its `layerFq`.
   *
   * Called once on `<FocusLayer>` mount. The provider keeps a map of
   * registered layer registries so the snapshot-driven nav path can
   * locate the registry that owns a focused FQM at decision time.
   * Returns the unsubscribe function — call it on layer unmount.
   *
   * Replaces any prior entry under the same FQM (rare; the FQM is
   * deterministic per layer instance and a remount with a different
   * registry under the same FQM is the placeholder/real-mount swap).
   */
  registerLayerRegistry: (
    layerFq: FullyQualifiedMoniker,
    registry: LayerScopeRegistry,
  ) => () => void;
  /**
   * Subscribe to every `focus-changed` payload the provider observes.
   *
   * Returns an unsubscribe function — call it on unmount to remove the
   * entry. Used by integrations that need to bridge spatial focus into
   * a peer store: most notably `EntityFocusProvider`, which mirrors
   * `payload.next_fq` into its FQM-keyed `FocusStore` so
   * `useFocusedMonikerRef` and `useFocusedScope` stay in sync with
   * spatial moves.
   *
   * Subscribers fire synchronously alongside per-FQM claim listeners
   * on the same dispatch tick, so the callback should keep its work
   * cheap.
   */
  subscribeFocusChanged: (subscriber: FocusChangedSubscriber) => () => void;
}

const SpatialFocusContext = createContext<SpatialFocusActions | null>(null);

// ---------------------------------------------------------------------------
// Provider
// ---------------------------------------------------------------------------

/**
 * Provider for the spatial focus claim registry.
 *
 * Mounts a single global `focus-changed` listener; on every event, looks
 * up `payload.prev_fq` and `payload.next_fq` in the local registry and
 * dispatches `false` / `true` to whichever callbacks are registered.
 * Unmount removes the listener so a hot-reloaded provider does not leak
 * subscriptions.
 *
 * Callers should mount this once at the root of every Tauri window — each
 * window has its own React tree and therefore needs its own provider.
 */
export function SpatialFocusProvider({ children }: { children: ReactNode }) {
  // Registry of per-FQM callbacks. Held in a ref so registrations
  // do not cause re-renders — the only thing that re-renders on a focus
  // change is the `<FocusScope>` whose listener fires.
  const registryRef = useRef<Map<FullyQualifiedMoniker, FocusClaimListener>>(
    new Map(),
  );

  // Set of broad `focus-changed` subscribers. Held in a `Set` for O(1)
  // unsubscribe; held in a ref for the same reason as `registryRef`.
  const subscribersRef = useRef<Set<FocusChangedSubscriber>>(new Set());

  // Latest focused FQM from `focus-changed` events. Tracked in a ref
  // because the global keybinding handler needs to read it on every
  // keystroke without re-registering. Mirrors what `SpatialState` holds
  // on the Rust side, scoped to this window.
  const focusedFqRef = useRef<FullyQualifiedMoniker | null>(null);

  // Map of layer FQM → LayerScopeRegistry. Populated by `<FocusLayer>`
  // on mount, drained on unmount. The snapshot-driven nav path locates
  // the registry that owns the focused FQM by walking this map.
  const layerRegistriesRef = useRef<
    Map<FullyQualifiedMoniker, LayerScopeRegistry>
  >(new Map());

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
      if (payload.prev_fq !== null) {
        registry.get(payload.prev_fq)?.(false);
      }
      if (payload.next_fq !== null) {
        registry.get(payload.next_fq)?.(true);
      }
      // Record the latest focused FQM so `drillIn` / `drillOut` callers
      // can thread it through without consulting the entity-focus
      // moniker store.
      focusedFqRef.current = payload.next_fq;
      // Fan out the full payload to broad subscribers. Iteration walks
      // a snapshot so subscriber callbacks that unsubscribe themselves
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
      focusedFqRef,
      layerRegistriesRef,
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
  registryRef: React.MutableRefObject<
    Map<FullyQualifiedMoniker, FocusClaimListener>
  >,
  subscribersRef: React.MutableRefObject<Set<FocusChangedSubscriber>>,
  focusedFqRef: React.MutableRefObject<FullyQualifiedMoniker | null>,
  layerRegistriesRef: React.MutableRefObject<
    Map<FullyQualifiedMoniker, LayerScopeRegistry>
  >,
): SpatialFocusActions {
  const registerClaim: SpatialFocusActions["registerClaim"] = (
    fq,
    listener,
  ) => {
    registryRef.current.set(fq, listener);
    return () => {
      // Compare against the listener identity so we don't accidentally
      // remove a successor that registered under the same FQM after this
      // entry was replaced.
      const current = registryRef.current.get(fq);
      if (current === listener) {
        registryRef.current.delete(fq);
      }
    };
  };

  const hasClaim: SpatialFocusActions["hasClaim"] = (fq) =>
    registryRef.current.has(fq);

  const focus: SpatialFocusActions["focus"] = async (fq) => {
    const snapshot = buildSnapshotForFocused(layerRegistriesRef, fq);
    await invoke("spatial_focus", { fq, snapshot });
  };

  const clearFocus: SpatialFocusActions["clearFocus"] = async () => {
    await invoke("spatial_clear_focus");
  };

  const registerScope: SpatialFocusActions["registerScope"] = async (
    fq,
    segment,
    rect,
    layerFq,
    parentZone,
    overrides,
    sampledAtMs,
  ) => {
    // The staleness check is meaningful only when the caller threads
    // `performance.now()` through from the same callsite that ran
    // `getBoundingClientRect()`. A missing timestamp falls back to the
    // adapter-boundary `performance.now()` (the legacy behaviour) — that
    // makes the staleness check a no-op for legacy callers but keeps the
    // adapter usable from contexts that have no rect-sample timing.
    validateAndLogRect(
      "register_scope",
      fq,
      rect,
      sampledAtMs ?? performance.now(),
    );
    await invoke("spatial_register_scope", {
      fq,
      segment,
      rect,
      layerFq,
      parentZone,
      overrides,
    });
  };

  const unregisterScope: SpatialFocusActions["unregisterScope"] = async (
    fq,
  ) => {
    await invoke("spatial_unregister_scope", { fq });
  };

  const updateRect: SpatialFocusActions["updateRect"] = async (
    fq,
    rect,
    sampledAtMs,
  ) => {
    validateAndLogRect(
      "update_rect",
      fq,
      rect,
      sampledAtMs ?? performance.now(),
    );
    await invoke("spatial_update_rect", { fq, rect });
  };

  const navigate: SpatialFocusActions["navigate"] = async (
    focusedFq,
    direction,
  ) => {
    const snapshot = buildSnapshotForFocused(layerRegistriesRef, focusedFq);
    await invoke("spatial_navigate", { focusedFq, direction, snapshot });
  };

  const registerLayerRegistry: SpatialFocusActions["registerLayerRegistry"] = (
    layerFq,
    registry,
  ) => {
    layerRegistriesRef.current.set(layerFq, registry);
    return () => {
      const current = layerRegistriesRef.current.get(layerFq);
      if (current === registry) {
        layerRegistriesRef.current.delete(layerFq);
      }
    };
  };

  const pushLayer: SpatialFocusActions["pushLayer"] = async (
    fq,
    segment,
    name,
    parent,
  ) => {
    await invoke("spatial_push_layer", { fq, segment, name, parent });
  };

  const popLayer: SpatialFocusActions["popLayer"] = async (fq) => {
    const nextFq = await invoke<FullyQualifiedMoniker | null>(
      "spatial_pop_layer",
      { fq },
    );
    if (nextFq !== null && nextFq !== undefined) {
      const snapshot = buildSnapshotForFocused(layerRegistriesRef, nextFq);
      await invoke("spatial_focus", { fq: nextFq, snapshot });
    }
  };

  const drillIn: SpatialFocusActions["drillIn"] = async (fq, focusedFq) => {
    return await invoke<FullyQualifiedMoniker>("spatial_drill_in", {
      fq,
      focusedFq,
    });
  };

  const drillOut: SpatialFocusActions["drillOut"] = async (fq, focusedFq) => {
    return await invoke<FullyQualifiedMoniker>("spatial_drill_out", {
      fq,
      focusedFq,
    });
  };

  const focusedFq: SpatialFocusActions["focusedFq"] = () =>
    focusedFqRef.current;

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
    clearFocus,
    registerScope,
    unregisterScope,
    updateRect,
    navigate,
    pushLayer,
    popLayer,
    drillIn,
    drillOut,
    focusedFq,
    registerLayerRegistry,
    subscribeFocusChanged,
  };
}

/**
 * Locate the layer registry that owns `focusedFq` and build a snapshot
 * for its layer.
 *
 * Returns `undefined` when no registry contains the FQM — typically the
 * transient unmount window where the focused scope's registry has
 * already torn down. The IPC adapter falls back to its registry path on
 * `undefined`, so this is the documented "no snapshot available" signal.
 */
function buildSnapshotForFocused(
  layerRegistriesRef: React.MutableRefObject<
    Map<FullyQualifiedMoniker, LayerScopeRegistry>
  >,
  focusedFq: FullyQualifiedMoniker,
): NavSnapshot | undefined {
  for (const [layerFq, registry] of layerRegistriesRef.current) {
    if (registry.has(focusedFq)) {
      return registry.buildSnapshot(layerFq);
    }
  }
  return undefined;
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
 * stack (e.g. `<FocusScope>` mounted in a unit test without a
 * `<SpatialFocusProvider>` wrapper). The strict variant
 * `useSpatialFocusActions` is still the right choice anywhere the absence
 * of the provider is a contract violation rather than a tolerated state.
 */
export function useOptionalSpatialFocusActions(): SpatialFocusActions | null {
  return useContext(SpatialFocusContext);
}

/**
 * Subscribe to focus changes for a single `FullyQualifiedMoniker`.
 *
 * Calls `listener(true)` when the FQM gains focus and `listener(false)`
 * when it loses focus. The listener is registered on mount and cleaned
 * up on unmount; subsequent listener identities replace the previous
 * one (the registry stores at most one callback per FQM).
 *
 * `listener` is intentionally read through a ref, so callers can pass
 * an inline arrow function without paying for re-registration on every
 * render.
 */
export function useFocusClaim(
  fq: FullyQualifiedMoniker,
  listener: FocusClaimListener,
): void {
  const { registerClaim } = useSpatialFocusActions();
  const listenerRef = useRef(listener);
  listenerRef.current = listener;

  // Stable shim that delegates to whatever listenerRef points at. This
  // means we register exactly once per (provider, fq) pair — re-renders
  // that change the listener identity do not trigger re-registration.
  const stableListener = useCallback(
    (focused: boolean) => listenerRef.current(focused),
    [],
  );

  useEffect(() => {
    return registerClaim(fq, stableListener);
  }, [registerClaim, fq, stableListener]);
}
