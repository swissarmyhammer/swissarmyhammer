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
 * registry. The kernel directs each `focus-changed` at a SINGLE window via
 * `emit_to(event.window_label, ...)`, but `emit_to` is NOT reliably confined
 * in a multi-window app — every window's global `listen` receives the event
 * regardless of target (the same Tauri behavior the `ui/request` responder
 * bus guards against). The provider's listener therefore filters by
 * `payload.window_label === getCurrentWindow().label` and ignores other
 * windows' events. Without that filter, another window's focus event
 * overwrote `focusedFqRef` (the value the kernel's `focus.current` /
 * `focus.geometry` pulls answer from) and a host-driven drill resolved the
 * WRONG window's focus when two windows showed the same board.
 *
 * Each window also roots its focus layer at its UNIQUE label
 * (`/<label>/window`, see `App.tsx`'s `WINDOW_ROOT_FQ`), so a card is
 * `/<label>/.../task:Z` — window-unique by construction, and the kernel
 * resolves the owning window from the window-rooted fq path rather than a
 * side field. Historically every window rooted at the bare `/window`, so a
 * card was `/window/.../task:Z` in *every* window showing that board and a
 * broadcast would light up the same card everywhere ("jump highlights all
 * windows"); the kernel clobber on the shared root then sent events to the
 * wrong window.
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
  useMemo,
  useRef,
  type ReactNode,
} from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
/**
 * Read this window's Tauri label via the static `getCurrentWindow()` import.
 *
 * This is the SAME reliable accessor `App.tsx` (`WINDOW_ROOT_FQ`),
 * `views-context.tsx`, and `perspective-context.tsx` use. The previous
 * `require("@tauri-apps/api/window")` form THREW in the Vite ESM production
 * bundle (`require` is undefined there) and silently fell back to `"main"`,
 * which mislabeled every `set focus` / drill commit's window and broke
 * cross-window focus routing. The kernel now derives the owning window from
 * the window-rooted fq path (the root segment IS the label), so this value is
 * no longer load-bearing for window resolution — but it must still be the real
 * label, not a "main" fallback, so nothing downstream is misled by a wrong
 * window arg. The static import is mocked in the spatial test harness exactly
 * as the other window-aware contexts are.
 */
function currentWindowLabel(): string {
  return getCurrentWindow().label;
}

/**
 * Read this window's Tauri label, or `null` when it is unknowable.
 *
 * `getCurrentWindow()` throws outside a real Tauri webview (no
 * `__TAURI_INTERNALS__`), which is the normal state for unit tests that do
 * not mock `@tauri-apps/api/window`. The `focus-changed` window guard uses
 * this safe form and — mirroring `handleUiRequest`'s own-window guard —
 * falls through to ACCEPTING the event when the label cannot be resolved,
 * so window-agnostic harnesses keep their pre-guard behavior.
 */
function currentWindowLabelOrNull(): string | null {
  try {
    return getCurrentWindow().label;
  } catch {
    return null;
  }
}
import {
  clearFocus as mcpClearFocus,
  drillIn as mcpDrillIn,
  drillOut as mcpDrillOut,
  loseFocus as mcpLoseFocus,
  navigateFocus as mcpNavigateFocus,
  popLayer as mcpPopLayer,
  pushLayer as mcpPushLayer,
  setFocus as mcpSetFocus,
} from "@/lib/focus-mcp";
import type {
  Direction,
  FocusChangedPayload,
  FullyQualifiedMoniker,
  LayerName,
  NavSnapshot,
  SegmentMoniker,
} from "@/types/spatial";
import type { LayerScopeRegistry } from "@/lib/layer-scope-registry-context";
import type { CommandDef } from "@/lib/command-scope";
import { CommandScopeProvider } from "@/lib/command-scope";
import { registerUiResponder } from "@/lib/ui-request-responder";

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
  /**
   * Enumerate every currently-registered scope in `layerFq`'s registry.
   *
   * Reads `getBoundingClientRect()` at call time for each entry's host
   * ref. Returns `[]` when the layer has no registered registry, or
   * when the registry exists but every entry's `ref.current` is null
   * (the brief unmount window where React has already cleared the
   * bound ref but the registry-deletion cleanup has not run yet).
   *
   * Mirrors the contract of `LayerScopeRegistry.buildSnapshot`: rects
   * are sampled fresh on every call, no cache; entries with a null
   * ref are skipped; zero-rect entries (host present but `display:
   * none` / detached layout) ARE included — the Jump-To overlay
   * filters zero-area rects when laying out pills.
   */
  enumerateScopesInLayer: (layerFq: FullyQualifiedMoniker) => Array<{
    fq: FullyQualifiedMoniker;
    rect: DOMRect;
    /** Enclosing scope FQM, for nearest-focusable-ancestor (tier) computation. */
    parentZone: FullyQualifiedMoniker | null;
    /** Whether this scope is a focus target (`showFocus`); zones are `false`. */
    focusable: boolean;
  }>;
  /**
   * Look up the layer FQM whose `LayerScopeRegistry` currently
   * contains `fq`. Returns `null` when no registry has the FQM (the
   * transient unmount window, or an unregistered FQM).
   *
   * Walks `layerRegistriesRef` in insertion order; in practice each
   * scope FQM lives in exactly one layer's registry by construction
   * so the first match is the only match.
   */
  layerFqOf: (fq: FullyQualifiedMoniker) => FullyQualifiedMoniker | null;
  /**
   * Read the FQM of the **topmost** (most-recently-pushed) layer, or
   * `null` when no layer is currently mounted.
   *
   * The layer stack is maintained as a side effect of `pushLayer` /
   * `popLayer` invocations. `pushLayer(fq)` appends `fq` to the
   * top-of-stack list, `popLayer(fq)` removes the matching entry
   * (whatever its position — pop is keyed by FQM, not by strict LIFO,
   * to tolerate the pop-not-on-top edge cases the kernel already
   * absorbs). The top is whichever entry was most recently appended
   * and not yet removed.
   *
   * Used by `nav.jump` (to enumerate scopes in the active layer) and
   * by `app.dismiss` (to decide which layer to close). When there is
   * exactly one layer (window) on the stack, `topLayerFq()` returns
   * the window's FQM; in that case `app.dismiss` is a no-op because
   * the window is the bottom-most layer and cannot be dismissed.
   */
  topLayerFq: () => FullyQualifiedMoniker | null;
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

  // Stack of layer FQMs in push order (oldest first, newest last). The
  // newest (top-of-stack) entry is the active modal layer — what
  // `nav.jump` enumerates against and what `app.dismiss` closes. Driven
  // off `pushLayer` / `popLayer` calls so it stays consistent with the
  // kernel's own stack on the Rust side. `popLayer` removes by FQM
  // identity (not strict LIFO) so out-of-order pop calls — which the
  // kernel itself tolerates — don't corrupt the React-side view.
  const layerStackRef = useRef<FullyQualifiedMoniker[]>([]);

  // Subscribe to the global `focus-changed` event exactly once for the
  // provider's lifetime. The cleanup is critical: an unmounted provider
  // that left its listener live would receive every focus-changed event
  // for the rest of the process and call into the now-empty registry,
  // leaking memory holding the closure references.
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    let cancelled = false;

    listen<FocusChangedPayload>("focus-changed", ({ payload }) => {
      // Window scoping: the kernel targets one window via `emit_to`, but a
      // multi-window app's global `listen` receives events for EVERY window
      // (same Tauri behavior the `ui/request` responder bus guards against).
      // Without this filter, another window's focus event overwrites
      // `focusedFqRef` — the value the kernel's `focus.current` /
      // `focus.geometry` pulls answer from — so a host-driven drill in THIS
      // window resolved the OTHER window's focus (the two-windows-on-one-board
      // "drill-in/Escape do nothing" bug). The event's `window_label` is
      // derived by the kernel from the window-rooted fq chain, so it is the
      // authoritative owner; only our own window's events apply here. When the
      // label is unknowable (window-agnostic test harness) fall through and
      // accept, matching `handleUiRequest`'s own-window guard.
      const ownLabel = currentWindowLabelOrNull();
      if (ownLabel !== null && payload.window_label !== ownLabel) {
        return;
      }
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

  // Register the host→UI geometry responders (Card F2). The focus kernel
  // PULLS live geometry and current focus from the webview on demand over
  // the F1 host→UI channel; this provider is the natural source because it
  // already owns the latest focused FQM (`focusedFqRef`) and the per-layer
  // scope registries (`layerRegistriesRef`). Both responders build their
  // answer ON DEMAND at request time — `focus.geometry` re-samples
  // `getBoundingClientRect` via `buildSnapshotForFocused`; nothing is
  // cached. (`focus.scopeChain` is registered by `EntityFocusProvider`,
  // which owns the focused entity's scope chain.) Registered in an effect
  // (not render) so the cleanup runs on unmount and a hot-reloaded provider
  // does not leak a stale responder.
  useEffect(() => {
    const unregisterGeometry = registerUiResponder("focus.geometry", () => {
      const focusedFq = focusedFqRef.current;
      if (focusedFq === null) return null;
      return buildSnapshotForFocused(layerRegistriesRef, focusedFq) ?? null;
    });
    const unregisterCurrent = registerUiResponder(
      "focus.current",
      () => focusedFqRef.current,
    );
    return () => {
      unregisterGeometry();
      unregisterCurrent();
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
      layerStackRef,
    );
  }

  // Register `nav.focus` at this level so any descendant `<FocusScope>`'s
  // click handler can dispatch it without requiring an
  // `<EntityFocusProvider>` ancestor. The execute closure calls
  // `actions.focus(fq)` directly — the kernel-facing primitive that
  // dispatches `spatial_focus` IPC. When `<EntityFocusProvider>` is
  // mounted as a descendant, it registers an inner `nav.focus` that
  // shadows this one (commands are scope-chained, inner wins). That
  // inner version routes through `setFocus`, which is identity-equal to
  // calling `spatial.focus(fq)` in production but also covers the
  // entity-focus test-harness fallback (direct store mutation when no
  // spatial provider is mounted, which doesn't apply here since this
  // closure only runs when `<SpatialFocusProvider>` is mounted).
  //
  // Per card `01KR7CDEFWWVF4WH0BCHE8Y21J`'s modal-layer model: every
  // non-null focus claim flows through `nav.focus`. Components do not
  // call `spatial.focus(fq)` or `setFocus(fq)` directly — they dispatch
  // `nav.focus({ args: { fq } })`. Cross-cutting concerns (telemetry,
  // animations, scroll-on-focus) hang off this one closure.
  const navFocusCommands = useMemo<readonly CommandDef[]>(
    () => [buildSpatialNavFocusCommand(actionsRef.current!)],
    [],
  );

  return (
    <SpatialFocusContext.Provider value={actionsRef.current}>
      <CommandScopeProvider commands={navFocusCommands}>
        {children}
      </CommandScopeProvider>
    </SpatialFocusContext.Provider>
  );
}

/**
 * Build the `nav.focus` command for the spatial-focus level — the
 * kernel-facing focus claim path.
 *
 * The execute closure reads `args.fq` from the dispatch options and
 * calls `actions.focus(fq)`, which composes a snapshot from the
 * per-layer registry and dispatches `spatial_focus` IPC. The kernel
 * emits `focus-changed` back to React; the registered claim listeners
 * for `prev_fq` and `next_fq` fire to update each scope's visible
 * focus indicator.
 *
 * When an `<EntityFocusProvider>` descendant registers its own
 * `nav.focus`, the inner registration shadows this one. The inner
 * version goes through `useFocusActions().setFocus(fq)`, which has the
 * same production behavior (calls `spatial.focus`) but also handles
 * the test-harness no-spatial-provider fallback. This dual
 * registration means every test setup that mounts at least one of the
 * two providers gets `nav.focus` resolution without modifying the test
 * harness.
 */
function buildSpatialNavFocusCommand(actions: SpatialFocusActions): CommandDef {
  return {
    id: "nav.focus",
    name: "Focus Scope",
    execute: (opts) => {
      const fq = opts?.args?.fq;
      if (typeof fq !== "string") {
        // Defensive: a dispatch without `args.fq` is a programming
        // error, not a user-visible state. Log so dev mode catches the
        // missing arg, then no-op so the rest of the command pipeline
        // keeps running.
        console.error("[nav.focus] missing or non-string args.fq", opts?.args);
        return;
      }
      void actions.focus(fq as FullyQualifiedMoniker).catch((err) => {
        console.error("[nav.focus] spatial.focus failed", err);
      });
    },
  };
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
  layerStackRef: React.MutableRefObject<FullyQualifiedMoniker[]>,
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
    await mcpSetFocus(fq, snapshot, currentWindowLabel());
  };

  const clearFocus: SpatialFocusActions["clearFocus"] = async () => {
    // MCP wire has no ambient `tauri::Window` — pass this window's
    // label explicitly (the legacy `spatial_clear_focus` Tauri command
    // derived it from the `Window` param on the host side).
    await mcpClearFocus(currentWindowLabel());
  };

  const navigate: SpatialFocusActions["navigate"] = async (
    focusedFq,
    direction,
  ) => {
    const snapshot = buildSnapshotForFocused(layerRegistriesRef, focusedFq);
    await mcpNavigateFocus(
      focusedFq,
      direction,
      snapshot,
      currentWindowLabel(),
    );
  };

  const registerLayerRegistry: SpatialFocusActions["registerLayerRegistry"] = (
    layerFq,
    registry,
  ) => {
    layerRegistriesRef.current.set(layerFq, registry);
    // Subscribe to scope deletions so we can detect the focused-scope
    // unmount case and dispatch `spatial_focus_lost` to the kernel.
    // The deletion listener fires AFTER the entry leaves the map, so
    // `buildSnapshot()` called below correctly excludes the lost FQM.
    const unsubscribeDeleted = registry.onDeleted((fq, entry) => {
      if (focusedFqRef.current !== fq) return;
      // Read the cached rect rather than calling
      // `getBoundingClientRect()` on `entry.ref.current`. By the time
      // this listener runs from a `<FocusScope>`'s `useEffect` cleanup,
      // React has already invoked the scope's bound `setRef(null)` in
      // the commit phase, so `entry.ref.current` is `null` and a fresh
      // sample would skip the IPC. The cached rect is seeded at mount
      // (alongside `LayerScopeRegistry.add`) and refreshed immediately
      // before unmount (via the scope's `useLayoutEffect` cleanup,
      // which runs while the ref is still attached), so it reflects
      // live geometry at the moment of unmount.
      const lostRect = entry.lastKnownRect;
      // No rect ever sampled (the scope unmounted in the same tick it
      // was registered) — skip the IPC entirely. There is no fallback
      // path in the kernel: without a rect the snapshot-driven
      // resolver has nothing to rank against, so the focused FQM stays
      // until something else moves it.
      if (lostRect === null) return;
      const snapshot = registry.buildSnapshot(layerFq);
      mcpLoseFocus({
        focusedFq: fq,
        lostParentZone: entry.parentZone,
        lostLayerFq: layerFq,
        lostRect,
        snapshot,
      }).catch((err) => console.error("[spatial_focus_lost] failed", err));
    });
    return () => {
      unsubscribeDeleted();
      const current = layerRegistriesRef.current.get(layerFq);
      if (current === registry) {
        layerRegistriesRef.current.delete(layerFq);
      }
    };
  };

  // Serialize the kernel layer ops (push / pop). Each op is dispatched
  // only after the previous one has FULLY completed on the host, so the
  // kernel registry observes layer mutations in React lifecycle order.
  //
  // Why this matters: the host handles each MCP call as an independent
  // async task (contending on the per-board platform lock), so two calls
  // issued back-to-back can complete out of order. React StrictMode's
  // double-invoked effects make `<FocusLayer>` fire push(fq) → pop(fq) →
  // push(fq) microseconds apart; when the cleanup pop was processed AFTER
  // the remount push, the window-root layer was deleted permanently and
  // every later focus commit in that window dropped with "focus snapshot
  // names an unregistered layer" — the two-windows-on-one-board "no focus
  // markers in the second window" bug. Window-unique layer FQs
  // (`/<label>/window`) made the lost push fatal: under the old shared
  // `/window` root the sibling window's surviving push masked it.
  //
  // The chain swallows rejections so one failed op never wedges the
  // queue; each caller still observes its own op's rejection.
  let layerOpChain: Promise<unknown> = Promise.resolve();
  const enqueueLayerOp = <T,>(op: () => Promise<T>): Promise<T> => {
    const run = layerOpChain.then(op, op);
    layerOpChain = run.catch(() => undefined);
    return run;
  };

  const pushLayer: SpatialFocusActions["pushLayer"] = async (
    fq,
    segment,
    name,
    parent,
  ) => {
    // Append to the React-side layer stack BEFORE the IPC fires. The
    // kernel push is async; if a sibling caller reads `topLayerFq()`
    // between IPC dispatch and resolve, they should see the new top.
    // The kernel does the same — its stack mutates synchronously inside
    // the command handler before the IPC reply.
    //
    // Idempotent: if the same FQM appears twice in a row (which would
    // already be a kernel-side error), keep only one entry. The kernel
    // is the authority on duplicates; we just avoid corrupting our own
    // top-of-stack view.
    const stack = layerStackRef.current;
    const existing = stack.indexOf(fq);
    if (existing !== -1) stack.splice(existing, 1);
    stack.push(fq);
    // MCP wire has no ambient `tauri::Window` — pass this window's
    // label explicitly (the legacy `spatial_push_layer` Tauri command
    // derived it from the `Window` param on the host side).
    await enqueueLayerOp(() =>
      mcpPushLayer({
        fq,
        segment,
        name,
        parent,
        window: currentWindowLabel(),
      }),
    );
  };

  const popLayer: SpatialFocusActions["popLayer"] = async (fq) => {
    // Remove from the React-side stack by FQM identity. Pop is keyed
    // by FQM (not by strict LIFO) to mirror the kernel, which also
    // tolerates pop-not-on-top (e.g. when a parent layer's React
    // subtree unmounts before a descendant's effect cleanup runs).
    const stack = layerStackRef.current;
    const idx = stack.indexOf(fq);
    if (idx !== -1) stack.splice(idx, 1);
    const nextFq = await enqueueLayerOp(() => mcpPopLayer(fq));
    if (nextFq !== null && nextFq !== undefined) {
      const snapshot = buildSnapshotForFocused(layerRegistriesRef, nextFq);
      await mcpSetFocus(nextFq, snapshot, currentWindowLabel());
    }
  };

  const topLayerFq: SpatialFocusActions["topLayerFq"] = () => {
    const stack = layerStackRef.current;
    return stack.length === 0 ? null : stack[stack.length - 1];
  };

  const drillIn: SpatialFocusActions["drillIn"] = async (fq, focusedFq) => {
    const snapshot = buildSnapshotForFocused(layerRegistriesRef, focusedFq);
    return await mcpDrillIn(fq, focusedFq, snapshot, currentWindowLabel());
  };

  const drillOut: SpatialFocusActions["drillOut"] = async (fq, focusedFq) => {
    const snapshot = buildSnapshotForFocused(layerRegistriesRef, focusedFq);
    return await mcpDrillOut(fq, focusedFq, snapshot, currentWindowLabel());
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

  const enumerateScopesInLayer: SpatialFocusActions["enumerateScopesInLayer"] =
    (layerFq) => {
      const registry = layerRegistriesRef.current.get(layerFq);
      if (registry === undefined) return [];
      const out: Array<{
        fq: FullyQualifiedMoniker;
        rect: DOMRect;
        parentZone: FullyQualifiedMoniker | null;
        focusable: boolean;
      }> = [];
      for (const [fq, entry] of registry.entries()) {
        const node = entry.ref.current;
        if (node === null) continue;
        out.push({
          fq,
          rect: node.getBoundingClientRect(),
          parentZone: entry.parentZone,
          focusable: entry.showFocus ?? true,
        });
      }
      return out;
    };

  const layerFqOf: SpatialFocusActions["layerFqOf"] = (fq) => {
    for (const [layerFq, registry] of layerRegistriesRef.current) {
      if (registry.has(fq)) return layerFq;
    }
    return null;
  };

  return {
    registerClaim,
    hasClaim,
    focus,
    clearFocus,
    navigate,
    pushLayer,
    popLayer,
    drillIn,
    drillOut,
    focusedFq,
    registerLayerRegistry,
    subscribeFocusChanged,
    enumerateScopesInLayer,
    layerFqOf,
    topLayerFq,
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
