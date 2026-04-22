/**
 * Test harness: minimal `@tauri-apps/api/*` boundary stub.
 *
 * ## Why this exists
 *
 * The spatial-nav algorithm lives entirely in Rust
 * (`swissarmyhammer-spatial-nav`). In React tests we do NOT want to
 * re-implement that algorithm in JavaScript — doing so creates a
 * second source of truth that silently drifts from Rust. Instead we
 * stub the Tauri boundary at the thinnest possible layer:
 *
 * - `spatial_register` / `spatial_register_batch` — record the
 *   `key → moniker` mapping so tests can reference scopes by moniker
 *   when scripting focus responses.
 * - `spatial_focus(key)` — immediately emit a matching `focus-changed`
 *   event, as the real Rust backend would.
 * - `spatial_clear_focus` — emit a `focus-changed` with
 *   `next_key: null`.
 * - `spatial_navigate` / `dispatch_command` / other — default no-op,
 *   but tests can install a *scripted* response: "when nav.down is
 *   invoked from moniker X, emit focus-changed with next_key of
 *   moniker Y". The script is whatever the test wants — there is no
 *   algorithm.
 * - `listen("focus-changed", cb)` — register the callback; unlisten
 *   returns a disposer.
 *
 * ## Vitest hoist
 *
 * `vi.mock(...)` calls are hoisted to the top of the file that
 * contains them — the hoist does not follow imports, so shared
 * helpers cannot install mocks on behalf of a test file. Tests call
 * the `tauri*Mock()` factories from module scope, and the factories
 * return the mock module object vitest expects.
 *
 * ## Usage
 *
 * ```ts
 * vi.mock("@tauri-apps/api/core", async () =>
 *   (await import("@/test/setup-tauri-stub")).tauriCoreMock(),
 * );
 * vi.mock("@tauri-apps/api/event", async () =>
 *   (await import("@/test/setup-tauri-stub")).tauriEventMock(),
 * );
 * vi.mock("@tauri-apps/api/window", async () =>
 *   (await import("@/test/setup-tauri-stub")).tauriWindowMock(),
 * );
 * vi.mock("@tauri-apps/api/webviewWindow", async () =>
 *   (await import("@/test/setup-tauri-stub")).tauriWebviewWindowMock(),
 * );
 * vi.mock("@tauri-apps/plugin-log", async () =>
 *   (await import("@/test/setup-tauri-stub")).tauriPluginLogMock(),
 * );
 *
 * beforeEach(() => {
 *   handles = setupTauriStub();
 * });
 * ```
 */

import { vi } from "vitest";

// ---------------------------------------------------------------------------
// Event payload shapes — deliberately mirror Rust's `FocusChanged` serde.
// ---------------------------------------------------------------------------

/**
 * `focus-changed` event payload shape.
 *
 * Snake-cased to match the Rust struct serialisation the real backend
 * emits.
 */
export interface FocusChangedPayload {
  prev_key: string | null;
  next_key: string | null;
}

// ---------------------------------------------------------------------------
// Module-level state — cleared on every `setupTauriStub()` call.
// ---------------------------------------------------------------------------

/** `key → moniker` recorded from every `spatial_register` / `spatial_register_batch`. */
let keyToMoniker: Map<string, string> = new Map();

/** Listeners registered via `listen("focus-changed", cb)`. */
let focusListeners: Set<(evt: { payload: FocusChangedPayload }) => void> =
  new Set();

/**
 * Record of every `invoke(cmd, args)` call for test inspection.
 *
 * Populated in source order so tests can assert both "command was
 * dispatched" and "dispatched in the expected sequence".
 */
let invocations: Array<{ cmd: string; args: unknown }> = [];

/** Record of every `dispatch_command` invocation (a subset of `invocations`). */
let dispatchedCommands: Array<{
  cmd: string;
  target?: string;
  args?: Record<string, unknown>;
  scopeChain?: string[];
  boardPath?: string;
}> = [];

/** Latest key passed to `spatial_focus` — mirrors Rust's `focused_key`. */
let focusedKey: string | null = null;

/**
 * Scripted response map for commands that would normally compute a
 * result in Rust (navigation, focus_first_in_layer, …).
 *
 * Key is a route identifier — `"dispatch_command:<cmd>"` for
 * `dispatch_command` routing, or the raw Tauri command name
 * (`"spatial_navigate"`, `"spatial_focus_first_in_layer"`).
 *
 * The handler receives the raw argument object and returns either:
 * - a `FocusChangedPayload` to emit a `focus-changed` event (focus
 *   moved), or
 * - `null` to emit nothing (focus blocked / no change).
 *
 * Tests install handlers via [`TauriStubHandles.scriptResponse`].
 */
type ScriptedHandler = (args: unknown) => FocusChangedPayload | null;
let scriptedHandlers: Map<string, ScriptedHandler> = new Map();

/** Argument shape for `spatial_register`. */
interface SpatialRegisterArgs {
  args: {
    key: string;
    moniker: string;
    x?: number;
    y?: number;
    w?: number;
    h?: number;
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
    x?: number;
    y?: number;
    w?: number;
    h?: number;
    layerKey?: string;
    layer_key?: string;
    parentScope?: string | null;
    parent_scope?: string | null;
    overrides?: Record<string, string | null> | null;
  }>;
}

/** Emit a `focus-changed` event to every registered listener. */
function emitFocusChanged(payload: FocusChangedPayload): void {
  for (const listener of focusListeners) {
    listener({ payload });
  }
}

// ---------------------------------------------------------------------------
// Public API — reset + handles exposed to tests.
// ---------------------------------------------------------------------------

/** Handles exposed by [`setupTauriStub`]. */
export interface TauriStubHandles {
  /**
   * Fire a `focus-changed` event exactly as Rust would. Tests use this
   * to simulate the backend responding to any op — most commonly after
   * a nav key is dispatched, "the backend says the next key is …".
   */
  emitFocusChanged: (payload: FocusChangedPayload) => void;

  /**
   * Fire a `focus-changed` event looked up by moniker. Convenience
   * wrapper around [`emitFocusChanged`] for the common case where the
   * test owns the `moniker → key` mapping via `spatial_register`.
   */
  emitFocusChangedForMoniker: (moniker: string | null) => void;

  /**
   * Install a scripted response handler for a `dispatch_command:<cmd>`
   * route or a raw spatial command name. Handler return value is
   * emitted as a `focus-changed` event (or skipped when `null`).
   *
   * Example:
   * ```ts
   * handles.scriptResponse("dispatch_command:nav.down", () =>
   *   handles.payloadForFocusMove("field:task:01.title", "field:task:02.title"),
   * );
   * ```
   */
  scriptResponse: (route: string, handler: ScriptedHandler) => void;

  /**
   * Look up the most recently registered `key` for the given
   * `moniker`. Returns `null` when the moniker was never registered.
   *
   * `FocusScope` generates a ULID per mount, so the test doesn't know
   * the key up front — it queries by moniker to build a
   * `FocusChangedPayload`.
   */
  keyForMoniker: (moniker: string) => string | null;

  /**
   * Build a `FocusChangedPayload` that models focus moving from one
   * moniker to another, using the keys recorded via `spatial_register`.
   */
  payloadForFocusMove: (
    prevMoniker: string | null,
    nextMoniker: string | null,
  ) => FocusChangedPayload;

  /**
   * Snapshot of every `invoke(cmd, args)` call observed since the
   * stub was last reset. Fresh array on each call.
   */
  invocations: () => Array<{ cmd: string; args: unknown }>;

  /**
   * Snapshot of every `invoke("dispatch_command", …)` payload since
   * the stub was last reset. Fresh array on each call.
   */
  dispatchedCommands: () => Array<{
    cmd: string;
    target?: string;
    args?: Record<string, unknown>;
    scopeChain?: string[];
    boardPath?: string;
  }>;

  /**
   * Moniker currently reported by the stub as focused — derived from
   * the last `spatial_focus(key)` invocation. Returns `null` when no
   * scope has been focused yet.
   */
  focusedMoniker: () => string | null;
}

/**
 * Reset the stub's state and return a fresh handle.
 *
 * Call this at the start of every test (e.g. in `beforeEach`) so one
 * test's `spatial_register` calls don't leak into the next.
 */
export function setupTauriStub(): TauriStubHandles {
  keyToMoniker = new Map();
  focusListeners = new Set();
  invocations = [];
  dispatchedCommands = [];
  focusedKey = null;
  scriptedHandlers = new Map();

  return {
    emitFocusChanged: (payload) => {
      // Mirror the real backend: `spatial_focus` in Rust updates
      // `focused_key` before emitting, so the stub updates its own
      // tracker when the payload carries a next_key.
      focusedKey = payload.next_key;
      emitFocusChanged(payload);
    },
    emitFocusChangedForMoniker: (moniker) => {
      const prev = focusedKey;
      const next = moniker === null ? null : findKeyForMoniker(moniker);
      focusedKey = next;
      emitFocusChanged({ prev_key: prev, next_key: next });
    },
    scriptResponse: (route, handler) => {
      scriptedHandlers.set(route, handler);
    },
    keyForMoniker: (moniker) => findKeyForMoniker(moniker),
    payloadForFocusMove: (prevMoniker, nextMoniker) => ({
      prev_key: prevMoniker === null ? null : findKeyForMoniker(prevMoniker),
      next_key: nextMoniker === null ? null : findKeyForMoniker(nextMoniker),
    }),
    invocations: () => invocations.slice(),
    dispatchedCommands: () => dispatchedCommands.slice(),
    focusedMoniker: () =>
      focusedKey ? (keyToMoniker.get(focusedKey) ?? null) : null,
  };
}

/**
 * Find the most recently registered key for a moniker.
 *
 * When the same moniker is registered more than once (e.g. same card
 * across remounts) the latest wins — this matches the practical
 * "current mount" semantics tests expect.
 */
function findKeyForMoniker(moniker: string): string | null {
  let found: string | null = null;
  for (const [key, mk] of keyToMoniker) {
    if (mk === moniker) found = key;
  }
  return found;
}

// ---------------------------------------------------------------------------
// Invoke dispatcher — the stub's core.
// ---------------------------------------------------------------------------

/**
 * Run a scripted handler for `route` if one was installed. Emits the
 * resulting `focus-changed` event (when non-null) and returns `true`
 * to indicate the invocation was handled.
 */
function runScriptedHandler(route: string, rawArgs: unknown): boolean {
  const handler = scriptedHandlers.get(route);
  if (!handler) return false;
  const result = handler(rawArgs);
  if (result !== null) {
    focusedKey = result.next_key;
    emitFocusChanged(result);
  }
  return true;
}

/**
 * Handle one Tauri `invoke(cmd, args)` call.
 *
 * Returns the value the real backend would resolve with. For
 * `spatial_navigate`, the real backend returns the new focused
 * moniker (or `null`); the stub returns the moniker derived from the
 * emitted `focus-changed` payload so any caller using the return
 * value sees a consistent answer.
 */
function dispatchInvoke(cmd: string, rawArgs: unknown): unknown {
  invocations.push({ cmd, args: rawArgs });

  switch (cmd) {
    case "spatial_register": {
      const a = rawArgs as SpatialRegisterArgs;
      keyToMoniker.set(a.args.key, a.args.moniker);
      return null;
    }
    case "spatial_register_batch": {
      const a = rawArgs as SpatialRegisterBatchArgs;
      for (const entry of a.entries) keyToMoniker.set(entry.key, entry.moniker);
      return null;
    }
    case "spatial_unregister": {
      const { key } = rawArgs as { key: string };
      keyToMoniker.delete(key);
      return null;
    }
    case "spatial_unregister_batch": {
      const { keys } = rawArgs as { keys: string[] };
      for (const k of keys) keyToMoniker.delete(k);
      return null;
    }
    case "spatial_focus": {
      // The real backend emits `focus-changed` when the focused key
      // actually changes. Mirror that by emitting a focus-changed
      // event with the new key; tests can override via scriptResponse
      // if they need different semantics.
      const { key } = rawArgs as { key: string };
      if (runScriptedHandler("spatial_focus", rawArgs)) return null;
      const prev = focusedKey;
      if (prev === key) return null;
      focusedKey = key;
      emitFocusChanged({ prev_key: prev, next_key: key });
      return null;
    }
    case "spatial_clear_focus": {
      if (runScriptedHandler("spatial_clear_focus", rawArgs)) return null;
      const prev = focusedKey;
      if (prev === null) return null;
      focusedKey = null;
      emitFocusChanged({ prev_key: prev, next_key: null });
      return null;
    }
    case "spatial_navigate": {
      // No algorithm — default is "nothing happens" unless the test
      // installed a scripted handler.
      if (!runScriptedHandler("spatial_navigate", rawArgs)) return null;
      // After the handler ran, return the moniker now focused so
      // callers of `invoke("spatial_navigate", …)` see the same
      // answer the real backend would return.
      return focusedKey ? (keyToMoniker.get(focusedKey) ?? null) : null;
    }
    case "spatial_push_layer":
    case "spatial_remove_layer": {
      if (runScriptedHandler(cmd, rawArgs)) return null;
      return null;
    }
    case "spatial_focus_first_in_layer": {
      if (runScriptedHandler(cmd, rawArgs)) return null;
      return null;
    }
    case "dispatch_command": {
      const payload = (rawArgs ?? {}) as {
        cmd: string;
        target?: string;
        args?: Record<string, unknown>;
        scopeChain?: string[];
        boardPath?: string;
      };
      dispatchedCommands.push(payload);
      // Route through a scripted handler keyed on the dispatched
      // command id. Tests install one per command they want to
      // observe emitting focus-changed.
      if (runScriptedHandler(`dispatch_command:${payload.cmd}`, rawArgs)) {
        return focusedKey ? (keyToMoniker.get(focusedKey) ?? null) : null;
      }
      return null;
    }
    default:
      return null;
  }
}

// ---------------------------------------------------------------------------
// Mock factories — called from a test file's `vi.mock(...)` hoist.
// ---------------------------------------------------------------------------

/**
 * Mock module for `@tauri-apps/api/core`.
 *
 * `invoke` is routed through [`dispatchInvoke`]. Every call is logged
 * to the `invocations` array (and `dispatch_command` calls are also
 * mirrored into `dispatchedCommands`) so tests can assert what the
 * frontend sent.
 */
export function tauriCoreMock() {
  return {
    invoke: async (cmd: string, args?: unknown) =>
      dispatchInvoke(cmd, args ?? {}),
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
 * `getCurrentWebviewWindow().listen()` so the stub must route
 * `focus-changed` subscriptions here too, mirroring the behaviour of
 * [`tauriEventMock`].
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
