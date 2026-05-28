/**
 * Hotkey dispatch is metadata-driven (card `01KS36XGKCQ36QM7P6MH3FHMBJ`): the
 * global keybinding layer is built from `useCommandList`'s `keys` rather than a
 * hardcoded table. `extractKeymapBindings` derives the `key → commandId` table
 * for the active keymap, and `createKeyHandler` dispatches the bound command.
 * When the keymap switches (itself a `settings.keymap.*` command) the table is
 * rebuilt for the new mode, so the same command answers to a different key.
 *
 * This test exercises that pipeline directly with a registry list containing
 * `task.untag` bound to `x` (vim) and `Delete` (cua):
 *
 *   - vim active: `x` dispatches `task.untag`,
 *   - switch to cua and rebuild: `Delete` dispatches `task.untag`,
 *   - the old vim key (`x`) no longer fires under cua.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import {
  createKeyHandler,
  extractKeymapBindings,
  type KeymapMode,
} from "@/lib/keybindings";
import type { CommandMetadata } from "@/hooks/use-command-list";

/** Minimal KeyboardEvent-like object. */
function fakeKeyEvent(key: string): KeyboardEvent {
  return {
    key,
    metaKey: false,
    ctrlKey: false,
    shiftKey: false,
    altKey: false,
    target: document.createElement("div"),
    preventDefault: vi.fn(),
    stopPropagation: vi.fn(),
  } as unknown as KeyboardEvent;
}

/** The registry as `useCommandList` would return it. */
const REGISTRY: CommandMetadata[] = [
  {
    id: "task.untag",
    name: "Untag",
    keys: { vim: "x", cua: "Delete" },
  },
];

/**
 * Stand-in for `KeybindingHandler`'s effect: build the registry-derived global
 * bindings for `mode` and hand them to `createKeyHandler`. This is exactly the
 * wiring in `app-shell.tsx` — `extractKeymapBindings(commands, mode)` feeds
 * `createKeyHandler(mode, exec, scopeBindings, globalBindings)`.
 */
function handlerForMode(
  mode: KeymapMode,
  exec: (id: string) => Promise<boolean>,
) {
  const globalBindings = extractKeymapBindings(REGISTRY, mode);
  return createKeyHandler(mode, exec, undefined, globalBindings);
}

describe("hotkey dispatch via useCommandList keys", () => {
  let exec: (id: string) => Promise<boolean>;

  beforeEach(() => {
    exec = vi.fn(async () => true) as (id: string) => Promise<boolean>;
  });

  it("derives the binding table from the registry for the active keymap", () => {
    expect(extractKeymapBindings(REGISTRY, "vim")).toEqual({
      x: "task.untag",
    });
    expect(extractKeymapBindings(REGISTRY, "cua")).toEqual({
      Delete: "task.untag",
    });
  });

  it("vim: pressing x dispatches task.untag", () => {
    const handler = handlerForMode("vim", exec);
    handler(fakeKeyEvent("x"));
    expect(exec).toHaveBeenCalledWith("task.untag");
  });

  it("rebinds on keymap switch: cua Delete dispatches the same command", () => {
    // Vim handler first — `x` works, `Delete` does not.
    const vimHandler = handlerForMode("vim", exec);
    vimHandler(fakeKeyEvent("Delete"));
    expect(exec).not.toHaveBeenCalled();

    // Keymap switches to cua → rebuild the handler from the registry.
    const cuaHandler = handlerForMode("cua", exec);
    cuaHandler(fakeKeyEvent("Delete"));
    expect(exec).toHaveBeenCalledWith("task.untag");

    // The old vim key no longer fires under cua.
    (exec as ReturnType<typeof vi.fn>).mockClear();
    cuaHandler(fakeKeyEvent("x"));
    expect(exec).not.toHaveBeenCalled();
  });
});
