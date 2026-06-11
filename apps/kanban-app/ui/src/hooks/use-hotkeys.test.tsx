/**
 * Hotkey dispatch is metadata-driven (card `01KS36XGKCQ36QM7P6MH3FHMBJ`): the
 * global keybinding layer is built from `useCommandList`'s `keys` rather than a
 * hardcoded table. `extractKeymapBindings` derives the `key → commandId` table
 * for the active keymap, and `createKeyHandler` dispatches the bound command.
 * When the keymap switches (itself a `settings.keymap.*` command) the table is
 * rebuilt for the new mode, so the same command answers to a different key.
 *
 * Scope-gated commands are excluded from the global table (card
 * `01KTQ6QZNB3VN4MAND7VPASM21`): a non-empty `scope` means the command's keys
 * apply only via the focused-chain walk (`extractChainBindings` over the
 * scope-level `CommandDef` a component registers — for `task.untag`, the tag
 * pill's `useTagUntagCommands` in `badge-list-display.tsx`).
 *
 * This test exercises both layers with the production-shaped `task.untag`
 * (`builtin/plugins/task-commands/index.ts`: `scope: ["entity:tag",
 * "entity:task"]`, `keys: { vim: "x", cua: "Delete" }`):
 *
 *   - the global table NEVER carries the scope-gated keys (no leak),
 *   - with a focused tag-pill scope, `x` (vim) / `Delete` (cua) dispatch
 *     `task.untag` through the scope path,
 *   - without a focused scope, the keys do not fire at all,
 *   - a global (unscoped) command still rebinds across keymap switches.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import {
  createKeyHandler,
  extractChainBindings,
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

/**
 * The registry as `useCommandList` would return it. `task.untag` carries its
 * REAL production shape — scope-gated with keys — so this suite catches a
 * regression where the global extraction starts (or stops) honoring `scope`.
 * `app.demo` is a stand-in global (unscoped) command for the rebind tests.
 */
const REGISTRY: CommandMetadata[] = [
  {
    id: "task.untag",
    name: "Untag",
    scope: ["entity:tag", "entity:task"],
    keys: { vim: "x", cua: "Delete" },
  },
  {
    id: "app.demo",
    name: "Demo",
    keys: { vim: "m", cua: "F6" },
  },
];

/**
 * The tag pill's scope-level `task.untag` CommandDef, as registered by
 * `useTagUntagCommands` (`badge-list-display.tsx`) on each pill's
 * `<FocusScope>` — the sole carrier of the scope-gated keys. (The
 * production-path guard that the real component registers exactly this
 * shape lives in `badge-list-display.test.tsx`.)
 */
const TAG_PILL_SCOPE = {
  commands: new Map([
    ["task.untag", { id: "task.untag", keys: { vim: "x", cua: "Delete" } }],
  ]),
  parent: null,
};

/**
 * Stand-in for `KeybindingHandler`'s effect: build the registry-derived global
 * bindings for `mode` and hand them to `createKeyHandler`. This is exactly the
 * wiring in `app-shell.tsx` — `extractKeymapBindings(commands, mode)` feeds
 * `createKeyHandler(mode, exec, scopeBindings, globalBindings)`, where
 * `scopeBindings` walks the focused chain via `extractChainBindings`.
 */
function handlerForMode(
  mode: KeymapMode,
  exec: (id: string) => Promise<boolean>,
  focusedScope: typeof TAG_PILL_SCOPE | null = null,
) {
  const globalBindings = extractKeymapBindings(REGISTRY, mode);
  return createKeyHandler(
    mode,
    exec,
    () => extractChainBindings(REGISTRY, mode, focusedScope),
    globalBindings,
  );
}

describe("hotkey dispatch via useCommandList keys", () => {
  let exec: (id: string) => Promise<boolean>;

  beforeEach(() => {
    exec = vi.fn(async () => true) as (id: string) => Promise<boolean>;
  });

  it("derives the global binding table from the registry, excluding scope-gated commands", () => {
    // `task.untag` is scope-gated → its keys never reach the global table.
    expect(extractKeymapBindings(REGISTRY, "vim")).toEqual({
      m: "app.demo",
    });
    expect(extractKeymapBindings(REGISTRY, "cua")).toEqual({
      F6: "app.demo",
    });
  });

  it("scope-gated keys do not fire without a focused scope", () => {
    const vimHandler = handlerForMode("vim", exec);
    vimHandler(fakeKeyEvent("x"));
    const cuaHandler = handlerForMode("cua", exec);
    cuaHandler(fakeKeyEvent("Delete"));
    expect(exec).not.toHaveBeenCalled();
  });

  it("vim: pressing x with a focused tag-pill scope dispatches task.untag", () => {
    const handler = handlerForMode("vim", exec, TAG_PILL_SCOPE);
    handler(fakeKeyEvent("x"));
    expect(exec).toHaveBeenCalledWith("task.untag");
  });

  it("cua: pressing Delete with a focused tag-pill scope dispatches task.untag", () => {
    const handler = handlerForMode("cua", exec, TAG_PILL_SCOPE);
    handler(fakeKeyEvent("Delete"));
    expect(exec).toHaveBeenCalledWith("task.untag");
  });

  it("rebinds the scope path on keymap switch", () => {
    // Vim handler first — `x` works, `Delete` does not.
    const vimHandler = handlerForMode("vim", exec, TAG_PILL_SCOPE);
    vimHandler(fakeKeyEvent("Delete"));
    expect(exec).not.toHaveBeenCalled();

    // Keymap switches to cua → rebuild the handler; `Delete` now fires.
    const cuaHandler = handlerForMode("cua", exec, TAG_PILL_SCOPE);
    cuaHandler(fakeKeyEvent("Delete"));
    expect(exec).toHaveBeenCalledWith("task.untag");

    // The old vim key no longer fires under cua.
    (exec as ReturnType<typeof vi.fn>).mockClear();
    cuaHandler(fakeKeyEvent("x"));
    expect(exec).not.toHaveBeenCalled();
  });

  it("rebinds a global command on keymap switch", () => {
    // Vim handler — `m` fires the global command, its cua key does not.
    const vimHandler = handlerForMode("vim", exec);
    vimHandler(fakeKeyEvent("F6"));
    expect(exec).not.toHaveBeenCalled();
    vimHandler(fakeKeyEvent("m"));
    expect(exec).toHaveBeenCalledWith("app.demo");

    // Keymap switches to cua → rebuild from the registry.
    (exec as ReturnType<typeof vi.fn>).mockClear();
    const cuaHandler = handlerForMode("cua", exec);
    cuaHandler(fakeKeyEvent("F6"));
    expect(exec).toHaveBeenCalledWith("app.demo");

    // The old vim key no longer fires under cua.
    (exec as ReturnType<typeof vi.fn>).mockClear();
    cuaHandler(fakeKeyEvent("m"));
    expect(exec).not.toHaveBeenCalled();
  });
});
