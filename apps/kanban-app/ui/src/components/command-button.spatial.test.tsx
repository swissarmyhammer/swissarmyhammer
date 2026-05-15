/**
 * Spatial-nav moniker test for `<CommandButton>`.
 *
 * Two `<CommandButton>`s with different command ids and the same surface
 * must register as distinct spatial-nav leaves. The moniker shape is
 * `${surface}.${command.id}:${surfaceId}` — the surface namespace prevents
 * two surfaces hosting the same command from colliding, and the command id
 * + surfaceId distinguish sibling buttons within one surface.
 *
 * This pins the contract on the moniker pattern itself — the migration
 * tasks for individual commands depend on this shape being stable across
 * the component's lifetime.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, act } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Tauri API mocks — must come before component imports.
// ---------------------------------------------------------------------------

const monikerToKey = new Map<string, string>();
const listenCallbacks: Record<string, (event: unknown) => void> = {};

/**
 * External call log so test assertions survive `vi.clearAllMocks()` in
 * `beforeEach` — `clearAllMocks` resets the `vi.fn` call history on the
 * factory-bound mock, which would otherwise wipe registrations recorded
 * during render.
 */
const invokeCallLog: Array<[string, unknown]> = [];

function defaultInvoke(cmd: string, args?: unknown): Promise<unknown> {
  invokeCallLog.push([cmd, args]);
  if (cmd === "get_ui_state")
    return Promise.resolve({
      palette_open: false,
      palette_mode: "command",
      keymap_mode: "cua",
      scope_chain: [],
      open_boards: [],
      windows: {},
      recent_boards: [],
    });
  if (cmd === "spatial_register_scope") {
    const a = (args ?? {}) as { fq?: string; segment?: string };
    if (a.fq && a.segment) monikerToKey.set(a.segment, a.fq);
    return Promise.resolve(null);
  }
  if (cmd === "spatial_unregister_scope") {
    const a = (args ?? {}) as { fq?: string };
    if (a.fq) {
      for (const [m, k] of monikerToKey.entries()) {
        if (k === a.fq) {
          monikerToKey.delete(m);
          break;
        }
      }
    }
    return Promise.resolve(null);
  }
  return Promise.resolve(null);
}

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn((cmd: string, args?: unknown) => defaultInvoke(cmd, args)),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((eventName: string, cb: (event: unknown) => void) => {
    listenCallbacks[eventName] = cb;
    return Promise.resolve(() => {});
  }),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main" }),
}));
vi.mock("@tauri-apps/plugin-log", () => ({
  error: vi.fn(),
  warn: vi.fn(),
  info: vi.fn(),
  debug: vi.fn(),
  trace: vi.fn(),
  attachConsole: vi.fn(() => Promise.resolve()),
}));

import { CommandButton } from "./command-button";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { FocusLayer } from "./focus-layer";
import { asSegment } from "@/types/spatial";
import type { CommandDef } from "@/types/kanban";

function registeredSegments(): string[] {
  return invokeCallLog
    .filter(([cmd]) => cmd === "spatial_register_scope")
    .map(([, args]) => {
      const a = (args ?? {}) as { segment?: string };
      return a.segment ?? "";
    });
}

describe("CommandButton spatial moniker", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    monikerToKey.clear();
    invokeCallLog.length = 0;
    for (const key of Object.keys(listenCallbacks)) {
      delete listenCallbacks[key];
    }
  });

  it("registers a distinct spatial-nav leaf per command id on the same surface", async () => {
    const filterCommand: CommandDef = {
      id: "perspective.focusFilter",
      name: "Focus filter",
      tab_button: { icon: "filter" },
    };
    const sortCommand: CommandDef = {
      id: "perspective.clearSort",
      name: "Clear sort",
      tab_button: { icon: "arrow-up-down" },
    };

    await act(async () => {
      render(
        <SpatialFocusProvider>
          <FocusLayer name={asSegment("window")}>
            <CommandButton
              command={filterCommand}
              surface="perspective_tab"
              surfaceId="p1"
            />
            <CommandButton
              command={sortCommand}
              surface="perspective_tab"
              surfaceId="p1"
            />
          </FocusLayer>
        </SpatialFocusProvider>,
      );
      await Promise.resolve();
    });
    // Two flush rounds: the global registry hook resolves the lazy
    // `import("@tauri-apps/api/core")` then re-fires the queued
    // registrations as `spatial_register_scope` invokes. One Promise.resolve
    // isn't always enough — the import settles in a microtask and the
    // re-fired calls land in the next.
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    const segments = registeredSegments();

    expect(segments).toContain("perspective_tab.perspective.focusFilter:p1");
    expect(segments).toContain("perspective_tab.perspective.clearSort:p1");

    // Distinct entries — moniker uniqueness comes from the command id.
    const tabButtonLeaves = segments.filter((s) =>
      s.startsWith("perspective_tab."),
    );
    const distinct = new Set(tabButtonLeaves);
    expect(distinct.size).toBe(tabButtonLeaves.length);
  });

  it("derives the moniker from the surface and surfaceId props", async () => {
    const command: CommandDef = {
      id: "perspective.focusFilter",
      name: "Focus filter",
      tab_button: { icon: "filter" },
    };

    await act(async () => {
      render(
        <SpatialFocusProvider>
          <FocusLayer name={asSegment("window")}>
            <CommandButton
              command={command}
              surface="perspective_tab"
              surfaceId="p1"
            />
            <CommandButton
              command={command}
              surface="perspective_tab"
              surfaceId="p2"
            />
          </FocusLayer>
        </SpatialFocusProvider>,
      );
      await Promise.resolve();
    });
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    const segments = registeredSegments();
    expect(segments).toContain("perspective_tab.perspective.focusFilter:p1");
    expect(segments).toContain("perspective_tab.perspective.focusFilter:p2");
  });
});
