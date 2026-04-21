/**
 * Test harness: route `@tauri-apps/api/*` calls to a JS `SpatialStateShim`.
 *
 * ## Design
 *
 * Vitest's `vi.mock(...)` calls are hoisted to the top of the file that
 * contains them — the hoist does not follow imports, so shared helpers
 * cannot install mocks on behalf of a test file. To keep the mock
 * installation DRY *and* give vitest the literal `vi.mock` calls it
 * needs, tests call [`installSpatialTauriMocks`] from module scope.
 * That function re-executes the `vi.mock` calls exactly as the test
 * file would hand-write them — the hoist works because the `vi.mock`
 * literals appear in this file and are re-invoked from inside a test
 * file's top-level statement.
 *
 * The dispatcher functions (`invokeShim`, `subscribeFocusChanged`, etc.)
 * live at module scope and read from a shared `SpatialStateShim` that
 * [`setupSpatialShim`] swaps between tests.
 *
 * ## Usage
 *
 * ```ts
 * import { setupSpatialShim } from "@/test/setup-spatial-shim";
 *
 * // Hoisted vi.mock calls that route into the shim dispatcher:
 * vi.mock("@tauri-apps/api/core", async () =>
 *   (await import("@/test/setup-spatial-shim")).tauriCoreMock(),
 * );
 * vi.mock("@tauri-apps/api/event", async () =>
 *   (await import("@/test/setup-spatial-shim")).tauriEventMock(),
 * );
 * vi.mock("@tauri-apps/api/window", async () =>
 *   (await import("@/test/setup-spatial-shim")).tauriWindowMock(),
 * );
 * vi.mock("@tauri-apps/api/webviewWindow", async () =>
 *   (await import("@/test/setup-spatial-shim")).tauriWebviewWindowMock(),
 * );
 * vi.mock("@tauri-apps/plugin-log", async () =>
 *   (await import("@/test/setup-spatial-shim")).tauriPluginLogMock(),
 * );
 *
 * beforeEach(() => setupSpatialShim());
 * ```
 *
 * See [`spatial-nav-canonical.test.tsx`] for a working example.
 *
 * ## Scope
 *
 * This file lives under `src/test/` and is never bundled into the
 * application — only test builds import it.
 */

import { vi } from "vitest";
import {
  SpatialStateShim,
  type FocusChangedPayload,
  type ShimDirection,
  type ShimSpatialEntry,
} from "./spatial-shim";

// ---------------------------------------------------------------------------
// Module-level state — the shim and its listener registry.
// ---------------------------------------------------------------------------
//
// `setupSpatialShim` replaces both on every call so each test starts
// with a fresh instance; the dispatcher closures below read through
// these let-bindings so they always see the current shim.

let currentShim: SpatialStateShim = new SpatialStateShim();

let focusListeners = new Set<(evt: { payload: FocusChangedPayload }) => void>();

/**
 * Record of every `dispatch_command` invocation observed by the shim's
 * `invoke` mock. Tests that care about which command fired (e.g. "Enter
 * on a focused LeftNav button dispatches `view.switch:<id>`") read
 * this log via [`SpatialShimHandles.dispatchedCommands`] instead of
 * trying to assert against the mocked `invoke` function directly —
 * the shim owns the module-level mock so there is no external `vi.fn`
 * handle to `toHaveBeenCalledWith`.
 */
let dispatchedCommands: Array<{
  cmd: string;
  target?: string;
  args?: Record<string, unknown>;
  scopeChain?: string[];
  boardPath?: string;
}> = [];

/** Argument shape for `spatial_register`. */
interface SpatialRegisterArgs {
  args: {
    key: string;
    moniker: string;
    x: number;
    y: number;
    w: number;
    h: number;
    layerKey?: string;
    layer_key?: string;
    parentScope?: string | null;
    parent_scope?: string | null;
    overrides?: Record<string, string | null> | null;
  };
}

/** Argument shape for `spatial_register_batch`. */
interface SpatialRegisterBatchArgs {
  entries: Array<{
    key: string;
    moniker: string;
    x: number;
    y: number;
    w: number;
    h: number;
    layerKey?: string;
    layer_key?: string;
    parentScope?: string | null;
    parent_scope?: string | null;
    overrides?: Record<string, string | null> | null;
  }>;
}

/** Accept both camelCase and snake_case (matches Rust's serde aliases). */
function toShimEntry(
  payload:
    | SpatialRegisterArgs["args"]
    | SpatialRegisterBatchArgs["entries"][number],
): ShimSpatialEntry {
  const layerKey = payload.layerKey ?? payload.layer_key;
  if (typeof layerKey !== "string") {
    throw new Error(
      `spatial_register: missing layerKey/layer_key (got ${JSON.stringify(payload)})`,
    );
  }
  return {
    key: payload.key,
    moniker: payload.moniker,
    rect: { x: payload.x, y: payload.y, width: payload.w, height: payload.h },
    layerKey,
    parentScope: payload.parentScope ?? payload.parent_scope ?? null,
    overrides: payload.overrides ?? {},
  };
}

/** Every command name the shim recognises. */
type SpatialCommand =
  | "spatial_register"
  | "spatial_register_batch"
  | "spatial_unregister"
  | "spatial_unregister_batch"
  | "spatial_focus"
  | "spatial_clear_focus"
  | "spatial_navigate"
  | "spatial_push_layer"
  | "spatial_remove_layer"
  | "spatial_focus_first_in_layer"
  | "__spatial_dump";

/** Narrow a command name to the spatial set. */
function isSpatialCommand(cmd: string): cmd is SpatialCommand {
  return (
    cmd === "spatial_register" ||
    cmd === "spatial_register_batch" ||
    cmd === "spatial_unregister" ||
    cmd === "spatial_unregister_batch" ||
    cmd === "spatial_focus" ||
    cmd === "spatial_clear_focus" ||
    cmd === "spatial_navigate" ||
    cmd === "spatial_push_layer" ||
    cmd === "spatial_remove_layer" ||
    cmd === "spatial_focus_first_in_layer" ||
    cmd === "__spatial_dump"
  );
}

/** Dispatch an event to every registered listener. */
function emitFocusChangedEvent(payload: FocusChangedPayload): void {
  for (const listener of focusListeners) {
    listener({ payload });
  }
}

/** Dispatch a `spatial_*` command by name. */
function dispatchSpatial(cmd: SpatialCommand, rawArgs: unknown): unknown {
  switch (cmd) {
    case "spatial_register": {
      const a = rawArgs as SpatialRegisterArgs;
      currentShim.register(toShimEntry(a.args));
      return null;
    }
    case "spatial_register_batch": {
      const a = rawArgs as SpatialRegisterBatchArgs;
      currentShim.registerBatch(a.entries.map(toShimEntry));
      return null;
    }
    case "spatial_unregister": {
      const { key } = rawArgs as { key: string };
      const event = currentShim.unregister(key);
      if (event) emitFocusChangedEvent(event);
      return null;
    }
    case "spatial_unregister_batch": {
      const { keys } = rawArgs as { keys: string[] };
      const event = currentShim.unregisterBatch(keys);
      if (event) emitFocusChangedEvent(event);
      return null;
    }
    case "spatial_focus": {
      const { key } = rawArgs as { key: string };
      const event = currentShim.focus(key);
      if (event) emitFocusChangedEvent(event);
      return null;
    }
    case "spatial_clear_focus": {
      const event = currentShim.clearFocus();
      if (event) emitFocusChangedEvent(event);
      return null;
    }
    case "spatial_navigate": {
      const { key, direction } = rawArgs as {
        // `key` is optional — React sends `null` when no moniker is
        // focused, so the shim's `navigate()` can fall back to
        // first-in-layer (same as the Rust command signature).
        key: string | null;
        direction: ShimDirection;
      };
      const event = currentShim.navigate(key ?? null, direction);
      if (event) {
        emitFocusChangedEvent(event);
        const nextKey = event.next_key;
        if (nextKey) {
          const entry = currentShim.get(nextKey);
          return entry?.moniker ?? null;
        }
      }
      return null;
    }
    case "spatial_push_layer": {
      const { key, name } = rawArgs as { key: string; name: string };
      currentShim.pushLayer(key, name);
      return null;
    }
    case "spatial_remove_layer": {
      const { key } = rawArgs as { key: string };
      const event = currentShim.removeLayer(key);
      if (event) emitFocusChangedEvent(event);
      return null;
    }
    case "spatial_focus_first_in_layer": {
      // Mirrors the Rust `SpatialFocusFirstInLayerArgs` struct: the outer
      // invoke payload is `{ args: { layerKey | layer_key } }`.
      const a = rawArgs as {
        args: { layerKey?: string; layer_key?: string };
      };
      const layerKey = a.args.layerKey ?? a.args.layer_key;
      if (typeof layerKey !== "string") {
        throw new Error(
          `spatial_focus_first_in_layer: missing layerKey/layer_key (got ${JSON.stringify(rawArgs)})`,
        );
      }
      const event = currentShim.focusFirstInLayer(layerKey);
      if (event) emitFocusChangedEvent(event);
      return null;
    }
    case "__spatial_dump": {
      const entries = currentShim.entriesSnapshot();
      const layers = currentShim.layersSnapshot();
      const focusedKey = currentShim.focusedKeySnapshot();
      const focused = focusedKey
        ? entries.find((e) => e.key === focusedKey)
        : undefined;
      const counts = new Map<string, number>();
      for (const e of entries) {
        counts.set(e.layerKey, (counts.get(e.layerKey) ?? 0) + 1);
      }
      return {
        focused_key: focusedKey,
        focused_moniker: focused?.moniker ?? null,
        entry_count: entries.length,
        layer_stack: layers.map((l) => ({
          key: l.key,
          name: l.name,
          last_focused: l.lastFocused,
          entry_count_in_layer: counts.get(l.key) ?? 0,
        })),
      };
    }
  }
}

// ---------------------------------------------------------------------------
// Mock factories — called from a test file's `vi.mock(...)` hoist
// ---------------------------------------------------------------------------

/**
 * Mock module for `@tauri-apps/api/core`.
 *
 * `invoke` routes `spatial_*` calls into the shim dispatcher, records every
 * `dispatch_command` invocation into the module-level `dispatchedCommands`
 * log (exposed via [`SpatialShimHandles.dispatchedCommands`]), and
 * resolves every other command to `null`. Extend via `mockInvokeOverride`
 * in a sibling test if you need additional commands.
 */
export function tauriCoreMock() {
  return {
    invoke: async (cmd: string, args?: unknown) => {
      if (isSpatialCommand(cmd)) return dispatchSpatial(cmd, args ?? {});
      if (cmd === "dispatch_command") {
        dispatchedCommands.push(
          (args ?? {}) as {
            cmd: string;
            target?: string;
            args?: Record<string, unknown>;
            scopeChain?: string[];
            boardPath?: string;
          },
        );
        return null;
      }
      return null;
    },
    transformCallback: vi.fn(),
    convertFileSrc: vi.fn((path: string) => path),
    Channel: class {
      // Minimal stub to satisfy any import-time type checks.
    },
  };
}

/** Mock module for `@tauri-apps/api/event`. */
export function tauriEventMock() {
  return {
    listen: async (
      event: string,
      cb: (e: { payload: FocusChangedPayload }) => void,
    ) => {
      if (event === "focus-changed") {
        focusListeners.add(cb);
        return () => focusListeners.delete(cb);
      }
      return () => {};
    },
    emit: vi.fn(() => Promise.resolve()),
    once: vi.fn(() => Promise.resolve(() => {})),
    TauriEvent: {},
  };
}

/** Mock module for `@tauri-apps/api/window`. */
export function tauriWindowMock() {
  return {
    getCurrentWindow: () => ({
      label: "main",
      listen: vi.fn(() => Promise.resolve(() => {})),
    }),
  };
}

/**
 * Mock module for `@tauri-apps/api/webviewWindow`.
 *
 * Production code listens for `focus-changed` via
 * `getCurrentWebviewWindow().listen()` so it only receives emits scoped to
 * its own window. The mock mirrors `tauriEventMock`'s behaviour for the
 * `focus-changed` channel so shim-driven events reach the frontend.
 */
export function tauriWebviewWindowMock() {
  return {
    getCurrentWebviewWindow: () => ({
      label: "main",
      listen: async (
        event: string,
        cb: (e: { payload: FocusChangedPayload }) => void,
      ) => {
        if (event === "focus-changed") {
          focusListeners.add(cb);
          return () => focusListeners.delete(cb);
        }
        return () => {};
      },
    }),
  };
}

/** Mock module for `@tauri-apps/plugin-log`. */
export function tauriPluginLogMock() {
  return {
    error: vi.fn(),
    warn: vi.fn(),
    info: vi.fn(),
    debug: vi.fn(),
    trace: vi.fn(),
    attachConsole: vi.fn(() => Promise.resolve()),
  };
}

// ---------------------------------------------------------------------------
// Public API — reset the shim, return handles for inspection
// ---------------------------------------------------------------------------

/** Handles exposed by [`setupSpatialShim`]. */
export interface SpatialShimHandles {
  /** The underlying state machine — inspect or mutate directly in tests. */
  shim: SpatialStateShim;
  /**
   * Manually fire a `focus-changed` event. Useful when a test needs to
   * simulate a backend-driven focus change without going through the
   * shim dispatcher (e.g. for an event-shape-only assertion).
   */
  emitFocusChanged: (payload: FocusChangedPayload) => void;
  /** Moniker of the currently focused key, or null. */
  focusedMoniker: () => string | null;
  /**
   * Snapshot of every `dispatch_command` invocation captured since
   * `setupSpatialShim()` was last called.
   *
   * Each entry mirrors the payload passed to
   * `invoke("dispatch_command", …)` by the frontend dispatch pipeline —
   * `cmd` is the command id (e.g. `"view.switch:board"`), and the
   * remaining fields are the dispatch options Rust would receive.
   *
   * Returns a fresh array each call so test assertions (`toContainEqual`,
   * `[0].cmd`) can iterate safely without worrying about later mutations.
   */
  dispatchedCommands: () => Array<{
    cmd: string;
    target?: string;
    args?: Record<string, unknown>;
    scopeChain?: string[];
    boardPath?: string;
  }>;
}

/**
 * Reset the shim + listener registry and return a fresh handle.
 *
 * Call this at the start of every test (e.g. in `beforeEach`). The
 * returned `shim` is the live instance the dispatcher reads from — any
 * `spatial_*` invokes made after this call route through the new
 * instance.
 */
export function setupSpatialShim(): SpatialShimHandles {
  currentShim = new SpatialStateShim();
  focusListeners = new Set();
  dispatchedCommands = [];
  return {
    shim: currentShim,
    emitFocusChanged: emitFocusChangedEvent,
    focusedMoniker: () => {
      const fk = currentShim.focusedKeySnapshot();
      if (!fk) return null;
      const e = currentShim.get(fk);
      return e?.moniker ?? null;
    },
    dispatchedCommands: () => dispatchedCommands.slice(),
  };
}
