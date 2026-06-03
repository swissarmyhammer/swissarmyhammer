/**
 * Architectural guardrail: no module under `apps/kanban-app/ui/src/`
 * may call `invoke("<handler>", …)` for any Tauri command other than
 * the two MCP transport bridges (`command_tool_call` and
 * `mcp_subscribe`) plus the small explicit allow-list of native-only
 * commands documented below.
 *
 * Stage 3 of the kanban cut-over migrated every domain `invoke` call
 * (entity reads, spatial nav, sneak codes, command-log telemetry) to
 * the in-process MCP servers behind `command_tool_call`. This test
 * prevents a regression: a new `invoke("get_entity", …)` or
 * `invoke("spatial_focus", …)` slipped into a hook would fail here
 * before it ever reached a review.
 *
 * # What is allow-listed
 *
 * - `command_tool_call` — the generic MCP request bridge.
 * - `mcp_subscribe` — the MCP-notification → Tauri-event pump bootstrap.
 * - **AppHandle-bound natives** that have no equivalent on the MCP
 *   wire because they need an ambient Tauri AppHandle / process
 *   primitive: file dialogs, native context menus, window creation,
 *   the OS file-drop sink, the AI agent's WebSocket bridge.
 * - **Schema / data-feed natives** the multi-board kanban app still
 *   serves from board-resolving Tauri commands (board lists, entity
 *   listings, UI state, view definitions, undo state, fuzzy entity
 *   search, mention autocomplete). These will migrate in a follow-up
 *   stage; the guardrail lists them explicitly so adding a new one
 *   to the production source has to be a conscious change to this
 *   allow-list.
 *
 * # Algorithm
 *
 * Walk every `.ts` / `.tsx` file under `apps/kanban-app/ui/src/`,
 * extract every `invoke("<name>"` literal, and assert each name is
 * present in {@link ALLOWED_INVOKE_HANDLERS}. Files whose own purpose
 * is to test or document the invoke surface are skipped (their string
 * literals are stubs, not real invocations).
 */

import fs from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";

/** Handlers a production module under `src/` may legitimately invoke. */
const ALLOWED_INVOKE_HANDLERS = new Set<string>([
  // MCP transport bridges — the seam everything migrated onto.
  "command_tool_call",
  "mcp_subscribe",

  // Legacy unified command dispatcher — used internally by
  // `mcp-transport.ts`'s `callCommandTool` for the `execute command` verb
  // (the in-process Command service still routes through it). Other call
  // sites must use the `useDispatchCommand` hook, which funnels here.
  "dispatch_command",

  // AppHandle-bound natives — these need an ambient Tauri AppHandle and
  // have no equivalent on the MCP wire today.
  // (`show_context_menu` migrated to the `window` MCP server's
  // `show context menu` op — the native NSMenu now mounts behind the
  // AppHandle-backed `WindowShell` seam, so it is no longer allow-listed.)
  "save_dropped_file", // HTML5-drop bytes → temp file → attachment path
  "ai_start_agent", // spawns the AI agent WebSocket bridge
  "ai_list_models", // queries the in-process AI agent registry
  "ai_set_streaming", // toggles AI agent stream mode on the host

  // Multi-board data feeds — still board-resolving Tauri commands
  // because each one threads through `resolve_handle(board_path)`. The
  // entity MCP server is per-task-local-scoped; the board enumeration
  // surface has no MCP equivalent yet. These migrate in a follow-up
  // stage.
  "list_open_boards",
  "get_board_data",
  "list_entities",
  "list_entity_types",
  "get_entity_schema",
  "list_views",
  "search_entities", // entity fuzzy search
  "search_mentions", // mention autocomplete (kept separate from search)

  // UI-state + undo: served by Tauri commands that read the shared
  // AppState fields without going through the MCP transport.
  "get_ui_state",
  "get_undo_state",
]);

/** Files whose `invoke("name"` literals are documentation, not calls. */
const ALLOWLISTED_FILE_BASENAMES = new Set<string>([
  // The grep test itself contains the literal names of allow-listed
  // handlers — those are not real invocations.
  "no-direct-invoke.node.test.ts",
]);

/** The repository's UI source root. */
const UI_SRC_ROOT = path.resolve(__dirname, "..");

/** Recursively walk `dir`, yielding every `.ts` / `.tsx` file. */
function* walkSources(dir: string): Generator<string> {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      yield* walkSources(full);
    } else if (
      entry.isFile() &&
      (entry.name.endsWith(".ts") || entry.name.endsWith(".tsx"))
    ) {
      yield full;
    }
  }
}

/**
 * Extract every `invoke("<name>"` literal from `source`.
 *
 * Matches both `invoke("foo"` and `invoke<T>("foo"` shapes; the regex
 * intentionally accepts a leading generic `<...>` so the type-parameterized
 * sites (e.g. `invoke<EntitySchema>("get_entity_schema", …)`) are caught.
 *
 * Returns an empty array for `// invoke("…")` lines and `*` doc-comment
 * lines so reference-only mentions in JSDoc are not flagged. Matching is
 * line-based; a line whose first non-whitespace characters are `//` or
 * `*` is ignored.
 */
function extractInvokeHandlerNames(source: string): string[] {
  const matches: string[] = [];
  // Allow optional `<...>` generics between `invoke` and `(`.
  const re = /\binvoke(?:\s*<[^>]+>)?\s*\(\s*"([^"]+)"/g;
  for (const line of source.split("\n")) {
    const trimmed = line.trimStart();
    if (trimmed.startsWith("//") || trimmed.startsWith("*")) continue;
    let m: RegExpExecArray | null;
    re.lastIndex = 0;
    while ((m = re.exec(line)) !== null) {
      matches.push(m[1]);
    }
  }
  return matches;
}

/**
 * The production source tree must call `invoke()` only for the
 * documented allow-list (transport bridges + AppHandle-bound natives +
 * still-Tauri data feeds).
 *
 * The set of allowed handlers is small and frozen by review — a new
 * direct-invoke call site forces a conscious decision: either migrate
 * the new handler to an MCP server (the default path after Stage 3) or
 * extend the allow-list with a justification.
 */
describe("no direct invoke() of non-MCP-transport handlers", () => {
  it("every invoke() call in apps/kanban-app/ui/src/ targets an allow-listed handler", () => {
    const offenders: Array<{ file: string; handler: string }> = [];

    for (const filePath of walkSources(UI_SRC_ROOT)) {
      const baseName = path.basename(filePath);
      if (ALLOWLISTED_FILE_BASENAMES.has(baseName)) continue;
      // Test files (`*.test.ts(x)`) routinely set up `invoke()` stubs
      // returning canned responses, so their `invoke("…")` literals are
      // not real invocations from the React tree. They are skipped here;
      // production code lives in non-`.test` files and is the only
      // surface this guardrail polices.
      if (
        baseName.endsWith(".test.ts") ||
        baseName.endsWith(".test.tsx") ||
        baseName.endsWith(".node.test.ts") ||
        baseName.endsWith(".node.test.tsx")
      ) {
        continue;
      }

      const source = fs.readFileSync(filePath, "utf8");
      for (const handler of extractInvokeHandlerNames(source)) {
        if (!ALLOWED_INVOKE_HANDLERS.has(handler)) {
          offenders.push({
            file: path.relative(UI_SRC_ROOT, filePath),
            handler,
          });
        }
      }
    }

    expect(
      offenders,
      `Found direct invoke() calls to non-allow-listed Tauri handlers. ` +
        `Migrate the call to the appropriate MCP server (e.g. \`entity\`, ` +
        `\`focus\`, \`store\`) via callMcpTool/callCommandTool, or — if the ` +
        `handler truly cannot be expressed on MCP — add the handler name to ` +
        `ALLOWED_INVOKE_HANDLERS in this file with an explanatory comment.` +
        `\n\nOffenders:\n${offenders
          .map((o) => `  - ${o.file}: invoke("${o.handler}", …)`)
          .join("\n")}`,
    ).toEqual([]);
  });
});
