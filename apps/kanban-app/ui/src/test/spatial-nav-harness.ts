/**
 * Shared bootstrap for the spatial-nav browser/test family.
 *
 * The `*enter*` / drill-in spatial tests all stand up the same Tauri-API mock
 * surface before importing the component under test: a `listeners` map, a
 * `mockInvoke` spy, and a `mockListen` spy that records `focus-changed`
 * subscribers. That `vi.hoisted` bootstrap was copy-pasted verbatim across
 * ~20 sibling test files (card `01KV87M0YFYHB1F3ZDWZD74S4T`). The helper
 * functions that read the `mockInvoke` spy's call log — `registerScopeArgs`,
 * the drill / focus IPC collectors, the `dispatch_command` filters — and the
 * `focus-changed` injector were duplicated alongside it.
 *
 * This module is the single source of truth for that bootstrap:
 *
 * - {@link setupSpatialMocks} builds the `{ mockInvoke, mockListen, listeners }`
 *   trio. It is `vi.hoisted`-safe: callers wrap it in an async `vi.hoisted`
 *   that dynamically imports this module (a static import is not yet evaluated
 *   when the hoisted block runs), so the same handles the `vi.mock`
 *   factories close over are returned to the test body.
 * - {@link makeSpatialTestHelpers} closes the call-log readers and the
 *   `focus-changed` injector over a `{ mockInvoke, listeners }` pair so each
 *   test gets the same helpers without re-declaring them.
 * - {@link makeDefaultInvokeImpl} builds the AppShell + BoardView default
 *   `invoke` responder, parameterized by the per-test mutable keymap mode and
 *   the spatial-kernel echo handler.
 *
 * The per-file ENTITY / UI-state IPC answers and the render-stack wrapper stay
 * local — they diverge by surface under test (see the sibling
 * {@link "@/test/mock-spatial-kernel"} harness, which made the same split for
 * the kernel echo contract).
 */

import { vi } from "vitest";
import { act } from "@testing-library/react";
import { FOCUS_CHANGED_EVENT } from "@/lib/mcp-notifications";
import { commandToolCall } from "@/test/mock-command-list";
import { wrapMcpDispatch } from "@/test/mcp-invoke-translator";
import { UNHANDLED } from "@/test/mock-spatial-kernel";
import type {
  FocusChangedPayload,
  FullyQualifiedMoniker,
  WindowLabel,
} from "@/types/spatial";

/** A `focus-changed` listener as registered through the mocked `listen`. */
export type ListenCallback = (event: { payload: unknown }) => void;

/** Spy standing in for `@tauri-apps/api/core::invoke`. */
export type MockInvoke = ReturnType<
  typeof vi.fn<(cmd: string, args?: unknown) => Promise<unknown>>
>;

/** Spy standing in for `@tauri-apps/api/event::listen`. */
export type MockListen = ReturnType<
  typeof vi.fn<(eventName: string, cb: ListenCallback) => Promise<() => void>>
>;

/** The mocked Tauri-API surface a spatial-nav test bootstraps. */
export interface SpatialMocks {
  /** Spy standing in for `@tauri-apps/api/core::invoke`. */
  mockInvoke: MockInvoke;
  /** Spy standing in for `@tauri-apps/api/event::listen`. */
  mockListen: MockListen;
  /** Event-name → registered callbacks, populated by `mockListen`. */
  listeners: Map<string, ListenCallback[]>;
}

/**
 * Build the `{ mockInvoke, mockListen, listeners }` trio the spatial-nav tests
 * mock the Tauri API with.
 *
 * `mockInvoke` defaults to resolving `undefined`; tests install their own
 * implementation in `beforeEach` (see {@link makeDefaultInvokeImpl}).
 * `mockListen` records each callback under its event name and returns an
 * unsubscribe that splices it back out — the shape `subscribeFocusChanged`
 * relies on.
 *
 * Must be called from inside an async `vi.hoisted` block that dynamically
 * imports this module, so the spies exist before the `vi.mock` factories that
 * close over them run:
 *
 * ```ts
 * const { mockInvoke, mockListen, listeners } = await vi.hoisted(async () => {
 *   const { setupSpatialMocks } = await import("@/test/spatial-nav-harness");
 *   return setupSpatialMocks();
 * });
 * ```
 *
 * @returns The freshly-built mock trio.
 */
export function setupSpatialMocks(): SpatialMocks {
  const listeners = new Map<string, ListenCallback[]>();
  const mockInvoke = vi.fn(
    async (_cmd: string, _args?: unknown): Promise<unknown> => undefined,
  );
  const mockListen = vi.fn(
    (eventName: string, cb: ListenCallback): Promise<() => void> => {
      const cbs = listeners.get(eventName) ?? [];
      cbs.push(cb);
      listeners.set(eventName, cbs);
      return Promise.resolve(() => {
        const arr = listeners.get(eventName);
        if (arr) {
          const idx = arr.indexOf(cb);
          if (idx >= 0) arr.splice(idx, 1);
        }
      });
    },
  );
  return { mockInvoke, mockListen, listeners };
}

/** The drill-in family's call-log readers + `focus-changed` injector. */
export interface SpatialTestHelpers {
  /** Pull every `spatial_register_scope` invocation argument bag. */
  registerScopeArgs(): Array<Record<string, unknown>>;
  /** Resolve the registered fully-qualified moniker for a segment moniker. */
  keyForMoniker(moniker: string): FullyQualifiedMoniker | undefined;
  /** Collect every client-side `drill_in layer` IPC, in order. */
  spatialDrillInCalls(): Array<Record<string, unknown>>;
  /** Collect every client-side `drill_out layer` IPC, in order. */
  spatialDrillOutCalls(): Array<Record<string, unknown>>;
  /** Collect every client-side `set focus` IPC, in order. */
  spatialFocusCalls(): Array<Record<string, unknown>>;
  /** Filter `dispatch_command` calls down to those for the given command id. */
  dispatchPayloads(cmdId: string): Array<Record<string, unknown>>;
  /** Filter `dispatch_command` calls down to those for `app.inspect`. */
  inspectDispatches(): Array<Record<string, unknown>>;
  /** Filter `dispatch_command` calls down to those for `entity.inspect`. */
  entityInspectDispatches(): Array<Record<string, unknown>>;
  /**
   * Drive a `focus-changed` event into the React tree as if the Rust kernel
   * had emitted one for the active window.
   */
  fireFocusChanged(args: {
    prev_fq?: FullyQualifiedMoniker | null;
    next_fq?: FullyQualifiedMoniker | null;
    next_segment?: string | null;
  }): Promise<void>;
}

/**
 * Close the spatial-nav call-log readers and the `focus-changed` injector over
 * a test's mock handles, so a test body gets the same helpers without
 * re-declaring them.
 *
 * The readers mirror the production drill wire contract: drill / focus that
 * executes host-side leaves NO client-side IPC, so the `spatialDrill*` /
 * `spatialFocusCalls` collectors must stay empty for keyboard drill. Both the
 * legacy bare `spatial_*` cmd and the post-Stage-3 `command_tool_call` focus
 * envelope are matched, so a test asserting "zero" catches either wire.
 *
 * @param mocks - The `{ mockInvoke, listeners }` handles from {@link setupSpatialMocks}.
 * @returns The bound helper set.
 */
export function makeSpatialTestHelpers(mocks: {
  mockInvoke: SpatialMocks["mockInvoke"];
  listeners: SpatialMocks["listeners"];
}): SpatialTestHelpers {
  const { mockInvoke, listeners } = mocks;

  function registerScopeArgs(): Array<Record<string, unknown>> {
    return mockInvoke.mock.calls
      .filter((c) => c[0] === "spatial_register_scope")
      .map((c) => c[1] as Record<string, unknown>);
  }

  function keyForMoniker(moniker: string): FullyQualifiedMoniker | undefined {
    const zone = registerScopeArgs().find((a) => a.segment === moniker);
    if (zone) return zone.fq as FullyQualifiedMoniker;
    const scope = registerScopeArgs().find((a) => a.segment === moniker);
    return scope?.fq as FullyQualifiedMoniker | undefined;
  }

  function collectFocusTool(
    legacyCmd: string,
    op: string,
  ): Array<Record<string, unknown>> {
    return mockInvoke.mock.calls
      .filter(
        (c) =>
          c[0] === legacyCmd ||
          (c[0] === "command_tool_call" &&
            (c[1] as { tool?: string; op?: string })?.tool === "focus" &&
            (c[1] as { tool?: string; op?: string })?.op === op),
      )
      .map((c) => {
        const outer = c[1] as Record<string, unknown>;
        return (outer?.params ?? outer) as Record<string, unknown>;
      });
  }

  function spatialDrillInCalls(): Array<Record<string, unknown>> {
    return collectFocusTool("spatial_drill_in", "drill_in layer");
  }

  function spatialDrillOutCalls(): Array<Record<string, unknown>> {
    return collectFocusTool("spatial_drill_out", "drill_out layer");
  }

  function spatialFocusCalls(): Array<Record<string, unknown>> {
    return collectFocusTool("spatial_focus", "set focus");
  }

  function dispatchPayloads(cmdId: string): Array<Record<string, unknown>> {
    return mockInvoke.mock.calls
      .filter((c) => c[0] === "dispatch_command")
      .map((c) => c[1] as Record<string, unknown>)
      .filter((p) => p.cmd === cmdId);
  }

  function inspectDispatches(): Array<Record<string, unknown>> {
    return dispatchPayloads("app.inspect");
  }

  function entityInspectDispatches(): Array<Record<string, unknown>> {
    return dispatchPayloads("entity.inspect");
  }

  async function fireFocusChanged({
    prev_fq = null,
    next_fq = null,
    next_segment = null,
  }: {
    prev_fq?: FullyQualifiedMoniker | null;
    next_fq?: FullyQualifiedMoniker | null;
    next_segment?: string | null;
  }): Promise<void> {
    const payload: FocusChangedPayload = {
      window_label: "main" as WindowLabel,
      prev_fq,
      next_fq,
      next_segment: next_segment as FocusChangedPayload["next_segment"],
    };
    const handlers = listeners.get(FOCUS_CHANGED_EVENT) ?? [];
    await act(async () => {
      for (const handler of handlers) handler({ payload });
      await Promise.resolve();
    });
  }

  return {
    registerScopeArgs,
    keyForMoniker,
    spatialDrillInCalls,
    spatialDrillOutCalls,
    spatialFocusCalls,
    dispatchPayloads,
    inspectDispatches,
    entityInspectDispatches,
    fireFocusChanged,
  };
}

/** A spatial-kernel echo handler, as built by `makeSpatialKernelMock`. */
type SpatialCommandHandler = (command: string, commandArgs?: unknown) => unknown;

/** Options for {@link makeDefaultInvokeImpl}. */
export interface DefaultInvokeImplOptions {
  /**
   * Read the current keymap mode for the `get_ui_state` answer. A getter
   * (not a value) so a test can flip the mode per-case without rebuilding the
   * responder.
   */
  keymapMode: () => "cua" | "vim" | "emacs";
  /**
   * The spatial-kernel echo handler — `makeSpatialKernelMock(...).handleSpatialCommand`.
   * Returns {@link UNHANDLED} for any command it does not own.
   */
  handleSpatialCommand: SpatialCommandHandler;
}

/**
 * Build the default `invoke` responder for the AppShell + BoardView provider
 * stack: the handful of IPCs the providers hit on mount, the MCP
 * `command_tool_call` envelope bridge, and a fall-through to the
 * spatial-kernel echo handler.
 *
 * The focus / entity MCP envelope is unwrapped back to the legacy `(cmd, args)`
 * shape and re-entered so the dispatcher (which pre-dates the MCP migration)
 * matches without per-branch changes. Spatial register / focus / drill calls
 * return `undefined` (void) unless the kernel handler claims them.
 *
 * @param options - The keymap-mode getter and spatial-kernel echo handler.
 * @returns The `invoke` implementation to install on the `mockInvoke` spy.
 */
export function makeDefaultInvokeImpl(
  options: DefaultInvokeImplOptions,
): (cmd: string, args?: unknown) => Promise<unknown> {
  const { keymapMode, handleSpatialCommand } = options;

  async function defaultInvokeImpl(
    cmd: string,
    args?: unknown,
  ): Promise<unknown> {
    if (cmd === "command_tool_call") {
      const env = args as
        | { tool?: string; op?: string; params?: Record<string, unknown> }
        | undefined;
      if (env?.tool === "focus" || env?.tool === "entity") {
        const wrapped = wrapMcpDispatch(
          { mock: { calls: [] } },
          (legacyCmd: string, legacyArgs?: unknown) =>
            defaultInvokeImpl(legacyCmd, legacyArgs),
        );
        return wrapped(cmd, args);
      }
      return commandToolCall(args);
    }
    if (cmd === "list_entity_types") return ["task", "column"];
    if (cmd === "get_entity_schema") {
      return {
        entity: { name: "task", entity_type: "task" },
        fields: [],
      };
    }
    if (cmd === "get_ui_state")
      return {
        palette_open: false,
        palette_mode: "command",
        keymap_mode: keymapMode(),
        scope_chain: [],
        open_boards: [],
        windows: {},
        recent_boards: [],
      };
    if (cmd === "get_undo_state") return { can_undo: false, can_redo: false };
    if (cmd === "dispatch_command") return undefined;
    const spatial = handleSpatialCommand(cmd, args);
    if (spatial !== UNHANDLED) return spatial;
    return undefined;
  }

  return defaultInvokeImpl;
}
