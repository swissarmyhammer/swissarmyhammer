/**
 * Kernel simulator — an in-process JS shadow registry that records every
 * spatial-nav IPC the React tree fires and answers `spatial_navigate` /
 * `spatial_focus` deterministically against the captured state.
 *
 * # Why this module exists
 *
 * Some inspector-shape tests need to assert the *registration tree* the
 * production code emits (which layer was pushed, which zones registered
 * with which `parent_zone`, whether any scopes registered for the
 * inspector entity moniker) AND drive realistic nav round-trips against
 * the kernel's beam-search cascade. The existing
 * `src/test/spatial-shadow-registry.ts` already provides a bigger
 * harness for end-to-end tests; this module is a smaller, focused
 * primitive aimed at single-component shape-snapshot + boundary-nav
 * tests.
 *
 * # What it records
 *
 * The simulator captures the full payload of every IPC call shaped by
 * `lib/spatial-focus-context.tsx`'s actions bag:
 *
 *   - `spatial_push_layer(key, name, parent)` — appends a `LayerRecord`
 *     and remembers which layer keys were pushed in order.
 *   - `spatial_pop_layer(key)` — drops the layer; downstream queries
 *     will not find it.
 *   - `spatial_register_zone` / `spatial_register_scope` — appends a
 *     `RegistrationRecord` with the captured rect, layer key, parent
 *     zone, kind ("zone" or "scope"), and overrides.
 *   - `spatial_unregister_scope(key)` — drops the registration.
 *   - `spatial_update_rect(key, rect)` — refreshes the live rect for an
 *     existing registration (the registration order remains intact).
 *
 * # Cascade simulation tradeoff
 *
 * Wasm bindings to the real Rust kernel are not available in the
 * browser test environment, so the simulator implements the cascade in
 * TypeScript (delegating to the same algorithm
 * `swissarmyhammer-focus/src/navigate.rs` runs). The Rust kernel
 * remains the source of truth for the algorithm; the JS port mirrors
 * the unified two-level cascade plus drill-out fallback. The
 * end-to-end `src/test/spatial-shadow-registry.ts` carries the canonical
 * port; this module re-uses its `navigateInShadow` to keep both files
 * in lock-step when the kernel rules change.
 *
 * # No-silent-dropout emit contract
 *
 * On a stay-put cascade (layer-root edge with no peer in the chosen
 * direction), the simulator emits a synthetic `focus-changed` event
 * carrying the focused moniker echoed back as `next_moniker` (with
 * `prev_key === next_key === fromKey`). This mirrors the real Rust
 * kernel's emit-after-write behavior — `cardinal_cascade` returns
 * `focused_moniker.clone()` on stay-put, which the adapter wires
 * through to a focus-changed emit so the IPC trace always shows the
 * end-state for every nav dispatch. See
 * `01KQAW97R9XTCNR1PJAWYSKBC7` for the contract; see
 * `swissarmyhammer-focus/src/navigate.rs` (fn `cardinal_cascade`) for
 * the kernel implementation. Without this emit, tests that count
 * focus-changed events or assert on a moniker echo during a no-motion
 * case would behave differently against the simulator vs. the real
 * kernel.
 *
 * # Mount-ordering history
 *
 * The simulator records every `spatial_push_layer` and
 * `spatial_register_*` call in the order they fire. Tests that need to
 * assert "layer push happened before any field zone registered" can
 * read `simulator.history` — the entries arrive in IPC order, so a
 * straightforward index lookup answers the question.
 */

import { vi } from "vitest";
import type {
  FocusChangedPayload,
  LayerKey,
  LayerName,
  SpatialKey,
  WindowLabel,
} from "@/types/spatial";
import {
  navigateInShadow,
  rectFromWire,
  type Direction as CardinalDirection,
  type RectLike,
  type ShadowEntry,
  type ShadowKind,
} from "@/test/spatial-shadow-registry";

// ---------------------------------------------------------------------------
// Captured records
// ---------------------------------------------------------------------------

/** One layer push captured from `spatial_push_layer`. */
export interface LayerRecord {
  key: LayerKey;
  name: LayerName;
  parent: LayerKey | null;
}

/** One zone or scope registration captured from `spatial_register_*`. */
export interface RegistrationRecord {
  kind: ShadowKind;
  key: SpatialKey;
  moniker: string;
  layerKey: LayerKey;
  parentZone: SpatialKey | null;
  rect: RectLike;
  overrides: Record<string, unknown>;
}

/** Unified history entry — mirrors IPC arrival order. */
export type HistoryEntry =
  | { type: "push_layer"; record: LayerRecord }
  | { type: "register"; record: RegistrationRecord };

// ---------------------------------------------------------------------------
// Simulator API
// ---------------------------------------------------------------------------

/** The handle a test holds onto for the lifetime of one render. */
export interface KernelSimulator {
  /** All currently-pushed layers, keyed by `LayerKey`. */
  layers: Map<LayerKey, LayerRecord>;
  /** All currently-registered zones/scopes, keyed by `SpatialKey`. */
  registrations: Map<SpatialKey, RegistrationRecord>;
  /** Push + register events in the order they arrived (for ordering assertions). */
  history: HistoryEntry[];
  /** The currently focused `SpatialKey`, mutated by `spatial_focus` / `spatial_navigate`. */
  currentFocus: { key: SpatialKey | null };
  /** Find a registration by moniker. Returns the most-recent live entry. */
  findByMoniker(moniker: string): RegistrationRecord | undefined;
  /** Find every registration whose moniker has the given prefix. */
  findByMonikerPrefix(prefix: string): RegistrationRecord[];
}

/**
 * Bootstrap-invoke handler signature. Returns a value to satisfy a Tauri
 * command that the simulator does not own (e.g. schema fetches). Returns
 * `undefined` to fall through to the simulator's default response.
 */
export type FallbackInvoke = (
  cmd: string,
  args?: unknown,
) => Promise<unknown> | unknown;

/**
 * Install a `mockInvoke` implementation that records spatial-nav IPCs
 * into a fresh simulator and routes `spatial_navigate` / `spatial_focus`
 * through the in-test cascade. Every other IPC falls through to
 * `fallback`.
 *
 * The `fireFocusChanged` callback the simulator hands back drives a
 * synthetic `focus-changed` payload through whatever listener the
 * spatial-focus provider installed via the supplied `mockListen`.
 *
 * @param mockInvoke - The vitest spy installed on
 *   `@tauri-apps/api/core::invoke`.
 * @param listeners - The map of `event-name → listener[]` populated by
 *   the spying `listen` mock.
 * @param fallback - Handler for non-spatial IPCs (schema fetches, UI
 *   state snapshots, etc.). Defaults to returning `undefined`.
 */
export function installKernelSimulator(
  mockInvoke: ReturnType<typeof vi.fn>,
  listeners: Map<string, Array<(event: { payload: unknown }) => void>>,
  fallback: FallbackInvoke = async () => undefined,
): KernelSimulator {
  const layers = new Map<LayerKey, LayerRecord>();
  const registrations = new Map<SpatialKey, RegistrationRecord>();
  const history: HistoryEntry[] = [];
  const currentFocus: { key: SpatialKey | null } = { key: null };

  const emitFocusChanged = (
    prev: SpatialKey | null,
    next: SpatialKey | null,
    nextMoniker: string | null,
  ) => {
    const payload: FocusChangedPayload = {
      window_label: "main" as WindowLabel,
      prev_key: prev,
      next_key: next,
      next_moniker: nextMoniker as FocusChangedPayload["next_moniker"],
    };
    queueMicrotask(() => {
      const handlers = listeners.get("focus-changed") ?? [];
      for (const h of handlers) h({ payload });
    });
  };

  const shadowFor = (rec: RegistrationRecord): ShadowEntry => ({
    kind: rec.kind,
    key: rec.key,
    moniker: rec.moniker,
    rect: rec.rect,
    layerKey: rec.layerKey,
    parentZone: rec.parentZone,
    overrides: rec.overrides,
  });

  mockInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
    const a = (args ?? {}) as Record<string, unknown>;

    if (cmd === "spatial_push_layer") {
      const record: LayerRecord = {
        key: a.key as LayerKey,
        name: a.name as LayerName,
        parent: (a.parent ?? null) as LayerKey | null,
      };
      layers.set(record.key, record);
      history.push({ type: "push_layer", record });
      return undefined;
    }
    if (cmd === "spatial_pop_layer") {
      layers.delete(a.key as LayerKey);
      return undefined;
    }
    if (cmd === "spatial_register_zone" || cmd === "spatial_register_scope") {
      const record: RegistrationRecord = {
        kind: cmd === "spatial_register_zone" ? "zone" : "scope",
        key: a.key as SpatialKey,
        moniker: String(a.moniker),
        layerKey: a.layerKey as LayerKey,
        parentZone: (a.parentZone ?? null) as SpatialKey | null,
        rect: rectFromWire(a.rect),
        overrides: (a.overrides ?? {}) as Record<string, unknown>,
      };
      registrations.set(record.key, record);
      history.push({ type: "register", record });
      return undefined;
    }
    if (cmd === "spatial_unregister_scope") {
      registrations.delete(a.key as SpatialKey);
      return undefined;
    }
    if (cmd === "spatial_update_rect") {
      const existing = registrations.get(a.key as SpatialKey);
      if (existing) existing.rect = rectFromWire(a.rect);
      return undefined;
    }
    if (cmd === "spatial_focus") {
      const nextKey = a.key as SpatialKey;
      const prev = currentFocus.key;
      currentFocus.key = nextKey;
      const entry = registrations.get(nextKey);
      emitFocusChanged(prev, nextKey, entry?.moniker ?? null);
      return undefined;
    }
    if (cmd === "spatial_focus_by_moniker") {
      // Moniker-keyed counterpart of `spatial_focus`. The kernel's
      // `SpatialState::focus_by_moniker` resolves the moniker via
      // `SpatialRegistry::find_by_moniker`, advances the per-window
      // focus map, and emits `focus-changed`. Mirror that here so
      // tests pinning the kernel-projection invariant see the same
      // wire shape they would in production.
      //
      // Under the no-silent-dropout contract, an unknown moniker
      // produces an error response (the kernel logs `tracing::error!`
      // and the Tauri adapter returns `Err(_)` to the React caller).
      // The React `setFocus` dispatch surfaces that as
      // `console.error`, which kernel-projection tests assert on.
      const moniker = String(a.moniker);
      let resolved: RegistrationRecord | undefined;
      for (const r of registrations.values()) {
        if (r.moniker === moniker) {
          resolved = r;
          break;
        }
      }
      if (!resolved) {
        throw new Error(`unknown moniker: ${moniker}`);
      }
      const prev = currentFocus.key;
      if (prev === resolved.key) {
        // Idempotent — no event emitted, just like the real kernel's
        // "already focused" short-circuit.
        return undefined;
      }
      currentFocus.key = resolved.key;
      emitFocusChanged(prev, resolved.key, resolved.moniker);
      return undefined;
    }
    if (cmd === "spatial_clear_focus") {
      // Explicit-clear counterpart of `spatial_focus_by_moniker`.
      // Mirrors `SpatialState::clear_focus`: when the window had
      // focus, drop the slot and emit a `Some(prev) → None`
      // `focus-changed` event so the React-side bridge can flip the
      // entity-focus store back to `null`. Idempotent when the
      // window had no prior focus.
      const prev = currentFocus.key;
      if (prev === null) {
        return undefined;
      }
      currentFocus.key = null;
      emitFocusChanged(prev, null, null);
      return undefined;
    }
    if (cmd === "spatial_navigate") {
      const fromKey = a.key as SpatialKey;
      const direction = a.direction as string;
      // The shadow navigator only handles cardinal directions
      // (up/down/left/right). The kernel's `first` / `last` directions
      // are not modelled here — boundary tests use up / down on the
      // first / last field rather than `first` / `last`.
      if (
        direction !== "up" &&
        direction !== "down" &&
        direction !== "left" &&
        direction !== "right"
      ) {
        return undefined;
      }
      const shadowRegistry = new Map<SpatialKey, ShadowEntry>();
      for (const [k, v] of registrations) shadowRegistry.set(k, shadowFor(v));
      const result = navigateInShadow(
        shadowRegistry,
        fromKey,
        direction as CardinalDirection,
      );
      if (!result) {
        // No-silent-dropout contract: the real Rust kernel echoes the
        // focused moniker on a stay-put cascade (layer-root edge with
        // no peer in the chosen direction) and emits a focus-changed
        // event carrying that moniker. Mirror that emit here so tests
        // that count `focus-changed` events or assert on a moniker
        // echo during a no-motion case see the same IPC trace as
        // production. See `01KQAW97R9XTCNR1PJAWYSKBC7` for the
        // contract; see `swissarmyhammer-focus/src/navigate.rs` (fn
        // `cardinal_cascade`) for the kernel implementation.
        const focusedEntry = registrations.get(fromKey);
        if (focusedEntry) {
          emitFocusChanged(fromKey, fromKey, focusedEntry.moniker);
        }
        return undefined;
      }
      currentFocus.key = result.nextKey;
      emitFocusChanged(fromKey, result.nextKey, result.nextMoniker);
      return undefined;
    }
    if (cmd === "spatial_drill_in" || cmd === "spatial_drill_out") {
      // No-silent-dropout contract: kernel echoes the focused moniker
      // when there's nothing to descend / drill-out into. Tests that
      // assert against drill-in / drill-out behavior should stub these
      // explicitly via the fallback.
      return (a.focusedMoniker ?? "") as string;
    }
    return fallback(cmd, args);
  });

  return {
    layers,
    registrations,
    history,
    currentFocus,
    findByMoniker(moniker) {
      for (const r of registrations.values()) {
        if (r.moniker === moniker) return r;
      }
      return undefined;
    },
    findByMonikerPrefix(prefix) {
      const out: RegistrationRecord[] = [];
      for (const r of registrations.values()) {
        if (r.moniker.startsWith(prefix)) out.push(r);
      }
      return out;
    },
  };
}
