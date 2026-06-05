// App sub-domain — ports the nine commands from `app.yaml`. Despite the shared
// `app.*` id prefix these fan out to THREE backends by concern:
//   - app.quit / app.about / app.help            → `app` server (OS chrome)
//   - app.undo / app.redo                         → `store` server (undo stack)
//   - app.command / app.palette / app.search /
//     app.dismiss                                 → `ui_state` server (UI toggles)
// Each registration carries `app.yaml`'s metadata (keys / menu / visible /
// undoable) 1:1 and makes exactly one MCP call into its backend.

import {
  type AppDispatch,
  type CommandContext,
  type CommandSpec,
  type StoreDispatch,
  type UiStateDispatch,
} from "./context.ts";

/** Build the nine `app.*` command registrations. */
export function appCommands(
  app: AppDispatch,
  store: StoreDispatch,
  uiState: UiStateDispatch,
): CommandSpec[] {
  return [
    // ─── app.about ──────────────────────────────────────────────────────────
    // YAML: menu {path:[App], group:0, order:0}; no keys. Routes to app
    // `show about`.
    {
      id: "app.about",
      name: "About",
      menu: { path: ["App"], group: 0, order: 0 },
      execute: async () => {
        return await app.app.app.about.show({});
      },
    },

    // ─── app.help ───────────────────────────────────────────────────────────
    // YAML: keys vim:F1 / cua:F1. Routes to app `show help`.
    {
      id: "app.help",
      name: "Help",
      keys: { vim: "F1", cua: "F1" },
      execute: async () => {
        return await app.app.app.help.show({});
      },
    },

    // ─── app.quit ───────────────────────────────────────────────────────────
    // YAML: keys cua:Mod+Q / vim:":q", menu {path:[App], group:2, order:0}.
    // Routes to app `quit app`.
    {
      id: "app.quit",
      name: "Quit",
      keys: { cua: "Mod+Q", vim: ":q" },
      menu: { path: ["App"], group: 2, order: 0 },
      execute: async () => {
        return await app.app.app.app.quit({});
      },
    },

    // ─── app.command ────────────────────────────────────────────────────────
    // YAML: keys vim:":" / cua:Mod+Shift+P / emacs:Mod+Shift+P. Routes to
    // ui_state `show command` (open palette in command mode) for the window.
    {
      id: "app.command",
      name: "Command Palette",
      keys: { vim: ":", cua: "Mod+Shift+P", emacs: "Mod+Shift+P" },
      execute: async (rawCtx: unknown) => {
        const ctx = (rawCtx ?? {}) as CommandContext;
        return await uiState.ui_state.ui_state.command.show({
          scope_chain: ctx.scope_chain ?? [],
        });
      },
    },

    // ─── app.palette ────────────────────────────────────────────────────────
    // YAML: visible:false; keys cua:Mod+Shift+P / vim:Mod+Shift+P /
    // emacs:Mod+Shift+P. Routes to ui_state `show palette` (open palette
    // without forcing a mode) for the window.
    {
      id: "app.palette",
      name: "Command Palette",
      visible: false,
      keys: { cua: "Mod+Shift+P", vim: "Mod+Shift+P", emacs: "Mod+Shift+P" },
      execute: async (rawCtx: unknown) => {
        const ctx = (rawCtx ?? {}) as CommandContext;
        return await uiState.ui_state.ui_state.palette.show({
          scope_chain: ctx.scope_chain ?? [],
        });
      },
    },

    // ─── app.search ─────────────────────────────────────────────────────────
    // YAML: keys vim:"/" / cua:Mod+F / emacs:Mod+F, menu {path:[Edit], group:0,
    // order:2}. Routes to ui_state `show search` (open palette in search mode)
    // for the window.
    {
      id: "app.search",
      name: "Find",
      keys: { vim: "/", cua: "Mod+F", emacs: "Mod+F" },
      menu: { path: ["Edit"], group: 0, order: 2 },
      execute: async (rawCtx: unknown) => {
        const ctx = (rawCtx ?? {}) as CommandContext;
        return await uiState.ui_state.ui_state.search.show({
          scope_chain: ctx.scope_chain ?? [],
        });
      },
    },

    // ─── app.dismiss ────────────────────────────────────────────────────────
    // YAML: keys vim:Escape / cua:Escape / emacs:Escape. Routes to ui_state
    // `dismiss ui` (layered close — palette first, then inspector) for the
    // window.
    {
      id: "app.dismiss",
      name: "Dismiss",
      keys: { vim: "Escape", cua: "Escape", emacs: "Escape" },
      execute: async (rawCtx: unknown) => {
        const ctx = (rawCtx ?? {}) as CommandContext;
        return await uiState.ui_state.ui_state.ui.dismiss({
          scope_chain: ctx.scope_chain ?? [],
        });
      },
    },

    // ─── app.undo ───────────────────────────────────────────────────────────
    // YAML: undoable:false (undo is not itself an undoable action); keys
    // cua:Mod+Z / vim:u, menu {path:[Edit], group:0, order:0}. Routes to the
    // store server's `undo stack` — the one unified stack that spans every
    // store in the substrate, NOT the app shell.
    {
      id: "app.undo",
      name: "Undo",
      undoable: false,
      keys: { cua: "Mod+Z", vim: "u" },
      menu: { path: ["Edit"], group: 0, order: 0 },
      execute: async () => {
        return await store.store.store.stack.undo({});
      },
    },

    // ─── app.redo ───────────────────────────────────────────────────────────
    // YAML: undoable:false; keys cua:Mod+Shift+Z / vim:Ctrl+R, menu
    // {path:[Edit], group:0, order:1}. Routes to the store server's
    // `redo stack`.
    {
      id: "app.redo",
      name: "Redo",
      undoable: false,
      keys: { cua: "Mod+Shift+Z", vim: "Ctrl+R" },
      menu: { path: ["Edit"], group: 0, order: 1 },
      execute: async () => {
        return await store.store.store.stack.redo({});
      },
    },
  ];
}
