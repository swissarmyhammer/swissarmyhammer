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
  /**
   * The most recent FQM focused under this layer (recursive: a `spatial_focus`
   * call against any scope under this layer walks up the parent chain and
   * records the focused FQM on every ancestor layer's `last_focused` slot).
   *
   * Mirrors `swissarmyhammer-focus/src/registry.rs::record_focus` — the
   * field is what `spatial_pop_layer` returns to the React side, and what
   * `resolve_fallback`'s Phase 2 walk uses to route focus to a parent
   * layer's remembered target.
   */
  lastFocused: FullyQualifiedMoniker | null;
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
  /** Real focus target vs structural zone — see `ShadowEntry.focusable`. */
  focusable: boolean;
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
 * Optional configuration for {@link installKernelSimulator}.
 */
export interface KernelSimulatorOptions {
  /**
   * When true, `spatial_focus` enforces the real kernel's validation:
   *   - The `snapshot` argument must be defined.
   *   - The snapshot's `layer_fq` must reference a pushed layer.
   *   - The target FQM must appear in `snapshot.scopes`.
   *
   * Tests that exercise the React-side focus bridge in isolation (no
   * `<FocusLayer>` mounted, no layer pushed) leave this off so the
   * simulator's existing permissive behavior is preserved. Tests that
   * need to faithfully reproduce production close/open/open cycles —
   * where snapshot validation is what drives the kernel-side
   * `last_focused` walk — turn this on so the simulator's
   * `record_focus` mirror only fires for accepted commits.
   *
   * Default: `false`.
   */
  strictFocusValidation?: boolean;
}

/** A lowered focus-bridge call: the legacy command, its args, and an
 * optional wrapper that re-envelopes the legacy handler's raw return into
 * the shape `focus-mcp.ts`'s wrapper expects. */
interface LoweredFocusCall {
  cmd: string;
  args: Record<string, unknown>;
  /** Re-envelope the legacy handler's raw return value, if needed. */
  wrap?: (raw: unknown) => unknown;
}

/**
 * Lower a `focus` MCP op (the verb `focus-mcp.ts` passes to
 * `callMcpTool("focus", op, params)`) onto the legacy `spatial_*` command
 * name + args the simulator's `dispatchSpatial` answers.
 *
 * The op strings and param shapes mirror `focus-mcp.ts` 1:1:
 *   - `"set focus"`      → `spatial_focus`        `{ fq, snapshot }`
 *   - `"clear focus"`    → `spatial_clear_focus`  `{}`
 *   - `"navigate focus"` → `spatial_navigate`     `{ focusedFq, direction }`
 *   - `"push layer"`     → `spatial_push_layer`   `{ fq, segment, name, parent }`
 *   - `"pop layer"`      → `spatial_pop_layer`    `{ fq }` (wrapped `{ next_fq }`)
 *   - `"drill_in layer"` → `spatial_drill_in`     `{ focusedFq }` (wrapped `{ next_fq }`)
 *   - `"drill_out layer"`→ `spatial_drill_out`    `{ focusedFq }` (wrapped `{ next_fq }`)
 *
 * `pop layer` / `drill_*` wrap the raw FQM the legacy handler returns into
 * the `{ ok, next_fq }` envelope the `popLayer` / `drillIn` / `drillOut`
 * wrappers read. Returns `null` for an unmodelled op so the caller can
 * answer with a benign success envelope.
 */
function lowerFocusBridge(
  op: string,
  params: Record<string, unknown>,
): LoweredFocusCall | null {
  const wrapNextFq = (raw: unknown) => ({ ok: true, next_fq: raw ?? null });
  switch (op) {
    case "set focus":
      return { cmd: "spatial_focus", args: params };
    case "clear focus":
      return { cmd: "spatial_clear_focus", args: params };
    case "navigate focus":
      return { cmd: "spatial_navigate", args: params };
    case "push layer":
      return { cmd: "spatial_push_layer", args: params };
    case "pop layer":
      return { cmd: "spatial_pop_layer", args: params, wrap: wrapNextFq };
    case "drill_in layer":
      return { cmd: "spatial_drill_in", args: params, wrap: wrapNextFq };
    case "drill_out layer":
      return { cmd: "spatial_drill_out", args: params, wrap: wrapNextFq };
    default:
      return null;
  }
}

/**
 * Lower an `entity` MCP op (the verb `entity-mcp.ts` passes to
 * `callMcpTool("entity", op, params)`) onto the legacy `get_entity` Tauri
 * command name + args the test fallbacks answer.
 *
 * Today only `get entity` is wired (it is the single verb `entity-mcp.ts`
 * routes through the bridge):
 *   - `"get entity"` → `get_entity` `{ entityType, id }`, wrapped back into
 *     the `{ ok, entity }` envelope `getEntity` unwraps.
 *
 * The wire params arrive as `{ type, id }` (the `entity` server's op shape);
 * the legacy command took `{ entityType, id }`, so the lowering renames
 * `type` → `entityType`. Returns `null` for an unmodelled op so the caller
 * can answer with a benign success envelope.
 */
function lowerEntityBridge(
  op: string,
  params: Record<string, unknown>,
): LoweredFocusCall | null {
  switch (op) {
    case "get entity":
      return {
        cmd: "get_entity",
        args: { entityType: params.type, id: params.id },
        wrap: (raw: unknown) => ({ ok: true, entity: raw ?? {} }),
      };
    default:
      return null;
  }
}

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
 * @param options - Optional simulator behavior toggles.
 *   See {@link KernelSimulatorOptions}.
 */
export function installKernelSimulator(
  mockInvoke: ReturnType<typeof vi.fn>,
  listeners: Map<string, Array<(event: { payload: unknown }) => void>>,
  fallback: FallbackInvoke = async () => undefined,
  options: KernelSimulatorOptions = {},
): KernelSimulator {
  const strictFocusValidation = options.strictFocusValidation ?? false;
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
      const handlers = listeners.get("notifications/focus/changed") ?? [];
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
    focusable: rec.focusable,
  });

  /**
   * Dispatch a legacy `spatial_*` command against the in-test cascade.
   *
   * This is the simulator's core: it answers the legacy Tauri command
   * names (`spatial_focus`, `spatial_navigate`, `spatial_push_layer`, …)
   * the simulator was originally written against. Production focus IPC
   * now flows through the in-process `focus` MCP server
   * (`callMcpTool("focus", op, …)` → `invoke("command_tool_call", …)`);
   * `mockInvoke`'s implementation below lowers that bridge call back onto
   * these legacy command names before delegating here, so a single
   * cascade implementation serves both wire shapes.
   */
  const dispatchSpatial = async (cmd: string, a: Record<string, unknown>) => {
    if (cmd === "spatial_push_layer") {
      const fq = a.fq as FullyQualifiedMoniker;
      // Re-pushing an existing FQM preserves the prior `lastFocused` —
      // mirrors `swissarmyhammer-focus/src/registry.rs::push_layer`,
      // which keeps drill-out memory across StrictMode double-mount
      // and IPC re-batches.
      const existingLastFocused = layers.get(fq)?.lastFocused ?? null;
      const record: LayerRecord = {
        fq,
        segment: a.segment as SegmentMoniker,
        name: a.name as LayerName,
        parent: (a.parent ?? null) as FullyQualifiedMoniker | null,
        lastFocused: existingLastFocused,
      };
      layers.set(record.fq, record);
      history.push({ type: "push_layer", record });
      return undefined;
    }
    if (cmd === "spatial_pop_layer") {
      // Mirrors the real kernel: returns the popped layer's
      // `last_focused` (or `null`) so the React side can issue the
      // follow-up `spatial_focus(next_fq, snapshot)`.
      const fq = a.fq as FullyQualifiedMoniker;
      const popped = layers.get(fq);
      const nextFq = popped?.lastFocused ?? null;
      layers.delete(fq);
      return nextFq;
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
        focusable: (a.focusable ?? true) as boolean,
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
      const snapshot = a.snapshot as
        | { layer_fq?: FullyQualifiedMoniker; scopes?: unknown[] }
        | undefined
        | null;
      // When strict validation is enabled, mirror the real kernel's
      // validation in `swissarmyhammer-focus/src/state.rs::focus`:
      //   - snapshot must be provided and resolvable; without one the
      //     kernel has no `layer_fq` and drops the commit (`None`).
      //   - the snapshot's `layer_fq` must reference a registered layer.
      //   - the target FQM must be present in the snapshot's scopes.
      // Otherwise stay permissive — accept the call regardless of the
      // snapshot so React-side bridge tests that exercise `setFocus`
      // without mounting a layer still see the focus-changed projection.
      if (strictFocusValidation) {
        if (!snapshot) return undefined;
        const snapshotLayerFq = snapshot.layer_fq;
        if (!snapshotLayerFq || !layers.has(snapshotLayerFq)) {
          return undefined;
        }
        const scopeFqs = new Set<FullyQualifiedMoniker>();
        for (const s of snapshot.scopes ?? []) {
          const scopeFq = (s as { fq?: FullyQualifiedMoniker }).fq;
          if (scopeFq) scopeFqs.add(scopeFq);
        }
        if (!scopeFqs.has(nextFq)) return undefined;
      }
      const prev = currentFocus.fq;
      if (prev === nextFq) {
        // Idempotent — no event emitted, just like the real kernel's
        // "already focused" short-circuit.
        return undefined;
      }
      currentFocus.fq = nextFq;
      // Mirror `swissarmyhammer-focus/src/registry.rs::record_focus`:
      // walk from the snapshot's `layer_fq` up the parent chain and
      // record `last_focused = nextFq` on every ancestor layer. This is
      // what drives `resolve_fallback`'s Phase 2 walk on layer pop and
      // what `spatial_pop_layer` returns to the React side. Skip when
      // the snapshot is absent — without a `layer_fq` there is no walk
      // origin, and the permissive-mode tests don't exercise pop-layer
      // restoration.
      const snapshotLayerFq = snapshot?.layer_fq;
      if (snapshotLayerFq && layers.has(snapshotLayerFq)) {
        let walkFq: FullyQualifiedMoniker | null = snapshotLayerFq;
        const visitedLayers = new Set<FullyQualifiedMoniker>();
        while (walkFq && !visitedLayers.has(walkFq)) {
          visitedLayers.add(walkFq);
          const layer = layers.get(walkFq);
          if (!layer) break;
          layer.lastFocused = nextFq;
          walkFq = layer.parent;
        }
      }
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
    return fallback(cmd, a);
  };

  mockInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
    const a = (args ?? {}) as Record<string, unknown>;

    // Production focus IPC flows through the in-process `focus` MCP server:
    // `focus-mcp.ts` calls `callMcpTool("focus", op, params)`, which lowers
    // onto `invoke("command_tool_call", { module: "focus", tool, op, params })`
    // (`mcp-transport.ts`). The simulator answers the legacy `spatial_*`
    // command names, so lower the bridge call back onto them here and delegate
    // to `dispatchSpatial`. Without this, the auto-focus `set focus` the
    // inspector dispatches on mount falls through to `fallback` (which never
    // emits `focus-changed`), so the title zone's `data-focused` never flips.
    if (cmd === "command_tool_call" && a.module === "focus") {
      const lowered = lowerFocusBridge(
        a.op as string,
        (a.params ?? {}) as Record<string, unknown>,
      );
      if (lowered) {
        const raw = await dispatchSpatial(lowered.cmd, lowered.args);
        return lowered.wrap ? lowered.wrap(raw) : raw;
      }
      // An unmodelled focus op (e.g. `lose focus`, `generate sneak_codes`):
      // answer with a benign success envelope so the wrapper resolves.
      return { ok: true };
    }

    // Production entity reads flow through the in-process `entity` MCP server:
    // `entity-mcp.ts::getEntity` calls `callMcpTool("entity", "get entity",
    // { type, id })`, which lowers onto `invoke("command_tool_call",
    // { module: "entity", tool: "entity", op: "get entity", params })`. The
    // existing test fallbacks (`defaultInvokeImpl`) answer the legacy
    // `invoke("get_entity", { entityType, id })` command, so lower the bridge
    // call back onto that name and re-envelope the raw field bag into the
    // `{ ok, entity }` shape `getEntity` unwraps. Without this, the inspector
    // panel's `getEntity` fetch resolves to `{}`, the entity carries no
    // `entity_type`/`fields`, and `<EntityInspector>` renders "Loading
    // schema..." forever — so no field `<FocusScope>`s ever register.
    if (cmd === "command_tool_call" && a.module === "entity") {
      const lowered = lowerEntityBridge(
        a.op as string,
        (a.params ?? {}) as Record<string, unknown>,
      );
      if (lowered) {
        const raw = await fallback(lowered.cmd, lowered.args);
        return lowered.wrap ? lowered.wrap(raw) : raw;
      }
      // An unmodelled entity op: answer with a benign success envelope so the
      // wrapper resolves rather than rejecting the panel's fetch.
      return { ok: true };
    }

    return dispatchSpatial(cmd, a);
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
