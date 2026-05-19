// file-notes — the filesystem-effect example.
//
// This plugin demonstrates driving the in-process `files` MCP tool to produce
// a real, observable effect: it writes a note file, reads it back, and writes
// the read-back content into a second note. It is the most relatable example
// for a plugin author — "my plugin touched the disk" — and it proves an
// in-process Rust tool round-trips its return values back into the isolate.
//
// ───────────────────────────────────────────────────────────────────────────
// The `files` tool and its direct dispatch form
// ───────────────────────────────────────────────────────────────────────────
//
// The in-process `files` tool is an *operation tool*: a single tool that
// multiplexes many operations behind one entry point, each selected by an `op`
// string of the form `"<verb> <noun>"` — here `"write file"` and `"read
// file"`. This example uses the SDK's *direct dispatch form*:
//
//     this.fs.files({ op: "write file", file_path: "...", content: "..." })
//
// The `op` is already in the arguments object, so the SDK passes it straight
// through to a real `tools/call("files", { ... })`. (The companion
// `kanban-tasks` example shows the alternative *path form*, which reads the
// tool's `_meta` to build the `op` for you.)
//
// Note the two operations name their path argument differently — `write file`
// takes `file_path`, `read file` takes `path`. The argument names are whatever
// each operation declares; this example uses each as the `files` tool expects.
//
// ───────────────────────────────────────────────────────────────────────────
// The working-directory contract: RELATIVE paths only
// ───────────────────────────────────────────────────────────────────────────
//
// The `files` tool resolves a RELATIVE path against the host **process's**
// current working directory before it touches the disk; an ABSOLUTE path is
// used verbatim. Each operation resolves at its own site: `write file` joins
// the relative path onto `std::env::current_dir()`, and `read file` resolves
// through the tool's `FilePathValidator`.
//
// This example — being committed source that ships in the repository — cannot
// hard-code an absolute path: there is no temp directory it could name at
// authoring time, and writing to a fixed absolute path would be unsafe. So it
// addresses the `files` tool with RELATIVE paths (`notes/hello.txt`,
// `notes/echo.txt`). Where those files actually land depends entirely on the
// process working directory at load time:
//
//   • the end-to-end test (`tests/file_notes_e2e.rs`) pins the process CWD to
//     a throwaway temp directory, so the notes land there and the real source
//     tree is never written to;
//   • a plugin you write for real should likewise either use a relative path
//     and know the process CWD, or compute an absolute path it controls.
//
// The `notes/` parent directory does not need to exist beforehand — the
// `files` `write file` operation creates parent directories as needed.

import { Plugin, makePluginThis } from "@swissarmyhammer/plugin";

// The two note paths this plugin writes, RELATIVE to the process working
// directory (see the working-directory contract above). The end-to-end test
// that drives this bundle (`tests/file_notes_e2e.rs`) asserts both files land
// under its temp CWD with the body below, so the paths and body are a fixed
// contract.
const HELLO_NOTE = "notes/hello.txt";
const ECHO_NOTE = "notes/echo.txt";

// The text written into the first note and then read back into the second.
// The end-to-end test asserts this exact body in BOTH note files.
const NOTE_BODY = "a note round-tripped through the in-process files tool";

/**
 * Extracts the file text from a `files` `read file` result.
 *
 * A `read file` call returns a `CallToolResult` shape — an object with a
 * `content` array whose first entry's `text` is the file's content. This walks
 * that shape and returns the text, so `load()` can echo it into a second note.
 *
 * @param result - the value returned by `this.fs.files({ op: "read file", ... })`.
 * @returns the read-back file content.
 * @throws if the result is not the expected `CallToolResult` shape.
 */
function readBackText(result: unknown): string {
  const content = (result as { content?: Array<{ text?: string }> }).content;
  if (content === undefined || content.length === 0) {
    throw new Error("read file result carried no content");
  }
  const text = content[0].text;
  if (typeof text !== "string") {
    throw new Error("read file content[0].text was not a string");
  }
  return text;
}

/**
 * The file-notes example plugin.
 *
 * Its `load()` registers the host-exposed in-process `files` operation tool
 * and round-trips a note through it — write, read, write — all against
 * relative paths resolved by the `files` tool against the process working
 * directory.
 */
class FileNotesPlugin extends Plugin {
  /**
   * Registers the `files` operation tool and round-trips a note through it.
   *
   * Steps:
   *   1. activate the host-exposed `files` Rust module under the name `fs`;
   *   2. `write file` the first note at the relative path `notes/hello.txt`;
   *   3. `read file` that note back; the result crosses the dispatcher back
   *      into the isolate as a `CallToolResult` JSON shape;
   *   4. `write file` the read-back content into `notes/echo.txt`.
   *
   * The host calls this exactly once, when the plugin is discovered.
   */
  async load(): Promise<void> {
    // (1) Activate the host-exposed real `files` operation tool under the
    //     name `fs`. `fs` must appear in plugin.json's `provides`. After this,
    //     `this.fs` is the dispatch index for the `files` tool.
    this.register("fs", { rust: "files" });

    // (2) Write the first note through the direct `op` dispatch form. The path
    //     is RELATIVE — the `files` tool resolves it against the process
    //     working directory. `write file` creates the `notes/` parent dir.
    await this.fs.files({
      op: "write file",
      file_path: HELLO_NOTE,
      content: NOTE_BODY,
    });

    // (3) Read the first note back. The return value crosses the dispatcher
    //     back into the isolate. `read file` names its path argument `path`
    //     (not `file_path`). A non-trivial check on the read-back value makes
    //     the plugin fail loudly if return-value marshalling is broken, rather
    //     than silently writing an empty echo note.
    const readResult = await this.fs.files({
      op: "read file",
      path: HELLO_NOTE,
    });
    const readBack = readBackText(readResult);
    if (readBack !== NOTE_BODY) {
      throw new Error("read file did not return the written note body");
    }

    // (4) Write the read-back content into the echo note. The echo note exists
    //     with the right body only if the read-file return value round-tripped
    //     back into the isolate.
    await this.fs.files({
      op: "write file",
      file_path: ECHO_NOTE,
      content: readBack,
    });

    this.log.info(
      `file-notes: round-tripped a note through '${HELLO_NOTE}' into '${ECHO_NOTE}'`,
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
 *   load-time filesystem effects.
 */
export async function load(): Promise<unknown> {
  const plugin = makePluginThis(new FileNotesPlugin()) as FileNotesPlugin;
  await plugin.load();
  return null;
}
