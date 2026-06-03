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

/**
 * Handlers a production module under `src/` may legitimately invoke.
 *
 * Every entry is one of two things, and carries a one-line justification:
 *   - a **transport bridge** (the MCP seam itself), or
 *   - a **documented native exception** — a handler that genuinely cannot be
 *     expressed on the MCP wire (an AppHandle / OS / process primitive, or a
 *     surface that has no MCP server yet and whose migration is a tracked
 *     follow-up, not a trivial op on an already-exposed server).
 *
 * The audit that produced these justifications is task
 * `01KT6R30JGKCFGW0WQWQHN2T1X`; the board-read follow-up is
 * `01KT777JCZBDJPETEXSPAGJDVE`.
 */
const ALLOWED_INVOKE_HANDLERS = new Set<string>([
  // ── Transport bridges — the seam everything migrated onto. ──
  "command_tool_call", // transport: generic `tools/call` MCP request bridge
  "mcp_subscribe", // transport: MCP-notification → Tauri-event pump bootstrap
  "dispatch_command", // transport: legacy unified dispatcher `callCommandTool` lowers `execute command` onto; other sites go through `useDispatchCommand`

  // ── AI panel (ai-panel project territory) — documented natives. ──
  // The in-process ACP agent's lifecycle (registry, loopback WebSocket bridge,
  // AppState-tracked teardown) and its transient availability flag have no
  // clean MCP-wire equivalent; an `ai` MCP server is out of scope here.
  "ai_list_models", // native: queries the in-process AI agent registry (ModelManager::list_agents) — ai-panel territory
  "ai_start_agent", // native: spawns the in-process ACP agent's loopback WebSocket bridge + registers it in AppState for teardown — ai-panel territory
  "ai_set_streaming", // native: flips the transient `#[serde(skip)]` UIState.ai_streaming availability-cache flag (parallel to set_undo_redo_state) — ai-panel territory

  // ── OS / process primitives — documented natives. ──
  // (`show_context_menu` migrated to the `window` MCP server's
  // `show context menu` op behind the AppHandle-backed `WindowShell` seam, so
  // it is no longer allow-listed.)
  "save_dropped_file", // native: receives raw HTML5-dropped bytes (possibly large binary) and writes a temp file — wrong payload for the MCP wire

  // ── Board-management reads — documented natives, follow-up filed. ──
  // Both enumerate / aggregate over the multi-board open set
  // (`state.boards`, `resolve_handle(board_path)`); the `entity` MCP server is
  // per-board-scoped and has no board-enumeration/summary op. Migrating them
  // needs a new board-management server surface, not a trivial op on an
  // exposed server — tracked as `01KT777JCZBDJPETEXSPAGJDVE`.
  "list_open_boards", // native: enumerates the open-board set + active-board marker — needs board-management server (01KT777JCZBDJPETEXSPAGJDVE)
  "get_board_data", // native: per-board aggregate summary across the open-set — needs board-management server (01KT777JCZBDJPETEXSPAGJDVE)

  // ── Still-Tauri data feeds (out of this audit's scope) — natives pending a
  // later migration stage. Each threads through `resolve_handle(board_path)`
  // and has no MCP equivalent yet. ──
  "list_entities", // native: board-resolving entity listing — later migration stage
  "list_entity_types", // native: board-resolving entity-type listing — later migration stage
  "get_entity_schema", // native: board-resolving field+entity schema read — later migration stage
  "list_views", // native: board-resolving view-definition listing — later migration stage
  "search_entities", // native: board-resolving entity fuzzy search — later migration stage
  "search_mentions", // native: board-resolving mention autocomplete — later migration stage

  // ── UI-state + undo — documented natives. ──
  // Served by Tauri commands that read shared AppState fields directly.
  "get_ui_state", // native: reads the shared AppState UIState snapshot on mount — later migration stage
  "get_undo_state", // native: reads the shared AppState undo/redo availability — later migration stage
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
