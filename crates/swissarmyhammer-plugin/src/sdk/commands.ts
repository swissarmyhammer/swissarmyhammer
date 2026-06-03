// @swissarmyhammer/plugin — SDK helper for registering palette/menu commands.
//
// This file is concatenated into the virtual `@swissarmyhammer/plugin` module
// after `plugin.ts` and `services.ts`, so it shares scope with the `Plugin`
// base class, the `PluginThis` dispatcher type, and the `ensureServices`
// helper. It must not carry top-level `import` statements; symbols are
// already in scope.
//
// ───────────────────────────────────────────────────────────────────────────
// What this file provides
// ───────────────────────────────────────────────────────────────────────────
//
// One public surface:
//
//   * {@link registerCommands} — loops a list of {@link CommandRegistration}
//     objects through `plugin.commands.command.command.register(...)`. This is
//     the conventional helper a plugin's `load()` calls *after*
//     {@link ensureServices} to install its commands.
//
// `registerCommands` is intentionally a thin wrapper: the underlying call is
// the SDK's operation-tool path form (`commands.command.command.register`),
// which goes through `tools/call("command", { op: "register command", ... })`.
// The helper exists purely to codify the convention every command-registering
// plugin follows — looping the registration list, awaiting each result,
// returning the responses — so plugin authors do not write the same loop by
// hand in every bundle.
//
// ───────────────────────────────────────────────────────────────────────────
// Auto-cleanup via the per-plugin ledger
// ───────────────────────────────────────────────────────────────────────────
//
// Registrations made through this helper are auto-purged on plugin unload by
// the platform's per-plugin ledger:
//
//   * The command service's lifecycle hook fires on every successful
//     `register command` and appends a purge opaque to the plugin's ledger.
//     When the plugin unloads, the ledger drains; the opaque calls back into
//     the service and removes every command this plugin registered.
//   * Each registration's `execute`/`available` function value is marshalled
//     to a `$callback` marker by the SDK before crossing the host boundary;
//     the host's `tools_call` envelope handler records every marker id in the
//     ledger as a callback handle, so the isolate's callback table is also
//     drained on unload.
//
// A plugin therefore never needs an `unload()` body to undo its commands —
// the convention is self-cleaning.

/**
 * One registration entry for {@link registerCommands}.
 *
 * Mirrors the fields the `register command` operation accepts (see
 * `swissarmyhammer-command-service::operations::RegisterCommand`). Every field
 * except `id`, `name`, and `execute` is optional and defaults to the absent
 * behavior the service defines.
 *
 * Function-valued fields (`execute`, `available`) are marshalled across the
 * host/plugin boundary by the SDK's callback primitive: each function is
 * replaced with an opaque `{ $callback: "cb_..." }` marker and stored in the
 * isolate's callback table. The host later invokes the callback by sending
 * the id back through `notifications/callbacks/invoke`.
 */
export interface CommandRegistration {
  /** Stable command id, e.g. `"task.move"`. Must be non-empty. */
  id: string;
  /** Human-readable name (palette / menu label). */
  name: string;
  /** Optional display name override for native menus. Falls back to {@link name}. */
  menu_name?: string;
  /** Optional long-form description (palette detail row, tooltip). */
  description?: string;
  /** Optional category for grouping (e.g. `"Cleanup"`, `"Navigation"`). */
  category?: string;
  /** Scope expression list (e.g. `["entity:task"]`). Empty / absent means global. */
  scope?: readonly string[];
  /** Keybindings keyed by keymap mode (e.g. `vim`, `cua`, `emacs`). */
  keys?: Record<string, string>;
  /** Native menu-bar placement payload. */
  menu?: unknown;
  /** Whether this command appears in the right-click context menu. */
  context_menu?: boolean;
  /** Context-menu group bucket (commands with the same group render contiguously). */
  context_menu_group?: number;
  /** Sort order within the same context-menu group. */
  context_menu_order?: number;
  /** Tab-button affordance payload. */
  tab_button?: unknown;
  /** View-kind UI-surface filter (e.g. `["grid"]`). */
  view_kinds?: readonly string[];
  /** Whether the command produces an undoable change. Defaults to `false`. */
  undoable?: boolean;
  /** Whether the command appears in palettes / menus. Defaults to `true`. */
  visible?: boolean;
  /** Param definitions. None or empty means the command takes no args. */
  params?: readonly unknown[];
  /**
   * Optional `available` callback. Returns whether the command can currently
   * run. Absent means the command is always available.
   */
  available?: (...args: unknown[]) => unknown;
  /** Required `execute` callback. Runs the command's effect. */
  execute: (...args: unknown[]) => unknown;
}

/**
 * The dispatch context the command service passes a command callback.
 *
 * Mirrors `swissarmyhammer_command_service::CommandContext` (the wire contract
 * the host serialises into each `available` / `execute` invocation): the active
 * scope monikers, the optional context-menu target moniker, and a free-form
 * args bag the dispatching surface populates. A moniker is an
 * `"<entity_type>:<id>"` pair (e.g. `"task:01ABC"`), which is what a YAML
 * `from: scope_chain` / `from: target` param resolves against.
 *
 * Every field is optional: the Rust struct serialises with
 * `skip_serializing_if` for each, so an empty context arrives as `{}`. Callbacks
 * conventionally coalesce the raw value to `{}` before reading it
 * (`(rawCtx ?? {}) as CommandContext`).
 */
export interface CommandContext {
  /** Active scope monikers, leaf-last (e.g. `["board:01A", "task:42"]`). */
  scope_chain?: string[];
  /** Context-menu target moniker (the entity the menu fired over). */
  target?: string;
  /** Free-form args bag populated by the dispatching surface. */
  args?: Record<string, unknown>;
}

/**
 * The result an `available` callback may return.
 *
 * The command service contracts `available` as synchronous and accepts the
 * full shape its `interpret_available` resolver understands
 * (`swissarmyhammer-command-service::service::interpret_available`):
 *
 *   * a bare `boolean` — `true` is available, `false` is unavailable with the
 *     service's canonical default reason;
 *   * `{ ok: true }` — available (an object missing `ok` is also treated as
 *     available, but `{ ok: true }` is the explicit form);
 *   * `{ ok: false; reason: string }` — unavailable, the `reason` surfaced to
 *     callers (palette tooltips on grayed-out entries).
 *
 * Returning nothing (an absent `available` callback) means always available.
 */
export type Availability =
  | boolean
  | { ok: true }
  | { ok: false; reason: string };

/**
 * Register every command in `commands` on `plugin` through the command service.
 *
 * For each entry, dispatches the SDK's operation-tool path form
 * `plugin.commands.command.command.register(...)` — which resolves to
 * `tools/call("command", { op: "register command", ... })` against the
 * registered `commands` server. The helper awaits every dispatch sequentially
 * and returns the array of results in registration order, so callers can
 * observe each registration's success.
 *
 * ## Precondition
 *
 * `plugin` must have the `commands` server registered before this is called —
 * conventionally by an `await ensureServices(plugin, ["commands"])` earlier in
 * the same `load()`. If `commands` is not registered, the underlying
 * `plugin.commands.command.command.register(...)` raises `UnknownServer` from
 * the dispatch Proxy.
 *
 * ## Auto-cleanup
 *
 * Every registration is paired with an automatic purge: when the plugin
 * unloads, the command service's per-plugin lifecycle hook removes every
 * command this plugin registered, and the SDK's callback marshalling places
 * each function-valued field's id in the plugin's ledger so the isolate's
 * callback table is also drained. A plugin never needs an `unload()` body to
 * undo what this helper did.
 *
 * @param plugin - the plugin instance to register the commands on; must have
 *   the `commands` server already registered (see {@link ensureServices}).
 * @param commands - the registrations to install, in order.
 * @returns the array of host responses to each `register command` dispatch,
 *   in the same order as `commands`.
 * @throws whatever the underlying `tools/call` throws — typically
 *   `UnknownServer` if `commands` was not registered first, or a host
 *   validation error if a registration is malformed (e.g. empty `id`).
 */
export async function registerCommands<T extends Plugin>(
  plugin: PluginThis<T>,
  commands: readonly CommandRegistration[],
): Promise<unknown[]> {
  const results: unknown[] = [];
  for (const command of commands) {
    // The path form goes through the dispatch Proxy:
    //   commands → server `commands`
    //   command  → tool `command`
    //   command  → operation noun `command`
    //   register → operation verb `register`
    // which resolves through the tool's `_meta` to op `"register command"`.
    // Function values inside the registration object (`execute`, optionally
    // `available`) are marshalled to `$callback` markers by the SDK's
    // callback primitive before the call leaves the isolate.
    const result = await plugin.commands.command.command.register(
      command as unknown as Record<string, unknown>,
    );
    results.push(result);
  }
  return results;
}
