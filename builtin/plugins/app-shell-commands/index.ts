// app-shell-commands — the SINGLE app-command builtin plugin: the port of the
// three small platform-shell YAML files (`app.yaml`, `settings.yaml`,
// `drag.yaml` — 15 commands) PLUS the former `ui-commands` bundle folded in
// by the ui.*→app.* rename (mop-up card 01KTEBZSVGAZ881RAZZWWZXGPE — there is
// no `ui.*` command namespace): the ported `ui.yaml` commands (every id now
// `app.*`), the Card D/E webview-bus UI-surface commands, and the Card G
// consolidated `entity.inspect` — 33 commands total. Every command here is a
// host-shell / UI-surface concern: OS chrome, undo/redo, UI toggles, keymap
// selection, the drag state machine, the inspector / palette stacks,
// perspective rename, focus recording, and new-window.
//
// Unlike the per-type bundles (`task-commands`, `perspective-commands`,
// `entity-commands`) whose commands target ONE backend, this bundle fans out
// across SIX services — so it is the proof that a single plugin can route by
// concern. The registrations are split into one helper module per source
// domain to keep this entry file readable; each helper returns an array of
// registration rows and `index.ts` only concatenates them (the same sub-file
// split `perspective-commands` uses).
//
//   1. A `Plugin` subclass carries `name` / `description` as descriptive class
//      props (plugin identity is the bundle directory name —
//      `app-shell-commands`).
//   2. `load()` calls `ensureServices(this, ["commands", "app", "ui_state",
//      "store", "window", "focus"])` FIRST — so the `commands` registry and
//      every backend is live before any registration — THEN
//      `registerCommands`.
//   3. Each registration carries the FULL UI metadata from the source YAML —
//      `name`, `keys`, `menu` (with `radio_group` for the keymaps), `scope`,
//      `context_menu*`, `visible`, `undoable`, `params` — 1:1, so each command
//      behaves identically to the YAML-driven version.
//   4. The plugin holds NO business logic: each `execute` makes exactly ONE
//      MCP call into its backend.
//
// Backend routing — 33 commands across 4 backends (`app.command` etc., the
// keymap/drag families, and the entire ui-origin set except `window.new` all
// live on `ui_state`; the webview-bus UI-surface commands have NO backend):
//   app.quit              → app `quit app`        (this.app.app.app.quit)
//   app.about             → app `show about`      (this.app.app.about.show)
//   app.help              → app `show help`       (this.app.app.help.show)
//   app.undo              → store `undo stack`    (this.store.store.stack.undo)
//   app.redo              → store `redo stack`    (this.store.store.stack.redo)
//   app.command           → ui_state `show command`  (...command.show)
//   app.palette           → ui_state `show palette`   (...palette.show)
//   app.search            → ui_state `show search`    (...search.show)
//   app.dismiss           → ui_state `dismiss ui`      (...ui.dismiss)
//   settings.keymap.cua   → ui_state `set keymap` mode=cua    (...keymap.set)
//   settings.keymap.vim   → ui_state `set keymap` mode=vim    (...keymap.set)
//   settings.keymap.emacs → ui_state `set keymap` mode=emacs  (...keymap.set)
//   drag.start            → ui_state `start drag`     (...drag.start)
//   drag.cancel           → ui_state `cancel drag`    (...drag.cancel)
//   drag.complete         → ui_state `complete drag`  (...drag.complete)
//   ...plus the ui-origin routing table in `commands/ui.ts` (app.inspect /
//   entity.inspect / app.inspector.* / app.palette.open / app.palette.close /
//   app.entity.startRename / app.mode.set / app.setFocus → ui_state;
//   window.new → window; field.* / pressable.* / *.drillIn → webview bus).

import {
  Plugin,
  ensureServices,
  registerCommands,
} from "@swissarmyhammer/plugin";

import {
  type AppDispatch,
  type StoreDispatch,
  type UiStateDispatch,
  type WindowDispatch,
} from "./commands/context.ts";
import { appCommands } from "./commands/app.ts";
import { settingsCommands } from "./commands/settings.ts";
import { dragCommands } from "./commands/drag.ts";
import { uiCommands } from "./commands/ui.ts";

/**
 * The app-shell-commands builtin plugin.
 *
 * Registers the fifteen platform-shell commands ported from `app.yaml` /
 * `settings.yaml` / `drag.yaml` plus the eighteen ui-origin commands (the
 * former `ui-commands` bundle, every id now `app.*`), routed across the
 * `app`, `store`, `ui_state`, and `window` MCP servers. Identity is the
 * bundle directory name (`app-shell-commands`); `name` / `description` are
 * descriptive metadata only.
 */
export default class AppShellCommandsPlugin extends Plugin {
  /** Human-readable name — descriptive metadata only, not plugin identity. */
  readonly name = "App Shell Commands";

  /** One-line description — descriptive metadata only. */
  readonly description =
    "Builtin app commands (app quit/about/help, undo/redo, palette/search/dismiss toggles, keymap selection, the drag state machine, inspector open/close, command palette open/close, perspective rename, spatial focus recording, and new window) routed to the app, store, ui_state, and window servers, plus the webview-bus handled field-edit, pressable-activation, and editor drill-in commands.";

  /**
   * Activate the services these commands route to, then register the commands.
   *
   * The convention every command-registering plugin follows: `ensureServices`
   * FIRST — so the `commands` registry and every backend (`app`, `ui_state`,
   * `store`, `window`, `focus`) is live before any registration — then
   * `registerCommands`. The metadata on each registration is the source YAML's
   * metadata, 1:1.
   *
   * `focus` is ensured (and thereby activated into the live registry) even
   * though no command here routes to it: the spatial-nav React layer reaches
   * the focus kernel through the `focus` MCP module via the generic
   * `command_tool_call` bridge, and module activation is what registers it.
   */
  async load(): Promise<void> {
    await ensureServices(this, [
      "commands",
      "app",
      "ui_state",
      "store",
      "window",
      "focus",
    ]);

    const app = this as unknown as AppDispatch;
    const store = this as unknown as StoreDispatch;
    const uiState = this as unknown as UiStateDispatch;
    const window = this as unknown as WindowDispatch;

    await registerCommands(this, [
      ...appCommands(app, store, uiState),
      ...settingsCommands(uiState),
      ...dragCommands(uiState),
      ...uiCommands(uiState, window),
    ]);

    this.log.info(
      "app-shell-commands: registered 33 commands (app.* / settings.keymap.* / drag.* / entity.inspect / window.new / field.* / pressable.* / filter_editor.drillIn) across app / store / ui_state / window",
    );
  }
}