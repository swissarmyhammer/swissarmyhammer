/**
 * Translate the post-Stage-3 MCP transport envelope back to the legacy
 * `(cmd, args)` shape that pre-MCP test mocks were written for.
 *
 * Production code now reaches the focus / entity kernels through
 * `invoke("command_tool_call", { module, tool, op, params })` rather than
 * the deleted `spatial_*` / `get_entity` / `generate_jump_codes` Tauri
 * commands. The kernel side hasn't changed, but the wire shape has —
 * legacy test mocks that key off the old verb names never match.
 *
 * `wrapMcpDispatch` wraps a legacy `(cmd, args) => unknown` dispatcher so
 * that any incoming `command_tool_call` envelope is unwrapped, the
 * `tool` + `op` are mapped to the corresponding legacy command name, the
 * params bag is normalized (snake_case → camelCase fallbacks), the
 * wrapped dispatcher is invoked, and the legacy return value is re-wrapped
 * in the focus / entity server's `{ ok, ... }` envelope so the production
 * wrappers (`focus-mcp.ts`, `entity-mcp.ts`) unwrap it correctly.
 *
 * The translated legacy call is ALSO pushed into the wrapping spy's
 * `mock.calls` array so existing assertions of the form
 * `mockInvoke.mock.calls.filter(c => c[0] === "spatial_focus")` keep
 * working without per-test rewrites.
 */

/**
 * Minimal Vitest spy shape this helper needs — we only push translated
 * legacy calls into `mock.calls` so existing assertions keep matching.
 * Typed loosely (rather than via `vitest`'s `Mock<...>` generic) because
 * the call-site spies are typed `Mock<Procedure | Constructable>` and the
 * narrow `Mock` import would force every caller to assert their spy type.
 */
interface SpyLike {
  mock: { calls: unknown[][] };
}

/** Mapping from focus server op verb → legacy `spatial_*` Tauri command. */
const FOCUS_OP_TO_LEGACY_CMD: Record<string, string> = {
  "set focus": "spatial_focus",
  "clear focus": "spatial_clear_focus",
  "navigate focus": "spatial_navigate",
  "lose focus": "spatial_focus_lost",
  "push layer": "spatial_push_layer",
  "pop layer": "spatial_pop_layer",
  "drill_in layer": "spatial_drill_in",
  "drill_out layer": "spatial_drill_out",
  "generate sneak_codes": "generate_jump_codes",
};

/** Translation of an MCP envelope into the legacy command shape. */
interface TranslatedLegacyCall {
  cmd: string;
  args: Record<string, unknown>;
  tool: string;
  op: string;
}

/**
 * Translate a `command_tool_call` argument bag into the legacy call shape.
 * Returns `null` when the envelope's `{tool,op}` pair is unknown — the
 * caller should fall through to the wrapped dispatcher with the raw
 * `command_tool_call` cmd in that case.
 */
function translate(bag: unknown): TranslatedLegacyCall | null {
  if (!bag || typeof bag !== "object") return null;
  const b = bag as Record<string, unknown>;
  const tool = b.tool as string | undefined;
  const op = b.op as string | undefined;
  const params = (b.params ?? {}) as Record<string, unknown>;
  if (!tool || !op) return null;
  if (tool === "focus" && FOCUS_OP_TO_LEGACY_CMD[op]) {
    // The kernel wire renames `focusedFq` → `focused_fq` for the focus
    // server. Map it back so legacy handlers find the field under its
    // original name.
    const remapped: Record<string, unknown> = { ...params };
    if ("focused_fq" in remapped && !("focusedFq" in remapped)) {
      remapped.focusedFq = remapped.focused_fq;
    }
    return { cmd: FOCUS_OP_TO_LEGACY_CMD[op], args: remapped, tool, op };
  }
  if (tool === "entity" && op === "get entity") {
    return {
      cmd: "get_entity",
      args: {
        entityType: params.type,
        id: params.id,
      },
      tool,
      op,
    };
  }
  if (tool === "window" && op === "list open boards") {
    return { cmd: "list_open_boards", args: {}, tool, op };
  }
  if (tool === "window" && op === "get board data") {
    // The window server wire uses `board_path`; legacy `get_board_data` keys off
    // `boardPath`. Map it back so legacy mocks find their argument.
    const args: Record<string, unknown> = {};
    if (params.board_path !== undefined) args.boardPath = params.board_path;
    return { cmd: "get_board_data", args, tool, op };
  }
  return null;
}

/**
 * Re-wrap a legacy command's raw return value into the focus / entity
 * server's `{ ok, ... }` envelope so the production MCP wrappers
 * (`focus-mcp.ts`, `entity-mcp.ts`) unwrap it correctly.
 */
function rewrap(tool: string, op: string, result: unknown): unknown {
  if (tool === "focus") {
    if (op === "pop layer" || op === "drill_in layer" || op === "drill_out layer") {
      return { ok: true, next_fq: result ?? null };
    }
    if (op === "generate sneak_codes") {
      return { ok: true, codes: result ?? [] };
    }
    return { ok: true, event: null };
  }
  if (tool === "entity" && op === "get entity") {
    return { ok: true, entity: result ?? {} };
  }
  if (tool === "window" && op === "list open boards") {
    // `listOpenBoards` unwraps `result.boards`; legacy mocks return the raw
    // `OpenBoard[]`, so wrap it in the server's `{ ok, boards }` envelope.
    return { ok: true, boards: result ?? [] };
  }
  if (tool === "window" && op === "get board data") {
    // `getBoardData` returns the projection object directly (the server merges
    // `ok` into it), so pass the legacy `BoardDataResponse` through unchanged.
    return result;
  }
  return result;
}

/** A legacy dispatcher signature: `(cmd, args) => unknown | Promise<unknown>`. */
export type LegacyDispatcher = (
  cmd: string,
  args?: unknown,
) => unknown | Promise<unknown>;

/**
 * Wrap a legacy dispatcher so it transparently handles the new MCP
 * envelope. Returns a new dispatcher with the same `(cmd, args)` shape;
 * install it via `mockInvoke.mockImplementation(wrapMcpDispatch(spy, legacy))`.
 *
 * @param spy - The Vitest spy carrying `mock.calls`. The translated
 *   legacy call is pushed into `spy.mock.calls` so existing assertions
 *   keying off legacy verb names keep working.
 * @param legacy - The original `(cmd, args) => unknown` dispatcher.
 */
export function wrapMcpDispatch(
  spy: SpyLike,
  legacy: LegacyDispatcher,
): LegacyDispatcher {
  return async (cmd: string, args?: unknown): Promise<unknown> => {
    if (cmd === "command_tool_call") {
      const translated = translate(args);
      if (translated) {
        // Append the synthetic legacy call to `spy.mock.calls` so the
        // pre-Stage-3 assertions of the form
        // `expect(mockInvoke).toHaveBeenCalledWith("spatial_focus", ...)`
        // match without per-test rewrites. The original
        // `["command_tool_call", envelope]` entry is preserved (Vitest
        // recorded it before the implementation ran) so post-Stage-3
        // tests that assert on the new envelope keep working too.
        //
        // Caveat: filters of the form `c[0] === "spatial_focus"
        // || (c[0] === "command_tool_call" && ...)` will match BOTH
        // entries for a single call (double-count). Such filters must
        // pick one shape; the recommended choice is the `command_tool_call`
        // envelope since that's the actual wire shape post-Stage-3.
        spy.mock.calls.push([translated.cmd, translated.args]);
        const result = await legacy(translated.cmd, translated.args);
        return rewrap(translated.tool, translated.op, result);
      }
    }
    return legacy(cmd, args);
  };
}
