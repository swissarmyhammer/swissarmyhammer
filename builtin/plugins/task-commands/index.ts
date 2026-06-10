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
  Availability,
  CommandContext,
  Plugin,
  ensureServices,
  registerCommands,
  scopeId,
  targetId,
  unwrapResult,
} from "@swissarmyhammer/plugin";

/** A task as it appears in a `list tasks` payload (only the fields we read). */
interface ListedTask {
  id: string;
  ordinal: string;
}

/**
 * List the tasks in `column`, excluding `excludeId`, sorted by ordinal ascending.
 *
 * Mirrors `task_commands::load_sorted_column_tasks`: the neighbor list the
 * legacy server-side ordinal computation walks. The plugin can't compute
 * FractionalIndex ordinals itself, so it materializes this ordered neighbor
 * list and then references a neighbor by id (`before_id`) — letting the
 * canonical `MoveTask::execute` compute the ordinal exactly as the legacy
 * `compute_ordinal_for_drop` did.
 */
async function sortedColumnTasks(
  plugin: TaskCommandsPlugin,
  column: string,
  excludeId: string | undefined,
): Promise<ListedTask[]> {
  const listed = await plugin.kanban.kanban.tasks.list({ column });
  const tasks = unwrapResult(listed).tasks;
  if (!Array.isArray(tasks)) return [];
  const out: ListedTask[] = [];
  for (const raw of tasks) {
    const task = raw as {
      id?: unknown;
      position?: { column?: unknown; ordinal?: unknown };
    };
    const id = typeof task.id === "string" ? task.id : undefined;
    if (id === undefined || id === excludeId) continue;
    // `list tasks { column }` is already column-scoped, but guard anyway so a
    // future server change to that filter can't silently pull in other columns.
    const taskColumn = task.position?.column;
    if (typeof taskColumn === "string" && taskColumn !== column) continue;
    const ordinal =
      typeof task.position?.ordinal === "string" ? task.position.ordinal : "";
    out.push({ id, ordinal });
  }
  out.sort((a, b) => (a.ordinal < b.ordinal ? -1 : a.ordinal > b.ordinal ? 1 : 0));
  return out;
}

/**
 * Read a string-valued entry out of the dispatch context's args bag.
 *
 * Returns `undefined` when the args bag is absent, the key is missing, or the
 * value is not a string — the signal to fall back to scope/target resolution.
 */
function strArg(ctx: CommandContext, key: string): string | undefined {
  const value = ctx.args?.[key];
  return typeof value === "string" ? value : undefined;
}

/**
 * The task-commands builtin plugin.
 *
 * Registers the three `task`-entity commands ported from `task.yaml`, each
 * wired to the `kanban` MCP server. Identity is the bundle directory name
 * (`task-commands`); `name` / `description` are descriptive metadata only.
 */
export default class TaskCommandsPlugin extends Plugin {
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
      // column(target) / drop_index(args). Needs a task (in scope or as the
      // `id` arg) AND a column to move into (target moniker or `column` arg).
      //
      // The args fallbacks mirror the legacy Rust `MoveTaskCmd`
      // (`resolve_move_task_args`): the board-view drop path
      // (`usePersistTaskMove`) dispatches `target: "task:<dragged>"` with
      // args `{ id, column, before_id | after_id }` — no column target
      // moniker — so dropping the fallbacks broke every internal drag drop.
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
          if (scopeId(ctx, "task") === undefined && strArg(ctx, "id") === undefined) {
            return { ok: false, reason: "Select a task first" } satisfies Availability;
          }
          if (
            targetId(ctx, "column") === undefined &&
            strArg(ctx, "column") === undefined
          ) {
            return { ok: false, reason: "Drop the task onto a column" } satisfies Availability;
          }
          return { ok: true } satisfies Availability;
        },
        execute: async (rawCtx) => {
          const ctx = (rawCtx ?? {}) as CommandContext;
          // An explicit `id` arg wins over the ambient scope chain: the drop
          // path names the DRAGGED task in args while focus (the scope chain)
          // may still sit on a different card.
          const id = strArg(ctx, "id") ?? scopeId(ctx, "task");
          const column = targetId(ctx, "column") ?? strArg(ctx, "column");
          const args: Record<string, unknown> = { id, column };

          // Placement precedence mirrors the legacy `MoveTaskCmd`:
          // ordinal > before_id > after_id > drop_index > append.
          // `before_id` / `after_id` neighbor references pass straight through
          // — the canonical `MoveTask::execute` computes the ordinal from the
          // neighbor, exactly as the legacy `compute_ordinal_for_drop` did.
          const ordinal = strArg(ctx, "ordinal");
          const beforeId = strArg(ctx, "before_id");
          const afterId = strArg(ctx, "after_id");
          const dropIndex = ctx.args?.drop_index;
          if (ordinal !== undefined) {
            args.ordinal = ordinal;
          } else if (beforeId !== undefined) {
            args.before_id = beforeId;
          } else if (afterId !== undefined) {
            args.after_id = afterId;
          } else if (typeof dropIndex === "number" && column !== undefined) {
            // `drop_index` is a NUMERIC index into the target column, but the
            // `move task` op only understands an `ordinal` FractionalIndex
            // STRING or a `before_id` / `after_id` neighbor reference.
            // Translate the index into a neighbor reference. Passing the raw
            // number as `ordinal` would parse to a garbage FractionalIndex and
            // mis-position the task — the regression this replaces.
            const neighbors = await sortedColumnTasks(this, column, id);
            // index 0 → before the first; 0 < i < len → before the task that
            // currently sits at the index (it shifts right); i >= len → append
            // (no neighbor reference, MoveTask appends at the end).
            if (dropIndex < neighbors.length) {
              args.before_id = neighbors[Math.max(0, dropIndex)].id;
            }
          }
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
        execute: async (rawCtx) => {
          const ctx = (rawCtx ?? {}) as CommandContext;
          const id = scopeId(ctx, "task");

          // The legacy `DoThisNextCmd` is an undoable MUTATION: it moves the
          // scoped task to the FRONT of the first column. (The kanban `next
          // task` op is a read-only query — routing here would do nothing, the
          // regression this replaces.) Find the order-0 column, then move the
          // task before that column's current first task so it lands at the top.
          const listedColumns = await this.kanban.kanban.columns.list({});
          const columns = unwrapResult(listedColumns).columns;
          if (!Array.isArray(columns) || columns.length === 0) {
            // No columns on the board — nothing to do (matches the legacy
            // "no columns on board" failure surface).
            return await this.kanban.kanban.columns.list({});
          }
          // `list columns` is already sorted by `order` ascending (lowest
          // first); break ties by id so the choice is deterministic, matching
          // the legacy `first_column_id`.
          const sortedColumns = [...columns].sort((a, b) => {
            const ao = Number((a as { order?: unknown }).order ?? 0);
            const bo = Number((b as { order?: unknown }).order ?? 0);
            if (ao !== bo) return ao - bo;
            const ai = String((a as { id?: unknown }).id ?? "");
            const bi = String((b as { id?: unknown }).id ?? "");
            return ai < bi ? -1 : ai > bi ? 1 : 0;
          });
          const firstColumn = String(
            (sortedColumns[0] as { id?: unknown }).id ?? "",
          );

          const neighbors = await sortedColumnTasks(this, firstColumn, id);
          const args: Record<string, unknown> = { id, column: firstColumn };
          // Place before the column's current first task so the scoped task
          // lands at position zero — exactly what `MoveTask::with_before(first)`
          // does. With no other task in the column, MoveTask appends (which is
          // position zero in an otherwise-empty column).
          if (neighbors.length > 0) {
            args.before_id = neighbors[0].id;
          }
          return await this.kanban.kanban.task.move(args);
        },
      },
    ]);

    this.log.info("task-commands: registered task.move, task.untag, task.doThisNext");
  }
}