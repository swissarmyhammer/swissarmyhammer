// task-commands — builtin plugin porting `task.yaml` (the `task` entity's
// type-specific commands) to the TypeScript plugin SDK.
//
// This is the FIRST of the seven builtin command-plugin ports, so it sets the
// pattern the rest mirror:
//
//   1. A `Plugin` subclass carries `name` / `description` as descriptive class
//      props (plugin identity is the bundle directory name — `task-commands`).
//   2. `load()` calls `ensureServices(this, [...])` FIRST to activate every
//      host service the plugin's commands route to, THEN `registerCommands`.
//   3. Each registration carries the FULL UI metadata from the source YAML —
//      `keys`, `scope`, `undoable`, `context_menu`, `params` — 1:1, so the
//      command behaves identically to the YAML-driven version.
//   4. The plugin holds NO business logic: each `execute` makes exactly ONE
//      MCP call into the backing server (here `kanban`), and each `available`
//      only encodes the YAML's preconditions over the command context.
//
// The three commands ported here (`task.move`, `task.untag`,
// `task.doThisNext`) all route to the `kanban` MCP server's operation tool.

import {
  Plugin,
  ensureServices,
  registerCommands,
  makePluginThis,
} from "@swissarmyhammer/plugin";

// ───────────────────────────────────────────────────────────────────────────
// The command context the host hands every `execute` / `available` callback.
// ───────────────────────────────────────────────────────────────────────────

/**
 * The dispatch context the command service passes a command callback.
 *
 * Mirrors `swissarmyhammer_command_service::CommandContext`: the active scope
 * monikers, the optional context-menu target moniker, and a free-form args
 * bag the dispatching surface populates. A moniker is an `"<entity_type>:<id>"`
 * pair (e.g. `"task:01ABC"`), which is what a YAML `from: scope_chain` /
 * `from: target` param resolves against.
 */
interface CommandContext {
  /** Active scope monikers, leaf-last (e.g. `["board:01A", "task:42"]`). */
  scope_chain?: string[];
  /** Context-menu target moniker (the entity the menu fired over). */
  target?: string;
  /** Free-form args bag populated by the dispatching surface. */
  args?: Record<string, unknown>;
}

/** An `available` callback result: ok, or not-ok with a user-facing reason. */
type Availability = { ok: true } | { ok: false; reason: string };

/**
 * Resolve the id of the first scope-chain moniker of `entityType`.
 *
 * A `from: scope_chain` param with `entity_type: <t>` resolves to the id half
 * of the nearest `"<t>:<id>"` moniker in the chain. Returns `undefined` when no
 * such moniker is in scope — the signal an `available` precondition is unmet.
 */
function scopeId(ctx: CommandContext, entityType: string): string | undefined {
  const prefix = `${entityType}:`;
  // Scope chains are leaf-last; scan from the leaf so the nearest entity wins.
  const chain = ctx.scope_chain ?? [];
  for (let i = chain.length - 1; i >= 0; i -= 1) {
    const moniker = chain[i];
    if (moniker.startsWith(prefix)) {
      return moniker.slice(prefix.length);
    }
  }
  return undefined;
}

/**
 * Resolve the id of the context target moniker when it is of `entityType`.
 *
 * A `from: target` param with `entity_type: <t>` resolves to the id half of
 * `ctx.target` when the target moniker is a `"<t>:<id>"` pair. Returns
 * `undefined` when there is no target or it is a different entity type.
 */
function targetId(ctx: CommandContext, entityType: string): string | undefined {
  const target = ctx.target;
  if (target === undefined) return undefined;
  const prefix = `${entityType}:`;
  return target.startsWith(prefix) ? target.slice(prefix.length) : undefined;
}

/**
 * The task-commands builtin plugin.
 *
 * Registers the three `task`-entity commands ported from `task.yaml`, each
 * wired to the `kanban` MCP server. Identity is the bundle directory name
 * (`task-commands`); `name` / `description` are descriptive metadata only.
 */
class TaskCommandsPlugin extends Plugin {
  /** Human-readable name — descriptive metadata only, not plugin identity. */
  readonly name = "Task Commands";

  /** One-line description — descriptive metadata only. */
  readonly description =
    "Builtin task-entity commands (move / untag / do-this-next) routed to the kanban server.";

  /**
   * Activate the services these commands route to, then register the commands.
   *
   * The convention every command-registering plugin follows:
   * `ensureServices` FIRST — so the `commands` registry and the `kanban`
   * backend are both live before any registration — then `registerCommands`.
   * The metadata on each registration is the source YAML's metadata, 1:1.
   */
  async load(): Promise<void> {
    await ensureServices(this, ["commands", "kanban"]);

    await registerCommands(this, [
      // ─── task.move ──────────────────────────────────────────────────────
      // YAML: scope entity:task, undoable, params task(scope_chain) /
      // column(target) / drop_index(args). Needs a task in scope AND a column
      // target to move into.
      {
        id: "task.move",
        name: "Move Task",
        scope: ["entity:task"],
        undoable: true,
        params: [
          { name: "task", from: "scope_chain", entity_type: "task" },
          { name: "column", from: "target", entity_type: "column" },
          { name: "drop_index", from: "args" },
        ],
        available: (rawCtx) => {
          const ctx = (rawCtx ?? {}) as CommandContext;
          if (scopeId(ctx, "task") === undefined) {
            return { ok: false, reason: "Select a task first" } satisfies Availability;
          }
          if (targetId(ctx, "column") === undefined) {
            return { ok: false, reason: "Drop the task onto a column" } satisfies Availability;
          }
          return { ok: true } satisfies Availability;
        },
        execute: async (rawCtx) => {
          const ctx = (rawCtx ?? {}) as CommandContext;
          const id = scopeId(ctx, "task");
          const column = targetId(ctx, "column");
          const ordinal = ctx.args?.drop_index;
          const args: Record<string, unknown> = { id, column };
          if (ordinal !== undefined) args.ordinal = ordinal;
          return await this.kanban.kanban.task.move(args);
        },
      },

      // ─── task.untag ─────────────────────────────────────────────────────
      // YAML: scope entity:tag,entity:task, undoable, context_menu, keys
      // vim:x / cua:Delete, params tag(scope_chain) / task(scope_chain).
      // Both a tag and a task must be in scope.
      {
        id: "task.untag",
        name: "Remove Tag",
        scope: ["entity:tag", "entity:task"],
        undoable: true,
        context_menu: true,
        keys: { vim: "x", cua: "Delete" },
        params: [
          { name: "tag", from: "scope_chain", entity_type: "tag" },
          { name: "task", from: "scope_chain", entity_type: "task" },
        ],
        available: (rawCtx) => {
          const ctx = (rawCtx ?? {}) as CommandContext;
          if (scopeId(ctx, "task") === undefined) {
            return { ok: false, reason: "Select a task first" } satisfies Availability;
          }
          if (scopeId(ctx, "tag") === undefined) {
            return { ok: false, reason: "Select a tag to remove" } satisfies Availability;
          }
          return { ok: true } satisfies Availability;
        },
        execute: async (rawCtx) => {
          const ctx = (rawCtx ?? {}) as CommandContext;
          const id = scopeId(ctx, "task");
          const tag = scopeId(ctx, "tag");
          return await this.kanban.kanban.task.untag({ id, tag });
        },
      },

      // ─── task.doThisNext ────────────────────────────────────────────────
      // YAML: scope entity:task, undoable, context_menu, params
      // task(scope_chain). Needs a task in scope.
      {
        id: "task.doThisNext",
        name: "Do This Next",
        scope: ["entity:task"],
        undoable: true,
        context_menu: true,
        params: [{ name: "task", from: "scope_chain", entity_type: "task" }],
        available: (rawCtx) => {
          const ctx = (rawCtx ?? {}) as CommandContext;
          if (scopeId(ctx, "task") === undefined) {
            return { ok: false, reason: "Select a task first" } satisfies Availability;
          }
          return { ok: true } satisfies Availability;
        },
        execute: async () => {
          return await this.kanban.kanban.task.next({});
        },
      },
    ]);

    this.log.info("task-commands: registered task.move, task.untag, task.doThisNext");
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
  const plugin = makePluginThis(new TaskCommandsPlugin()) as TaskCommandsPlugin;
  await plugin.load();
  return null;
}
