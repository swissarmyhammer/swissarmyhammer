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
 *   - `spatial_push_layer(fq, segment, name, parent)` — appends a
 *     `LayerRecord` and remembers which layer FQMs were pushed in
 *     order.
 *   - `spatial_pop_layer(fq)` — drops the layer; downstream queries
 *     will not find it.
 *   - `spatial_register_scope` — appends a `RegistrationRecord` with
 *     the captured rect, layer FQM, parent zone, and overrides. After
 *     the single-primitive collapse every registered entry is a scope.
 *   - `spatial_unregister_scope(fq)` — drops the registration.
 *   - `spatial_update_rect(fq, rect)` — refreshes the live rect for an
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
 * carrying the focused FQM echoed back as `next_fq` (with
 * `prev_fq === next_fq === fromFq`). This mirrors the real Rust
 * kernel's emit-after-write behavior — `cardinal_cascade` returns the
 * focused FQM on stay-put, which the adapter wires through to a
 * focus-changed emit so the IPC trace always shows the end-state for
 * every nav dispatch. See `01KQAW97R9XTCNR1PJAWYSKBC7` for the
 * contract; see `swissarmyhammer-focus/src/navigate.rs` (fn
 * `cardinal_cascade`) for the kernel implementation. Without this
 * emit, tests that count focus-changed events or assert on a moniker
 * echo during a no-motion case would behave differently against the
 * simulator vs. the real kernel.
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
  FullyQualifiedMoniker,
  LayerName,
  SegmentMoniker,
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
  fq: FullyQualifiedMoniker;
  segment: SegmentMoniker;
  name: LayerName;
  parent: FullyQualifiedMoniker | null;
}

/** One zone or scope registration captured from `spatial_register_*`. */
export interface RegistrationRecord {
  kind: ShadowKind;
  fq: FullyQualifiedMoniker;
  segment: SegmentMoniker;
  layerFq: FullyQualifiedMoniker;
  parentZone: FullyQualifiedMoniker | null;
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
  /** All currently-pushed layers, keyed by FQM. */
  layers: Map<FullyQualifiedMoniker, LayerRecord>;
  /** All currently-registered zones/scopes, keyed by FQM. */
  registrations: Map<FullyQualifiedMoniker, RegistrationRecord>;
  /** Push + register events in the order they arrived (for ordering assertions). */
  history: HistoryEntry[];
  /** The currently focused FQM, mutated by `spatial_focus` / `spatial_navigate`. */
  currentFocus: { fq: FullyQualifiedMoniker | null };
  /**
   * Find the unique registration with the given segment.
   *
   * With path-based monikers (FQM identity), a `segment` is a *relative*
   * name — not a unique identifier. Two zones in different layers may
   * legitimately share the same segment (e.g. two open inspector panels
   * each containing `field:task:T1.title`). Returning the first match
   * silently would mask that ambiguity, so this method **throws** when
   * more than one live registration matches the segment. Callers that
   * need to enumerate matches should use {@link findBySegmentPrefix} or
   * iterate `simulator.registrations` directly.
   *
   * @returns the matching registration, or `undefined` when no
   *   registration matches.
   * @throws when more than one live registration shares the segment.
   */
  findBySegment(segment: string): RegistrationRecord | undefined;
  /** Find every registration whose segment has the given prefix. */
  findBySegmentPrefix(prefix: string): RegistrationRecord[];
  /** Find a registration by FQM. */
  findByFq(fq: FullyQualifiedMoniker): RegistrationRecord | undefined;
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
  const layers = new Map<FullyQualifiedMoniker, LayerRecord>();
  const registrations = new Map<FullyQualifiedMoniker, RegistrationRecord>();
  const history: HistoryEntry[] = [];
  const currentFocus: { fq: FullyQualifiedMoniker | null } = { fq: null };

  const emitFocusChanged = (
    prev: FullyQualifiedMoniker | null,
    next: FullyQualifiedMoniker | null,
    nextSegment: SegmentMoniker | null,
  ) => {
    const payload: FocusChangedPayload = {
      window_label: "main" as WindowLabel,
      prev_fq: prev,
      next_fq: next,
      next_segment: nextSegment,
    };
    queueMicrotask(() => {
      const handlers = listeners.get("focus-changed") ?? [];
      for (const h of handlers) h({ payload });
    });
  };

  const shadowFor = (rec: RegistrationRecord): ShadowEntry => ({
    kind: rec.kind,
    fq: rec.fq,
    segment: rec.segment,
    rect: rec.rect,
    layerFq: rec.layerFq,
    parentZone: rec.parentZone,
    overrides: rec.overrides,
  });

  mockInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
    const a = (args ?? {}) as Record<string, unknown>;

    if (cmd === "spatial_push_layer") {
      const record: LayerRecord = {
        fq: a.fq as FullyQualifiedMoniker,
        segment: a.segment as SegmentMoniker,
        name: a.name as LayerName,
        parent: (a.parent ?? null) as FullyQualifiedMoniker | null,
      };
      layers.set(record.fq, record);
      history.push({ type: "push_layer", record });
      return undefined;
    }
    if (cmd === "spatial_pop_layer") {
      // Mirrors the real kernel: returns the popped layer's
      // `last_focused` (or `null`) so the React side can issue the
      // follow-up `spatial_focus(next_fq, snapshot)`. The simulator
      // tracks layers without a `last_focused` slot, so the return is
      // `null` — the no-restoration case the round-trip handler
      // tolerates.
      layers.delete(a.fq as FullyQualifiedMoniker);
      return null;
    }
    if (cmd === "spatial_register_scope") {
      const record: RegistrationRecord = {
        kind: "scope",
        fq: a.fq as FullyQualifiedMoniker,
        segment: a.segment as SegmentMoniker,
        layerFq: a.layerFq as FullyQualifiedMoniker,
        parentZone: (a.parentZone ?? null) as FullyQualifiedMoniker | null,
        rect: rectFromWire(a.rect),
        overrides: (a.overrides ?? {}) as Record<string, unknown>,
      };
      registrations.set(record.fq, record);
      history.push({ type: "register", record });
      return undefined;
    }
    if (cmd === "spatial_unregister_scope") {
      registrations.delete(a.fq as FullyQualifiedMoniker);
      return undefined;
    }
    if (cmd === "spatial_update_rect") {
      const existing = registrations.get(a.fq as FullyQualifiedMoniker);
      if (existing) existing.rect = rectFromWire(a.rect);
      return undefined;
    }
    if (cmd === "spatial_focus") {
      const nextFq = a.fq as FullyQualifiedMoniker;
      const prev = currentFocus.fq;
      if (prev === nextFq) {
        // Idempotent — no event emitted, just like the real kernel's
        // "already focused" short-circuit.
        return undefined;
      }
      currentFocus.fq = nextFq;
      const entry = registrations.get(nextFq);
      emitFocusChanged(prev, nextFq, entry?.segment ?? null);
      return undefined;
    }
    if (cmd === "spatial_clear_focus") {
      // Explicit-clear counterpart of `spatial_focus`. Mirrors
      // `SpatialState::clear_focus`: when the window had focus, drop the
      // slot and emit a `Some(prev) → None` `focus-changed` event so the
      // React-side bridge can flip the entity-focus store back to
      // `null`. Idempotent when the window had no prior focus.
      const prev = currentFocus.fq;
      if (prev === null) return undefined;
      currentFocus.fq = null;
      emitFocusChanged(prev, null, null);
      return undefined;
    }
    if (cmd === "spatial_navigate") {
      const fromFq = a.focusedFq as FullyQualifiedMoniker;
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
      const shadowRegistry = new Map<FullyQualifiedMoniker, ShadowEntry>();
      for (const [k, v] of registrations) shadowRegistry.set(k, shadowFor(v));
      const result = navigateInShadow(
        shadowRegistry,
        fromFq,
        direction as CardinalDirection,
      );
      if (!result) {
        // No-silent-dropout contract: the real Rust kernel echoes the
        // focused FQM on a stay-put cascade (layer-root edge with no
        // peer in the chosen direction) and emits a focus-changed
        // event carrying that FQM. Mirror that emit here so tests that
        // count `focus-changed` events or assert on an FQM echo during
        // a no-motion case see the same IPC trace as production. See
        // `01KQAW97R9XTCNR1PJAWYSKBC7` for the contract; see
        // `swissarmyhammer-focus/src/navigate.rs` (fn
        // `cardinal_cascade`) for the kernel implementation.
        const focusedEntry = registrations.get(fromFq);
        if (focusedEntry) {
          emitFocusChanged(fromFq, fromFq, focusedEntry.segment);
        }
        return undefined;
      }
      currentFocus.fq = result.nextFq;
      emitFocusChanged(fromFq, result.nextFq, result.nextSegment);
      return undefined;
    }
    if (cmd === "spatial_drill_in" || cmd === "spatial_drill_out") {
      // No-silent-dropout contract: kernel echoes the focused FQM
      // when there's nothing to descend / drill-out into. Tests that
      // assert against drill-in / drill-out behavior should stub these
      // explicitly via the fallback.
      return (a.focusedFq ?? "") as FullyQualifiedMoniker;
    }
    return fallback(cmd, args);
  });

  return {
    layers,
    registrations,
    history,
    currentFocus,
    findBySegment(segment) {
      let match: RegistrationRecord | undefined;
      for (const r of registrations.values()) {
        if (r.segment !== segment) continue;
        if (match !== undefined) {
          // Surface ambiguity: with FQM identity, a segment is a relative
          // name and may be reused across layers. Returning the first match
          // silently would mask the duplicate and produce flaky tests when
          // a future fixture introduces same-segment zones in different
          // layers. Callers that legitimately want every match should use
          // `findBySegmentPrefix` or iterate `registrations` directly.
          throw new Error(
            `findBySegment("${segment}"): more than one live registration matches; ` +
              `segment is not unique under path-based monikers. ` +
              `Use findBySegmentPrefix or iterate registrations to disambiguate.`,
          );
        }
        match = r;
      }
      return match;
    },
    findBySegmentPrefix(prefix) {
      const out: RegistrationRecord[] = [];
      for (const r of registrations.values()) {
        if (r.segment.startsWith(prefix)) out.push(r);
      }
      return out;
    },
    findByFq(fq) {
      return registrations.get(fq);
    },
  };
}
