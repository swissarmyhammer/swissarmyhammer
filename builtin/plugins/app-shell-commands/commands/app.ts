// App sub-domain — ports the nine commands from `app.yaml`. Despite the shared
// `app.*` id prefix these fan out to THREE backends by concern:
//   - app.quit / app.about / app.help            → `app` server (OS chrome)
//   - app.undo / app.redo                         → `store` server (undo stack)
//   - app.command / app.palette / app.search /
//     app.dismiss                                 → `ui_state` server (UI toggles)
// Each registration carries `app.yaml`'s metadata (keys / menu / visible /
// undoable) 1:1 and makes exactly one MCP call into its backend.
//
// ───────────────────────────────────────────────────────────────────────────
// Hotkey-canonical key literals
// ───────────────────────────────────────────────────────────────────────────
//
// Since Card I deleted `app-shell.tsx`'s static scope defs, this registry
// metadata is the ONLY key source for the webview hotkey path:
// `extractKeymapBindings` matches the declared string LITERALLY against
// `normalizeKeyEvent` output, which emits lowercase letters for unshifted
// chords. So every unshifted-letter chord must be declared lowercase
// (`Mod+z`, not `Mod+Z`) or it is structurally unreachable from a real
// keydown. The macOS native menu accelerators parse letters case-insensitively,
// so the lowercase form serves the accelerator AND the webview. The
// `app-shell-plugin-commands-mirror.spatial.node.test.ts` drift guard pins the
// `APP_COMMANDS` keys against `BINDING_TABLES`.

import {
  type AppDispatch,
  type CommandContext,
  type CommandSpec,
  type StoreDispatch,
  type UiStateDispatch,
} from "./context.ts";

/**
 * Dispatch holders the {@link APP_COMMANDS} executes close over.
 *
 * `APP_COMMANDS` is a module-level data table (so the frontend drift guard can
 * parse its `id` / `name` / `keys` from source the way it parses
 * `nav-commands`' `NAV_DIRECTIONS` / `ai-commands`' `AI_COMMANDS`), but the
 * `app.*` executes — unlike the webview-reactive `ai.*` no-ops — route to real
 * backends. Rather than thread three dispatch args through a `.map()` (which
 * would push the array inside a function, defeating the `const NAME = [ … ];`
 * anchor the guard relies on), the executes read these holders, which
 * {@link appCommands} sets before returning the table. Each plugin runs in its
 * OWN isolate with exactly one `AppCommandsPlugin`, so this module-level state
 * is per-plugin-instance — no cross-instance sharing.
 */
let appD: AppDispatch | null = null;
let storeD: StoreDispatch | null = null;
let uiStateD: UiStateDispatch | null = null;

/**
 * The nine `app.*` command registrations, as a module-level data table (the
 * same hoisted-table structure as `nav-commands`' `NAV_DIRECTIONS` /
 * `ai-commands`' `AI_COMMANDS`, which lets the frontend drift guard
 * `app-shell-plugin-commands-mirror.spatial.node.test.ts` parse it from
 * source). Each `execute` reads the dispatch holders {@link appCommands} sets.
 *
 * `keys` use the canonical lowercase form `normalizeKeyEvent` emits for an
 * unshifted letter chord — see the module header. The drift guard pins this
 * against `BINDING_TABLES`.
 */
const APP_COMMANDS: CommandSpec[] = [
  // ─── app.about ──────────────────────────────────────────────────────────
  // YAML: menu {path:[App], group:0, order:0}; no keys. Routes to app
  // `show about`.
  {
    id: "app.about",
    name: "About",
    menu: { path: ["App"], group: 0, order: 0 },
    execute: async () => {
      return await appD!.app.app.about.show({});
    },
  },

  // ─── app.help ───────────────────────────────────────────────────────────
  // YAML: keys vim:F1 / cua:F1. Routes to app `show help`.
  {
    id: "app.help",
    name: "Help",
    keys: { vim: "F1", cua: "F1" },
    execute: async () => {
      return await appD!.app.app.help.show({});
    },
  },

  // ─── app.quit ───────────────────────────────────────────────────────────
  // YAML: keys cua:Mod+q / vim:":q", menu {path:[App], group:2, order:0}.
  // Routes to app `quit app`.
  //
  // The cua key is canonicalized to lowercase `Mod+q` (was `Mod+Q`): there is
  // NO `BINDING_TABLES` entry for app.quit — it rides the native App-menu
  // accelerator — but the lowercase form keeps the accelerator working (its
  // letter parse is case-insensitive) AND makes the chord reachable in the
  // webview on non-Mac, matching the `file.closeBoard` precedent (Card I). The
  // drift guard treats app.quit as a COMMENTED menu-accelerator-only
  // allowlist entry (no `BINDING_TABLES` row to pin against).
  {
    id: "app.quit",
    name: "Quit",
    keys: { cua: "Mod+q", vim: ":q" },
    menu: { path: ["App"], group: 2, order: 0 },
    execute: async () => {
      return await appD!.app.app.app.quit({});
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
      return await uiStateD!.ui_state.ui_state.command.show({
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
      return await uiStateD!.ui_state.ui_state.palette.show({
        scope_chain: ctx.scope_chain ?? [],
      });
    },
  },

  // ─── app.search ─────────────────────────────────────────────────────────
  // YAML: keys vim:"/" / cua:Mod+f, menu {path:[Edit], group:0, order:2}.
  // Routes to ui_state `show search` (open palette in search mode) for the
  // window.
  //
  // The cua key is canonicalized to lowercase `Mod+f` (was `Mod+F`) —
  // `BINDING_TABLES.cua` agrees (`Mod+f` → app.search). The vim `/` is already
  // canonical (`BINDING_TABLES.vim` also carries `Mod+f` → app.search, the
  // CUA-style alias). The emacs key is DROPPED on purpose: `BINDING_TABLES.emacs`
  // binds `Mod+f` → `nav.right` (the non-Mac normalization of emacs forward-char
  // Ctrl+f), so canonicalizing app.search's emacs key to `Mod+f` would hijack
  // emacs Ctrl+F from navigate-right to Find and reopen the first-id-wins
  // nondeterminism (cards 01KTQ6QZNB3VN4MAND7VPASM21 /
  // 01KMT56FTBAP8PQ4QQND08MP97). Emacs Find is left to the command palette.
  {
    id: "app.search",
    name: "Find",
    keys: { vim: "/", cua: "Mod+f" },
    menu: { path: ["Edit"], group: 0, order: 2 },
    execute: async (rawCtx: unknown) => {
      const ctx = (rawCtx ?? {}) as CommandContext;
      return await uiStateD!.ui_state.ui_state.search.show({
        scope_chain: ctx.scope_chain ?? [],
      });
    },
  },

  // ─── app.dismiss ────────────────────────────────────────────────────────
  // Routes to ui_state `dismiss ui` (layered close — palette first, then
  // inspector) for the window.
  //
  // No longer keyed to Escape (card `01KTPDTH772HSEV5F7R1DKYDNJ`): Escape is
  // owned globally by `nav.drillOut`, which drills out one focus level and,
  // at a layer-root edge, falls through to this same `dismiss ui` op. Binding
  // Escape here too made `app.dismiss` a competing Escape owner that the
  // first-id-wins keymap layer could pick over `nav.drillOut`. The command id
  // survives — it is still dispatched programmatically (inspector backdrop
  // click, quick-capture) and discoverable in the palette — just unbound from
  // Escape.
  {
    id: "app.dismiss",
    name: "Dismiss",
    execute: async (rawCtx: unknown) => {
      const ctx = (rawCtx ?? {}) as CommandContext;
      return await uiStateD!.ui_state.ui_state.ui.dismiss({
        scope_chain: ctx.scope_chain ?? [],
      });
    },
  },

  // ─── app.undo ───────────────────────────────────────────────────────────
  // YAML: undoable:false (undo is not itself an undoable action); keys
  // cua:Mod+z / vim:u, menu {path:[Edit], group:0, order:0}. Routes to the
  // store server's `undo stack` — the one unified stack that spans every
  // store in the substrate, NOT the app shell.
  //
  // The cua key is canonicalized to lowercase `Mod+z` (was `Mod+Z`) —
  // `BINDING_TABLES.cua` agrees (`Mod+z` → app.undo). The emacs `Ctrl+/`
  // binding moved here from `app-shell.tsx`'s deleted `STATIC_GLOBAL_COMMANDS`
  // (Card I): the registry is now the only key source for the webview hotkey
  // path, so the key must live on this registration to keep emacs-mode undo
  // working.
  {
    id: "app.undo",
    name: "Undo",
    undoable: false,
    keys: { cua: "Mod+z", vim: "u", emacs: "Ctrl+/" },
    menu: { path: ["Edit"], group: 0, order: 0 },
    execute: async () => {
      return await storeD!.store.store.stack.undo({});
    },
  },

  // ─── app.redo ───────────────────────────────────────────────────────────
  // YAML: undoable:false; keys cua:Mod+Shift+Z / vim:Mod+r, menu
  // {path:[Edit], group:0, order:1}. Routes to the store server's
  // `redo stack`.
  //
  // The vim key is canonicalized to `Mod+r` (was `Ctrl+R`) per
  // `BINDING_TABLES.vim` (`Mod+r` → app.redo): non-Mac Ctrl+R normalizes to
  // `Mod+r` anyway, and on Mac this is Cmd+R (Ctrl stays distinct there). The
  // literal `Ctrl+R` form is unreachable from `normalizeKeyEvent` output.
  {
    id: "app.redo",
    name: "Redo",
    undoable: false,
    keys: { cua: "Mod+Shift+Z", vim: "Mod+r" },
    menu: { path: ["Edit"], group: 0, order: 1 },
    execute: async () => {
      return await storeD!.store.store.stack.redo({});
    },
  },
];

/** Build the nine `app.*` command registrations. */
export function appCommands(
  app: AppDispatch,
  store: StoreDispatch,
  uiState: UiStateDispatch,
): CommandSpec[] {
  // Bind the dispatch holders the APP_COMMANDS executes close over, then hand
  // back the data table.
  appD = app;
  storeD = store;
  uiStateD = uiState;
  return APP_COMMANDS;
}
