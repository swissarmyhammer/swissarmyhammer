// perspective-commands — builtin plugin porting `perspective.yaml` (the
// perspective entity's 17 type-specific commands) to the TypeScript plugin SDK.
//
// This is the LARGEST of the builtin command-plugin ports, so the seventeen
// registrations are split into one helper module per logical sub-domain to
// keep this entry file readable. Each helper returns an array of registration
// rows; `index.ts` only concatenates them. The template the helpers mirror is
// `task-commands` / `kanban-misc-commands`:
//
//   1. A `Plugin` subclass carries `name` / `description` as descriptive class
//      props (plugin identity is the bundle directory name —
//      `perspective-commands`).
//   2. `load()` calls `ensureServices(this, ["commands", "views"])` FIRST to
//      activate the host services the commands route to, THEN
//      `registerCommands`.
//   3. Each registration carries the FULL UI metadata from `perspective.yaml`
//      — `scope`, `undoable`, `visible`, `context_menu`, `view_kinds`,
//      `tab_button`, `keys`, and the complex `params` shapes (filter
//      expressions, enum `group` / `field` / `direction` params with their
//      `options_from` / `clear_command` annotations, scope-chain perspective
//      ids) — 1:1, so each command behaves identically to the YAML-driven
//      version.
//   4. The plugin holds NO business logic: each `execute` makes exactly ONE
//      MCP call into the `views` server (the in-process face over the
//      PerspectiveContext + ViewsContext kernels), and each `available` only
//      encodes the YAML's scope preconditions.
//
// Backend routing — all 17 commands target the `views` server's perspective
// operations:
//   perspective.load         → views `load perspective`    (perspective.load)
//   perspective.save         → views `save perspective`    (perspective.save)
//   perspective.delete       → views `delete perspective`  (perspective.delete)
//   perspective.rename       → views `rename perspective`  (perspective.rename)
//   perspective.list         → views `list perspective`    (perspective.list)
//   perspective.filter.focus → views `focus filter`        (filter.focus)
//   perspective.filter       → views `set filter`          (filter.set)
//   perspective.clearFilter  → views `clear filter`        (filter.clear)
//   perspective.group        → views `set group`           (group.set)
//   perspective.clearGroup   → views `clear group`         (group.clear)
//   perspective.sort.set     → views `set sort`            (sort.set)
//   perspective.sort.clear   → views `clear sort`          (sort.clear)
//   perspective.sort.toggle  → views `toggle sort`         (sort.toggle)
//   perspective.next         → views `next perspective`    (perspective.next)
//   perspective.prev         → views `prev perspective`    (perspective.prev)
//   perspective.goto         → views `goto perspective`    (perspective.goto)
//   perspective.switch       → views `switch perspective`  (perspective.switch)

import {
  Plugin,
  ensureServices,
  registerCommands,
  makePluginThis,
} from "@swissarmyhammer/plugin";

import { type ViewsDispatch } from "./commands/context.ts";
import { filterCommands } from "./commands/filter.ts";
import { groupCommands } from "./commands/group.ts";
import { sortCommands } from "./commands/sort.ts";
import { navCommands } from "./commands/nav.ts";
import { lifecycleCommands } from "./commands/lifecycle.ts";

/**
 * The perspective-commands builtin plugin.
 *
 * Registers the seventeen perspective-entity commands ported from
 * `perspective.yaml`, every one wired to the `views` MCP server. Identity is
 * the bundle directory name (`perspective-commands`); `name` / `description`
 * are descriptive metadata only.
 */
class PerspectiveCommandsPlugin extends Plugin {
  /** Human-readable name — descriptive metadata only, not plugin identity. */
  readonly name = "Perspective Commands";

  /** One-line description — descriptive metadata only. */
  readonly description =
    "Builtin perspective-entity commands (load / save / delete / rename / list, filter, group, sort, and navigation) routed to the views server.";

  /**
   * Activate the services these commands route to, then register the commands.
   *
   * The convention every command-registering plugin follows: `ensureServices`
   * FIRST — so the `commands` registry and the `views` backend are both live
   * before any registration — then `registerCommands`. The metadata on each
   * registration is `perspective.yaml`'s metadata, 1:1.
   */
  async load(): Promise<void> {
    await ensureServices(this, ["commands", "views"]);

    const views = this as unknown as ViewsDispatch;
    await registerCommands(this, [
      ...lifecycleCommands(views),
      ...filterCommands(views),
      ...groupCommands(views),
      ...sortCommands(views),
      ...navCommands(views),
    ]);

    this.log.info(
      "perspective-commands: registered 17 perspective.* commands (lifecycle / filter / group / sort / nav)",
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
  const plugin = makePluginThis(new PerspectiveCommandsPlugin()) as PerspectiveCommandsPlugin;
  await plugin.load();
  return null;
}
