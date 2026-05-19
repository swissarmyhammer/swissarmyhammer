// kanban-tasks — the canonical operation-tool example.
//
// This plugin demonstrates the SwissArmyHammer SDK's *operation-tool path
// dispatch*: driving a real MCP operation tool by spelling out its
// `noun.verb` path instead of passing a raw `op` string.
//
// ───────────────────────────────────────────────────────────────────────────
// What an operation tool is
// ───────────────────────────────────────────────────────────────────────────
//
// A flat MCP tool has one entry point — you call it with an arguments object.
// An *operation tool* multiplexes many related operations behind a single
// tool, each selected by an `op` string of the form `"<verb> <noun>"`. The
// in-process `kanban` tool is an operation tool: `"add task"`, `"list tasks"`,
// `"init board"`, `"move task"`, and so on are all operations of the one
// `kanban` tool.
//
// An operation tool publishes a discovery tree in its `tools/list` definition
// under the `_meta` key `io.swissarmyhammer/operations`. The tree is keyed
//
//     _meta["io.swissarmyhammer/operations"][<noun>][<verb>] = { op, ... }
//
// where the leaf's `op` is the canonical wire selector. For the `kanban` tool:
//
//     _meta["io.swissarmyhammer/operations"]["task"]["add"].op   === "add task"
//     _meta["io.swissarmyhammer/operations"]["tasks"]["list"].op === "list tasks"
//
// ───────────────────────────────────────────────────────────────────────────
// The two dispatch forms
// ───────────────────────────────────────────────────────────────────────────
//
// The SDK lets you reach an operation two ways:
//
//   • Direct form  — `this.board.kanban({ op: "add task", title: "..." })`.
//     The `op` is already in the arguments; the SDK passes it straight
//     through. This is the form the `files-dispatch` example uses.
//
//   • Path form    — `this.board.kanban.task.add({ title: "..." })`.
//     There is no `op` in the arguments. The SDK walks the registered tool's
//     `_meta` tree — `[noun: "task"][verb: "add"]` — reads the leaf's
//     `op` ("add task"), and dispatches `tools/call("kanban", { op: "add
//     task", ... })` for you.
//
// THIS example uses the path form. The path form is the one that exercises
// `io.swissarmyhammer/operations` `_meta` lookup: if the SDK could not read
// the operation tool's `_meta`, `this.board.kanban.task.add` could not
// produce the `"add task"` selector and the call would fail before any
// `tools/call` left the isolate.
//
// ───────────────────────────────────────────────────────────────────────────
// noun vs. verb: read the `_meta`, do not guess
// ───────────────────────────────────────────────────────────────────────────
//
// The path segments are the operation's *noun* and *verb* exactly as the tool
// declares them — NOT an English pluralization you invent. The `kanban` tool
// declares its add-a-task operation with noun `task` and its list-the-tasks
// operation with noun `tasks` (plural). So the paths are:
//
//     this.board.kanban.task.add({ ... })     // op "add task"   — noun "task"
//     this.board.kanban.tasks.list({ ... })   // op "list tasks" — noun "tasks"
//
// `this.board.kanban.task.list(...)` would be wrong: the `task` noun has no
// `list` verb in the tool's `_meta`, and the SDK would raise `UnknownOperation`
// rather than dispatch a phantom call. When in doubt, the noun/verb pair is
// whatever the tool's `_meta` tree publishes — the SDK is the source of truth.

import { Plugin, makePluginThis } from "@swissarmyhammer/plugin";

// The two tasks this plugin seeds onto the board. The end-to-end test that
// drives this bundle (`tests/kanban_tasks_e2e.rs`) asserts the board holds
// exactly these two titles after load, so they are a fixed contract.
const FIRST_TASK_TITLE = "Draft the plugin proposal";
const SECOND_TASK_TITLE = "Review the plugin proposal";

/**
 * Counts the tasks in a `kanban` `list tasks` result.
 *
 * A `list tasks` call returns a `CallToolResult` shape — an object with a
 * `content` array whose first entry's `text` is the listing JSON. The listing
 * is itself an object `{ tasks: [...], count: N }`. This walks that shape and
 * returns the task count, so `load()` can log it.
 *
 * @param result - the value returned by `this.board.kanban.tasks.list({})`.
 * @returns the number of tasks on the board.
 * @throws if the result is not the expected `CallToolResult`/listing shape.
 */
function countTasks(result: unknown): number {
  const content = (result as { content?: Array<{ text?: string }> }).content;
  if (content === undefined || content.length === 0) {
    throw new Error("list tasks result carried no content");
  }
  const text = content[0].text;
  if (typeof text !== "string") {
    throw new Error("list tasks content[0].text was not a string");
  }
  const listing = JSON.parse(text) as { tasks?: unknown };
  if (!Array.isArray(listing.tasks)) {
    throw new Error("list tasks content carried no `tasks` array");
  }
  return listing.tasks.length;
}

/**
 * The kanban-tasks example plugin.
 *
 * Its `load()` registers the host-exposed in-process `kanban` operation tool
 * and seeds two tasks onto the board, then lists the board back — all through
 * the SDK's operation-tool *path form*.
 */
class KanbanTasksPlugin extends Plugin {
  /** Human-readable name — descriptive metadata only, not plugin identity. */
  readonly name = "Kanban Tasks Example";

  /** Version string — descriptive metadata only. */
  readonly version = "1.0.0";

  /**
   * Registers the `kanban` operation tool and drives it through the path form.
   *
   * Steps:
   *   1. activate the host-exposed `kanban` Rust module under the name `board`;
   *   2. add two tasks with the path form `this.board.kanban.task.add(...)`;
   *   3. list the tasks with `this.board.kanban.tasks.list({})` and log the
   *      count.
   *
   * The host calls this exactly once, when the plugin is discovered.
   */
  async load(): Promise<void> {
    // (1) Activate the host-exposed real `kanban` operation tool under the
    //     name `board`. After this, `this.board` is the dispatch index for the
    //     `kanban` tool.
    this.register("board", { rust: "kanban" });

    // (2) Add two tasks through the PATH FORM. Neither call passes an `op`:
    //     the SDK reads the `kanban` tool's
    //     `_meta["io.swissarmyhammer/operations"]["task"]["add"].op`, finds
    //     `"add task"`, and dispatches `tools/call("kanban", { op: "add
    //     task", ... })`. The tasks land on the board only if that `_meta`
    //     lookup succeeded.
    await this.board.kanban.task.add({ title: FIRST_TASK_TITLE });
    await this.board.kanban.task.add({ title: SECOND_TASK_TITLE });

    // (3) List the board back through the path form. Note the noun is `tasks`
    //     (plural) — the `kanban` tool declares its list operation under noun
    //     `tasks`, verb `list` (op `"list tasks"`). The path segments mirror
    //     the tool's `_meta` exactly; they are not guessed.
    const listed = await this.board.kanban.tasks.list({});
    this.log.info(`kanban-tasks: board now has ${countTasks(listed)} task(s)`);
  }
}

/**
 * The plugin entry point.
 *
 * The host calls this once when the bundle is discovered. It builds the
 * plugin, wraps it with `makePluginThis` so `this.<server>` dispatch works,
 * and runs the plugin's `load()`.
 *
 * @returns `null` — this plugin exposes no value to the host beyond its
 *   load-time effects.
 */
export async function load(): Promise<unknown> {
  const plugin = makePluginThis(new KanbanTasksPlugin()) as KanbanTasksPlugin;
  await plugin.load();
  return null;
}
