/**
 * The webview command handler bus — a reusable seam between plugin-defined
 * commands and the presentation-only behaviors that live in the webview React
 * tree.
 *
 * # Why a module-level registry keyed by command id
 *
 * Every command is DEFINED in a plugin / the Command service (id, name, keys,
 * menu, availability). Most commands carry a backend op, so dispatch routes
 * them through the Command service's `execute command` verb. But a class of
 * commands is inherently presentation — opening an overlay, focusing an
 * editor, toggling a grid cell's edit mode, activating a pressable field.
 * Those have no backend op; their effect lives deep in a React subtree whose
 * live closures cannot be reached from the dispatch path directly.
 *
 * This bus bridges the two: a presentation component registers its live
 * handler here against the plugin command id on mount, and `useDispatchCommand`
 * (see `command-scope.tsx`) consults the bus before the backend — when a
 * dispatched id has a registered handler, it runs the handler and skips the
 * backend call. A registered handler is itself the signal that an id is
 * "handled in webview"; no command metadata needs to mark it.
 *
 * This is the generalization of the AI panel's module-bus (`ai/commands.ts`,
 * `registerAiCommandHandlers` / `triggerAi*`) — the accepted precedent for a
 * command whose effect lives in a sibling subtree. Where `ai/commands.ts`
 * hard-codes its handler keys (`toggle` / `focus` / …) because the AI panel
 * owns a fixed set of `ai.*` commands, this bus is keyed by arbitrary command
 * id so any webview behavior can register against the plugin id it implements.
 * The ownership-guarded cleanup is copied verbatim so HMR / StrictMode
 * double-mounts never leak a stale handler.
 *
 * The bus DEFINES no command — it only routes execution of ids the plugins
 * own.
 *
 * # Handler invariant — presentation only
 *
 * A handler registered here MUST be pure presentation. It may touch live
 * webview state — DOM focus, an editor instance, a grid handle, a cell's
 * edit-mode toggle, a local `onPress` closure — and nothing else. It MUST NOT
 * perform a durable domain mutation directly: no writing to a store, no
 * constructing a `CommandDef` / command object (all command logic lives in
 * Rust), and in particular no reaching the MCP transport (`@/lib/mcp-transport`
 * / `callCommandTool`).
 *
 * When a webview behavior needs a durable effect, it dispatches BACK through
 * `useDispatchCommand` to a plugin command that owns a backend op — the way
 * `grid.deleteRow` re-dispatches `${entity}.archive` and `board.newTask`
 * re-dispatches `entity.addTask`. The bus only sequences presentation around
 * backend ops; it must never become a home for them, or it degrades into a
 * client-side command-logic dumping ground (the failure mode this invariant
 * exists to prevent).
 *
 * This is guarded mechanically: a file that registers a handler here and also
 * imports the MCP transport is the smell, and `webview-command-bus.guard.node
 * .test.ts` fails on it. Every card that moves a behavior onto the bus (C–F)
 * must keep that guard green.
 */

import type { DispatchOptions } from "./command-scope";

/**
 * A webview command handler.
 *
 * Receives the same {@link DispatchOptions} the dispatcher was called with, so
 * a handler that takes per-call arguments (e.g. an overlay-open with a `which`
 * arg) can read them. The return value is propagated back as the dispatch
 * result; most handlers are side-effecting and return nothing.
 */
export type WebviewCommandHandler = (
  opts: DispatchOptions,
) => void | Promise<unknown>;

/**
 * The live handler set, keyed by plugin command id.
 *
 * A slot is present only while the owning component is mounted and has
 * registered it. The dispatch path treats a present slot as "this id is
 * handled in the webview"; an absent slot routes the id to the backend.
 */
const handlers = new Map<string, WebviewCommandHandler>();

/**
 * Register (or replace) the webview handler for a plugin command id.
 *
 * Called by a presentation component as it mounts. A later call for the same
 * id replaces the slot (a remount installs a fresh closure).
 *
 * @param commandId - The plugin command id this handler implements.
 * @param handler - The behavior to run when the id is dispatched.
 * @returns A cleanup function that clears the slot only if this call still
 *   owns it — call it on unmount so a stale closure never lingers, and so a
 *   later registration of the same id (e.g. a StrictMode / HMR remount) is
 *   not wiped by an older cleanup.
 */
export function registerWebviewCommandHandler(
  commandId: string,
  handler: WebviewCommandHandler,
): () => void {
  handlers.set(commandId, handler);
  return () => {
    // Only clear a slot this call still owns — a later registration of the
    // same id (a remount) must not be wiped by an older cleanup.
    if (handlers.get(commandId) === handler) {
      handlers.delete(commandId);
    }
  };
}

/**
 * Look up the registered webview handler for a command id.
 *
 * @param commandId - The plugin command id to resolve.
 * @returns The handler, or `undefined` when the id is not webview-handled.
 */
export function getWebviewCommandHandler(
  commandId: string,
): WebviewCommandHandler | undefined {
  return handlers.get(commandId);
}

/**
 * Whether a command id is currently handled in the webview.
 *
 * The dispatch path consults this before the backend: a `true` result means
 * the id has a registered handler and the backend call is skipped.
 *
 * @param commandId - The plugin command id to test.
 * @returns `true` when a handler is registered for the id.
 */
export function hasWebviewCommandHandler(commandId: string): boolean {
  return handlers.has(commandId);
}

/**
 * Reset the bus to its initial state.
 *
 * Test-only — clears every registration so one test's handlers never leak
 * into the next.
 */
export function resetWebviewCommandBusForTest(): void {
  handlers.clear();
}
