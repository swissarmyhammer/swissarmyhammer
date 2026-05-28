/**
 * The frontend's single seam onto the in-process MCP `command` tool.
 *
 * Every user-invocable command in the app flows through the Command MCP
 * service (`crates/swissarmyhammer-command-service`). The service is exposed
 * as an in-process MCP server (`tools/call("command", { op, ... })`) at host
 * bootstrap. This module is the one place the React tree reaches that tool, so
 * `useDispatchCommand` and `useCommandList` never construct a raw `tools/call`
 * payload themselves — they call {@link callCommandTool} and let this module
 * own the wire shape.
 *
 * ## Why a bridge, not a JS MCP client
 *
 * The kanban-app UI has no in-process JS MCP client — the host runs the MCP
 * server in Rust. The frontend reaches it through the Tauri bridge command
 * `command_tool_call`, which forwards `{ op, params }` to
 * `tools/call("command", { op, ...params })` on the host's in-process server
 * and returns the structured result. Keeping that crossing isolated here means:
 *
 *   - The call shape (`tools/call("command", { op: "execute command", ... })`)
 *     lives in exactly one module and is asserted by the hook unit tests via a
 *     mock of this module.
 *   - If the host later exposes a direct JS MCP transport, only this file
 *     changes — every hook keeps calling {@link callCommandTool}.
 *
 * The `commands/changed` notification stream is likewise surfaced through the
 * Tauri event of the same name; {@link subscribeCommandsChanged} wraps the
 * `listen(...)` so subscribers do not import the Tauri event API directly.
 */

import { invoke } from "@tauri-apps/api/core";

/** The MCP tool name for the Command service. */
export const COMMAND_TOOL = "command" as const;

/** The Tauri event the host raises for a debounced `commands/changed`. */
export const COMMANDS_CHANGED_EVENT = "notifications/commands/changed" as const;

/**
 * Execution context forwarded to a command's registered callback.
 *
 * Mirrors `swissarmyhammer-command-service`'s `CommandContext`: the active
 * scope chain (innermost → root monikers), an optional context-menu target,
 * and the free-form args bag the dispatching surface populated. The Command
 * service forwards this to the registered callback unchanged — the callback
 * (in the plugin / host) reads whatever it needs.
 */
export interface CommandContext {
  /** Active scope monikers, innermost first (e.g. `["task:abc", "window:main"]`). */
  scope_chain: string[];
  /** Context-menu / dispatch target moniker (the entity the action fires over). */
  target?: string;
  /** Free-form args bag populated by the dispatching surface. */
  args?: Record<string, unknown>;
}

/**
 * Perform `tools/call("command", { op, ...params })` against the in-process
 * Command MCP server.
 *
 * `op` is the verb-noun string the operation tool dispatches on (e.g.
 * `"execute command"`, `"list command"`). `params` is the rest of the
 * operation payload (`id`, `ctx`, `scope`, …). Conceptually this is one MCP
 * arguments object: `tools/call("command", { op, ...params })`.
 *
 * ## Lowering onto the existing Tauri commands (transitional adapter)
 *
 * The kanban-app UI has no in-process JS MCP client — the host runs the MCP
 * server in Rust. Until the host exposes a generic `command_tool_call` Tauri
 * bridge, this function lowers the two verbs the frontend uses onto the
 * existing Tauri commands, which the downstream Rust work rewires to sit on
 * top of the Command service:
 *
 *   - `execute command` → `invoke("dispatch_command", { cmd, target, args,
 *     scopeChain, boardPath })` — the unified dispatcher entry point.
 *
 * Every other verb (e.g. `list command`, `available command`) routes through
 * the generic `command_tool_call` Tauri bridge, which the host exposes for the
 * Command service; those verbs have no legacy Tauri equivalent to lower onto.
 *
 * The seam is what matters: callers express the Command-service verb shape and
 * never construct a Tauri payload themselves, so when the host adds a real MCP
 * bridge for `execute command` too, only this function changes. The hook unit
 * tests mock this module and assert the verb shape directly, independent of
 * the lowering below.
 *
 * @returns The tool's structured result, typed by the caller.
 */
export async function callCommandTool<T = unknown>(
  op: string,
  params: Record<string, unknown> = {},
): Promise<T> {
  if (op === EXECUTE_COMMAND_OP) {
    return invoke<T>("dispatch_command", lowerExecuteCommand(params));
  }
  return invoke<T>("command_tool_call", { tool: COMMAND_TOOL, op, params });
}

/** Verb constant for `execute command` (kept here so callers stay declarative). */
export const EXECUTE_COMMAND_OP = "execute command" as const;
/** Verb constant for `list command`. */
export const LIST_COMMAND_OP = "list command" as const;

/** `ctx` payload shape carried by an `execute command` call. */
interface ExecuteCommandParams {
  id: string;
  ctx?: CommandContext;
  board_path?: string;
}

/**
 * Lower an `execute command` payload onto the `dispatch_command` Tauri
 * argument shape (`cmd` / `target` / `args` / `scopeChain` / `boardPath`).
 *
 * `board_path` is omitted from the result when absent so the dispatcher's
 * optional-arg handling (and the existing tests' exact-match assertions)
 * stay intact.
 */
function lowerExecuteCommand(
  params: Record<string, unknown>,
): Record<string, unknown> {
  const { id, ctx, board_path } = params as unknown as ExecuteCommandParams;
  const context = ctx ?? { scope_chain: [] };
  return {
    cmd: id,
    target: context.target,
    args: context.args,
    scopeChain: context.scope_chain ?? [],
    ...(board_path ? { boardPath: board_path } : {}),
  };
}

/**
 * Subscribe to the host's debounced `commands/changed` notification.
 *
 * The Command service coalesces registry mutations and emits a single
 * `commands/changed` after a ~100ms quiet window (see the service's
 * `ChangeNotifier`); the host re-broadcasts it as the Tauri event
 * {@link COMMANDS_CHANGED_EVENT}. Subscribers (e.g. `useCommandList`) re-fetch
 * their slice of the registry when the callback fires.
 *
 * @returns A promise resolving to an unsubscribe function.
 */
export function subscribeCommandsChanged(
  onChanged: () => void,
): Promise<() => void> {
  // `@tauri-apps/api/event` is imported lazily so this module's static graph
  // does not pull it in (and, transitively, `transformCallback` from
  // `@tauri-apps/api/core`). Modules that only need `callCommandTool` — e.g.
  // `command-scope` — must not drag the event API into tests that mock
  // `@tauri-apps/api/core` without re-exporting `transformCallback`.
  return import("@tauri-apps/api/event").then(({ listen }) =>
    listen(COMMANDS_CHANGED_EVENT, () => onChanged()),
  );
}
