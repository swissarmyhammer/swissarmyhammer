// multi-module — the relative sibling-module import example.
//
// Every other example bundle in this directory is a single `index.ts`. This
// one is deliberately TWO files:
//
//     multi-module/
//       index.ts            this file — the entry module
//       board-helpers.ts    a sibling module, imported below
//
// ───────────────────────────────────────────────────────────────────────────
// What this example demonstrates
// ───────────────────────────────────────────────────────────────────────────
//
// A plugin bundle is not limited to one source file. The entry module — or any
// module it imports — can pull in sibling files with ordinary RELATIVE imports.
// The line right below this comment block:
//
//     import { addBoardTask, normalizeTaskTitle } from "./board-helpers.ts";
//
// IS THE POINT OF THIS EXAMPLE. The `./board-helpers.ts` specifier is relative:
// the sandboxed module loader resolves it against THIS bundle's directory,
// reads `board-helpers.ts` from disk, transpiles it, and links it into the same
// V8 isolate as this entry module. If that resolution failed, the isolate would
// throw at module-resolution time and `load()` would never run.
//
// The loader enforces one hard rule on relative imports: the resolved path may
// not escape the bundle directory. `./board-helpers.ts` stays inside the
// bundle, so it resolves; `../something.ts` would be rejected, keeping the
// bundle a self-contained sandbox.
//
// ───────────────────────────────────────────────────────────────────────────
// How the example proves the import resolved
// ───────────────────────────────────────────────────────────────────────────
//
// `board-helpers.ts` exports two helpers — a pure `normalizeTaskTitle` and an
// async `addBoardTask`. This entry module's `load()` registers the host-exposed
// `kanban` operation tool as `board`, then calls the imported `addBoardTask`
// helper to add one tagged task. The task lands on the board ONLY if the
// relative import resolved and the sibling module's code ran — that observable
// board state is what the end-to-end test (`tests/multi_module_e2e.rs`) asserts
// on.

import { Plugin, makePluginThis } from "@swissarmyhammer/plugin";

// The RELATIVE sibling-module import — the whole point of this example. The
// loader resolves `./board-helpers.ts` against this bundle's directory and
// links the sibling module into the isolate.
import { addBoardTask, normalizeTaskTitle } from "./board-helpers.ts";

// The raw, deliberately un-tidy task title passed to the imported helper. The
// stray surrounding and internal whitespace exercises the helper's
// `normalizeTaskTitle` — the end-to-end test asserts the board holds the
// NORMALIZED form ("Ship the multi-module example"), proving the imported
// helper's code actually ran rather than the raw string passing through.
const RAW_TASK_TITLE = "  Ship   the multi-module    example  ";

// The bare tag name (no leading `#`) the imported helper applies to the task.
const TASK_TAG = "multi-module";

/**
 * The multi-module example plugin.
 *
 * Its `load()` registers the host-exposed in-process `kanban` operation tool
 * and adds one tagged task to the board — but it does so by calling a helper
 * imported from the sibling `./board-helpers.ts` module, not by driving the
 * board itself.
 */
class MultiModulePlugin extends Plugin {
  /** Human-readable name — descriptive metadata only, not plugin identity. */
  readonly name = "Multi-Module Example";

  /** Version string — descriptive metadata only. */
  readonly version = "1.0.0";

  /** One-line description — descriptive metadata only. */
  readonly description =
    "Splits its logic across a sibling module imported with a relative specifier.";

  /**
   * Registers the `kanban` operation tool and adds a task via the sibling
   * helper module.
   *
   * Steps:
   *   1. activate the host-exposed `kanban` Rust module under the name `board`;
   *   2. call the imported async `addBoardTask` helper, which normalizes the
   *      title and adds a tagged task through the `board` dispatcher;
   *   3. log the normalized title — computed by the imported pure helper — so
   *      both exports of the sibling module are exercised.
   *
   * The host calls this exactly once, when the plugin is discovered.
   */
  async load(): Promise<void> {
    // (1) Activate the host-exposed real `kanban` operation tool under the
    //     name `board`. After this, `this.board` is the dispatch index for the
    //     `kanban` tool.
    this.register("board", { rust: "kanban" });

    // (2) Add the task by calling the helper IMPORTED from the sibling module.
    //     `this.board` is the dispatcher for the just-registered `kanban` tool;
    //     `addBoardTask` lives in `./board-helpers.ts` and drives it through
    //     the `_meta` path form. The task reaches the board only because the
    //     relative import resolved and the sibling module linked.
    await addBoardTask(this.board, RAW_TASK_TITLE, TASK_TAG);

    // (3) Also exercise the sibling module's pure export directly, so the log
    //     reports exactly the normalized title the board now holds.
    this.log.info(
      `multi-module: sibling helper added task '${normalizeTaskTitle(RAW_TASK_TITLE)}'`,
    );
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
  const plugin = makePluginThis(new MultiModulePlugin()) as MultiModulePlugin;
  await plugin.load();
  return null;
}
