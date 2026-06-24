/**
 * Shared spatial-kernel mock harness for the spatial browser/test suite.
 *
 * The spatial-kernel echo contract — `spatial_register_scope` /
 * `spatial_unregister_scope` / `spatial_focus` / `spatial_clear_focus` /
 * `spatial_drill_in` / `spatial_drill_out` — was copy-pasted near-verbatim
 * across ~13 sibling test files (card `01KV6250AH0DPRMG9SJ6A45SPW`). Every
 * copy maintained the same `monikerToKey` (segment → fq) projection,
 * advanced the same `currentFocusKey` slot, and emitted the same queued
 * `focus-changed` event. This module is the single source of truth for that
 * contract so a future change to the kernel-echo behavior propagates to
 * every test at once.
 *
 * The per-file ENTITY / UI-state IPC answers (entity lists, schemas,
 * keymap mode) are NOT part of this harness — they diverge by test and stay
 * local. A caller layers its own answers in front of {@link
 * SpatialKernelMock.handleSpatialCommand}, which returns {@link UNHANDLED}
 * for any command it does not own so the caller can fall through.
 *
 * The listener container itself stays local too: callers vary between a
 * `Map<string, cb[]>` and a single-callback `Record<string, cb>`. Rather
 * than pin one shape, the factory takes an {@link FocusChangedEmit}
 * callback the caller wires to its own container — the harness owns the
 * payload shape and the `queueMicrotask` timing, the caller owns delivery.
 */

/**
 * Sentinel returned by {@link SpatialKernelMock.handleSpatialCommand} when
 * the command it was handed isn't a spatial-kernel command it owns. Lets a
 * caller's default-invoke dispatcher distinguish "handler matched and
 * intentionally returned `undefined`" from "handler doesn't apply, keep
 * looking".
 */
import { FOCUS_CHANGED_EVENT } from "@/lib/mcp-notifications";

export const UNHANDLED = Symbol("unhandled");

/** The `focus-changed` event the kernel emits, in the loose mock wire shape. */
export interface FocusChangedEvent {
  payload: {
    window_label: string;
    prev_fq: string | null;
    next_fq: string | null;
    next_segment: string | null;
  };
}

/**
 * Deliver a synthesized `focus-changed` event to the caller's listeners.
 * Wired by the caller to its own listener container (Map-of-arrays or
 * single-callback record) so the harness need not know the shape.
 */
export type FocusChangedEmit = (event: FocusChangedEvent) => void;

/** Mutable focus slot — the kernel's per-window focused-fq projection. */
export interface FocusSlot {
  key: string | null;
}

/** Options for {@link makeSpatialKernelMock}. */
export interface SpatialKernelMockOptions {
  /** Deliver a queued `focus-changed` event to the caller's listeners. */
  emit: FocusChangedEmit;
  /**
   * Window label stamped on emitted events. Defaults to `"main"` — the
   * label every inline copy hardcoded.
   */
  windowLabel?: string;
}

/** The mutable state + handler a {@link makeSpatialKernelMock} call exposes. */
export interface SpatialKernelMock {
  /**
   * Segment → fully-qualified-moniker projection, maintained by
   * register/unregister. Exposed so tests can read it (e.g. resolve a
   * leaf's fq) or seed it directly, matching the inline copies.
   */
  readonly monikerToKey: Map<string, string>;
  /**
   * Mutable focused-fq slot. Exposed so tests can read it or seed a
   * starting focus directly, matching the inline copies.
   */
  readonly currentFocusKey: FocusSlot;
  /**
   * Per-test `spatial_drill_in` override keyed by the drilled fq. A
   * non-null value means "drill walked to this child"; a `null` value
   * means "stay put — echo the focused moniker"; an absent key means "use
   * the default echo".
   */
  readonly drillInResponses: Map<string, string | null>;
  /**
   * Answer a spatial-kernel command with the no-silent-dropout echo
   * contract, mirroring focus effects through the supplied `emit`. Returns
   * {@link UNHANDLED} when `command` is not a spatial command so the caller
   * can fall through to its own handlers.
   */
  handleSpatialCommand(command: string, commandArgs?: unknown): unknown;
  /** Clear the projection, focus slot, and drill overrides (call in `beforeEach`). */
  reset(): void;
}

/**
 * Build a spatial-kernel mock: the `monikerToKey` projection, the
 * `currentFocusKey` slot, the `drillInResponses` override map, and a
 * `handleSpatialCommand` dispatcher implementing the production kernel's
 * echo contract.
 *
 * @param options - The {@link FocusChangedEmit} sink and optional window label.
 * @returns The mutable state plus the `handleSpatialCommand` dispatcher.
 */
export function makeSpatialKernelMock(
  options: SpatialKernelMockOptions,
): SpatialKernelMock {
  const { emit, windowLabel = "main" } = options;
  const monikerToKey = new Map<string, string>();
  const currentFocusKey: FocusSlot = { key: null };
  const drillInResponses = new Map<string, string | null>();

  /** Resolve the registered segment for a fully-qualified moniker, if any. */
  function segmentForFq(fq: string | null): string | null {
    if (fq === null) return null;
    for (const [segment, key] of monikerToKey.entries()) {
      if (key === fq) return segment;
    }
    return null;
  }

  function handleSpatialCommand(command: string, commandArgs?: unknown): unknown {
    if (command === "spatial_register_scope") {
      const a = (commandArgs ?? {}) as { fq?: string; segment?: string };
      if (a.fq && a.segment) monikerToKey.set(a.segment, a.fq);
      return undefined;
    }
    if (command === "spatial_unregister_scope") {
      const a = (commandArgs ?? {}) as { fq?: string };
      if (a.fq) {
        for (const [segment, key] of monikerToKey.entries()) {
          if (key === a.fq) {
            monikerToKey.delete(segment);
            break;
          }
        }
      }
      return undefined;
    }
    if (command === "spatial_drill_in") {
      const key = (commandArgs as { fq?: string })?.fq ?? "";
      const focusedMoniker =
        (commandArgs as { focusedFq?: string })?.focusedFq ?? null;
      // Under the no-silent-dropout contract the kernel echoes the focused
      // moniker when there's nothing to descend into. A non-null
      // `drillInResponses` entry means "drill walked to a child" — return
      // it verbatim. A `null` entry means "stay put" — echo the focused
      // moniker so the React closure's compare-to-focused fall-through
      // fires. No entry → echo the focused moniker.
      if (drillInResponses.has(key)) {
        const v = drillInResponses.get(key);
        return v === null ? focusedMoniker : v;
      }
      return focusedMoniker;
    }
    if (command === "spatial_drill_out") {
      // Same echo contract for drill-out — the layer-root edge returns the
      // focused moniker so the React side dispatches app.dismiss.
      return (commandArgs as { focusedFq?: string })?.focusedFq ?? null;
    }
    if (command === "spatial_navigate") return null;
    if (command === "spatial_focus") {
      // Queued via `queueMicrotask` to match the kernel simulator and real
      // Tauri events — emitting synchronously would hide regressions where
      // `setFocus` writes the store synchronously.
      const fq = ((commandArgs ?? {}) as { fq?: string }).fq ?? null;
      if (fq) {
        const prev = currentFocusKey.key;
        const moniker = segmentForFq(fq);
        currentFocusKey.key = fq;
        queueMicrotask(() => {
          emit({
            payload: {
              window_label: windowLabel,
              prev_fq: prev,
              next_fq: fq,
              next_segment: moniker,
            },
          });
        });
      }
      return undefined;
    }
    if (command === "spatial_clear_focus") {
      const prev = currentFocusKey.key;
      if (prev === null) return undefined;
      currentFocusKey.key = null;
      queueMicrotask(() => {
        emit({
          payload: {
            window_label: windowLabel,
            prev_fq: prev,
            next_fq: null,
            next_segment: null,
          },
        });
      });
      return undefined;
    }
    return UNHANDLED;
  }

  function reset(): void {
    monikerToKey.clear();
    currentFocusKey.key = null;
    drillInResponses.clear();
  }

  return {
    monikerToKey,
    currentFocusKey,
    drillInResponses,
    handleSpatialCommand,
    reset,
  };
}

/**
 * Build a {@link FocusChangedEmit} that fans an event out to every
 * `focus-changed` listener stored in a `Map<string, cb[]>` container — the
 * multi-callback listener convention used by the `vi.hoisted` browser-test
 * harnesses.
 *
 * @param listeners - The Map keyed by event name to arrays of callbacks.
 */
export function emitToListenerMap(
  listeners: Map<string, Array<(event: { payload: unknown }) => void>>,
): FocusChangedEmit {
  return (event) => {
    const handlers = listeners.get(FOCUS_CHANGED_EVENT) ?? [];
    for (const handler of handlers) handler(event);
  };
}

/**
 * Build a {@link FocusChangedEmit} that delivers an event to the single
 * `focus-changed` callback stored in a `Record<string, cb>` container — the
 * single-callback listener convention used by the lighter `*-enter` tests.
 *
 * @param listenCallbacks - The record keyed by event name to one callback.
 */
export function emitToCallbackRecord(
  listenCallbacks: Record<string, (event: { payload: unknown }) => void>,
): FocusChangedEmit {
  return (event) => {
    const cb = listenCallbacks[FOCUS_CHANGED_EVENT];
    if (cb) cb(event);
  };
}
