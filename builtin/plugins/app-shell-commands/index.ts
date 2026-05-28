// app-shell-commands — builtin plugin porting the three small platform-shell
// YAML files (`app.yaml`, `settings.yaml`, `drag.yaml` — 15 commands total) to
// the TypeScript plugin SDK. Every command here is a host-shell concern: OS
// chrome, undo/redo, UI toggles, keymap selection, and the drag state machine.
//
// Unlike the per-type bundles (`task-commands`, `perspective-commands`,
// `entity-commands`) whose commands target ONE backend, this bundle fans out
// across FOUR services — so it is the proof that a single plugin can route by
// concern. The fifteen registrations are split into one helper module per
// source-YAML domain to keep this entry file readable; each helper returns an
// array of registration rows and `index.ts` only concatenates them (the same
// sub-file split `perspective-commands` uses).
//
//   1. A `Plugin` subclass carries `name` / `description` as descriptive class
//      props (plugin identity is the bundle directory name —
//      `app-shell-commands`).
//   2. `load()` calls `ensureServices(this, ["commands", "app", "ui_state",
//      "store"])` FIRST — so the `commands` registry and all three backends are
//      live before any registration — THEN `registerCommands`.
//   3. Each registration carries the FULL UI metadata from the source YAML —
//      `name`, `keys`, `menu` (with `radio_group` for the keymaps), `visible`,
//      `undoable` — 1:1, so each command behaves identically to the
//      YAML-driven version.
//   4. The plugin holds NO business logic: each `execute` makes exactly ONE
//      MCP call into its backend.
//
// Backend routing — 15 commands across 3 backends (`app.command` etc. and the
// keymap/drag families all live on `ui_state`):
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

import {
  Plugin,
  ensureServices,
  registerCommands,
  makePluginThis,
} from "@swissarmyhammer/plugin";

import {
  type AppDispatch,
  type StoreDispatch,
  type UiStateDispatch,
} from "./commands/context.ts";
import { appCommands } from "./commands/app.ts";
import { settingsCommands } from "./commands/settings.ts";
import { dragCommands } from "./commands/drag.ts";

/**
 * The app-shell-commands builtin plugin.
 *
 * Registers the fifteen platform-shell commands ported from `app.yaml` /
 * `settings.yaml` / `drag.yaml`, routed across the `app`, `store`, and
 * `ui_state` MCP servers. Identity is the bundle directory name
 * (`app-shell-commands`); `name` / `description` are descriptive metadata only.
 */
class AppShellCommandsPlugin extends Plugin {
  /** Human-readable name — descriptive metadata only, not plugin identity. */
  readonly name = "App Shell Commands";

  /** One-line description — descriptive metadata only. */
  readonly description =
    "Builtin platform-shell commands (app quit/about/help, undo/redo, palette/search/dismiss toggles, keymap selection, and the drag state machine) routed to the app, store, and ui_state servers.";

  /**
   * Activate the services these commands route to, then register the commands.
   *
   * The convention every command-registering plugin follows: `ensureServices`
   * FIRST — so the `commands` registry and all three backends (`app`,
   * `ui_state`, `store`) are live before any registration — then
   * `registerCommands`. The metadata on each registration is the source YAML's
   * metadata, 1:1.
   */
  async load(): Promise<void> {
    await ensureServices(this, ["commands", "app", "ui_state", "store"]);

    const app = this as unknown as AppDispatch;
    const store = this as unknown as StoreDispatch;
    const uiState = this as unknown as UiStateDispatch;

    await registerCommands(this, [
      ...appCommands(app, store, uiState),
      ...settingsCommands(uiState),
      ...dragCommands(uiState),
    ]);

    this.log.info(
      "app-shell-commands: registered 15 commands (app.* / settings.keymap.* / drag.*) across app / store / ui_state",
    );
  }
}

/**
 * The plugin entry point.
 *
 * The host calls this once when the bundle is discovered: build the plugin,
 * wrap it with `makePluginThis` so `this.<server>` dispatch works, and run
 * `load()`.
 *
 * @returns `null` — this plugin's only effect is its load-time registrations.
 */
export async function load(): Promise<unknown> {
  const plugin = makePluginThis(new AppShellCommandsPlugin()) as AppShellCommandsPlugin;
  await plugin.load();
  return null;
}
