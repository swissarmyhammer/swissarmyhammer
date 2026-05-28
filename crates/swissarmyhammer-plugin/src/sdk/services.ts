// @swissarmyhammer/plugin — SDK helpers for activating host-exposed services.
//
// This file is concatenated into the virtual `@swissarmyhammer/plugin` module
// after `plugin.ts`, so it shares scope with the `Plugin` base class and the
// dispatch primitives. It must not carry top-level `import` statements; symbols
// from `plugin.ts` (notably the `Plugin` type) are already in scope.
//
// ───────────────────────────────────────────────────────────────────────────
// What this file provides
// ───────────────────────────────────────────────────────────────────────────
//
// Two public surfaces:
//
//   1. {@link RUST_MODULE_IDS} — a host-curated table mapping the *public*
//      service name a plugin asks for (`"commands"`, `"window"`, `"app"`, …)
//      to the *internal* Rust module id that name resolves to. This indirection
//      lets the host evolve internal module names without forcing every plugin
//      to be edited; plugins always speak the public name.
//   2. {@link ensureServices} — the conventional helper a plugin's `load()`
//      calls first to register every service it depends on. Idempotent: if
//      another plugin already registered the same `(name, source)` first, the
//      platform's structural-equality dedupe makes the second call a no-op.
//
// ───────────────────────────────────────────────────────────────────────────
// The convention every command-registering plugin follows
// ───────────────────────────────────────────────────────────────────────────
//
//     async load(): Promise<void> {
//       await ensureServices(this, ["commands", "window"]);
//       await registerCommands(this, [
//         { id: "task.move", name: "Move Task", execute: async (ctx) => { ... } },
//         // ...
//       ]);
//     }
//
// `ensureServices` reads each name from {@link RUST_MODULE_IDS}, builds a
// `{ rust: <internal-id> }` source, and calls `plugin.register(name, source)`.
// Multiple plugins calling `ensureServices` with the same name share ONE live
// registration thanks to the registry's idempotent same-`(name, source)`
// merge — only a plugin asking for a name with a DIFFERENT source produces
// the `ServerNameTaken` error.

/**
 * The host-curated table mapping public service names to internal Rust module
 * ids.
 *
 * The *public* name (the key) is what a plugin says in
 * `ensureServices(this, ["<name>"])`. The *internal* id (the value) is the id
 * the host exposed the module under via `PluginHost::expose_rust_module`. This
 * indirection lets the host evolve internal module names without forcing every
 * plugin to be edited; plugins always speak the public name.
 *
 * The table is intentionally small — one entry per host-provided service. New
 * entries are added as new services come online.
 */
export const RUST_MODULE_IDS: Readonly<Record<string, string>> = Object.freeze({
  // NOTE: the public/internal split is structural — entries may diverge as
  // services migrate (host-internal rename, split, or namespace move). Do NOT
  // collapse this map even when every entry's key equals its value today; the
  // indirection is the contract that lets the host evolve internal ids
  // without breaking already-shipped plugins.

  /**
   * The command service: registry of palette/menu/keybinding commands. Exposed
   * under id `"commands"` by `swissarmyhammer-command-service::bootstrap::install_commands_module`.
   */
  commands: "commands",

  /**
   * The kanban operation tool: the in-process board server every kanban-backed
   * builtin command routes to (`move task`, `untag task`, `next task`, …).
   * Exposed under id `"kanban"` by the host's `expose_rust_module` wiring
   * (`apps/kanban-app/src/plugins.rs::expose_kanban_module`, mirrored in the
   * plugin-platform test harness). A builtin plugin asks for it by the public
   * name `"kanban"` and reaches it afterward as `this.kanban...`.
   */
  kanban: "kanban",

  /**
   * The window operation tool: window-manager actions plus the OS file actions
   * (`open path`, `reveal path`) that back `attachment.open` / `attachment.reveal`
   * and the board-file lifecycle verbs. Exposed under id `"window"` by the
   * host's `expose_rust_module` wiring (the kanban app wraps a
   * `swissarmyhammer_window_service::WindowService` in an `InProcessServer`).
   * A builtin plugin asks for it by the public name `"window"` and reaches it
   * as `this.window...`.
   */
  window: "window",

  /**
   * The views operation tool: the perspective/view state mutations the
   * `perspective.*` and `view.set` commands depend on (`set view`, …). Exposed
   * under id `"views"` by the host's `expose_rust_module` wiring (the kanban
   * app wraps a `swissarmyhammer_views::ViewsServer` in an `InProcessServer`).
   * A builtin plugin asks for it by the public name `"views"` and reaches it as
   * `this.views...`.
   */
  views: "views",

  /**
   * The entity operation tool: the generic, type-agnostic CRUD + clipboard
   * face over the entity kernel that the cross-cutting `entity.*` commands
   * (`add entity`, `update field`, `delete entity`, `archive`/`unarchive`,
   * `copy`/`cut`/`paste`) route to. Exposed under id `"entity"` by the host's
   * `expose_rust_module` wiring (the kanban app wraps a
   * `swissarmyhammer_entity_mcp::EntityServer` — clipboard-wired via
   * `with_clipboard` — in an `InProcessServer`). A builtin plugin asks for it
   * by the public name `"entity"` and reaches it as `this.entity...`.
   */
  entity: "entity",
});

/**
 * Raised when {@link ensureServices} is asked to register a name that is not in
 * {@link RUST_MODULE_IDS}.
 *
 * Names in `ensureServices` map through the host-curated table — a plugin
 * asking for a service the host does not provide is a code-side bug (typo,
 * outdated plugin) rather than a runtime condition, so the helper fails fast
 * with this error rather than silently no-op-ing.
 */
export class UnknownService extends Error {
  /** Construct an {@link UnknownService} for the unrecognized service name. */
  constructor(name: string) {
    super(
      `unknown service '${name}': not in the host's service table ` +
        `(known: ${Object.keys(RUST_MODULE_IDS).sort().join(", ")})`,
    );
    this.name = "UnknownService";
  }
}

/**
 * Idempotently register every host-provided service named in `names` on `plugin`.
 *
 * For each entry in `names`, looks up its internal Rust module id in
 * {@link RUST_MODULE_IDS} and calls `plugin.register(name, { rust: <id> })`.
 *
 * ## Idempotency
 *
 * The platform's registry treats two `register(name, source)` calls with the
 * SAME name AND the SAME source as one shared registration (refcount bumped).
 * So a second plugin calling `ensureServices(this, ["commands"])` after the
 * first one already did is a no-op — the same live `commands` server is shared
 * between them, and both plugins' unload triggers a refcount decrement.
 *
 * A second plugin asking for `"commands"` with a DIFFERENT source (impossible
 * via this helper, since the table is the only source of `{ rust }` ids) would
 * surface `ServerNameTaken`. That contract is preserved end to end: the
 * registry rejects mismatched sources, and that rejection propagates through
 * the SDK's `register` to here.
 *
 * ## Why use this helper instead of `plugin.register` directly
 *
 * A plugin author could write `this.register("commands", { rust: "commands" })`
 * by hand, but that hard-codes the internal Rust module id (`"commands"`) into
 * the plugin source. {@link RUST_MODULE_IDS} centralizes that mapping so the
 * host can evolve internal ids without breaking plugins.
 *
 * @param plugin - the plugin instance to register the services on.
 * @param names - the public service names to ensure are registered.
 * @throws {UnknownService} when `names` contains a name not in
 *   {@link RUST_MODULE_IDS}.
 * @throws when the underlying `plugin.register` throws — typically
 *   `ServerNameTaken` on a different-source collision, surfaced verbatim.
 */
export async function ensureServices(
  plugin: Plugin,
  names: readonly string[],
): Promise<void> {
  for (const name of names) {
    const rustId = RUST_MODULE_IDS[name];
    if (rustId === undefined) {
      throw new UnknownService(name);
    }
    // The platform's registry dedupes structurally-equal `(name, source)`
    // pairs, so a second plugin calling this with the same name is a no-op —
    // the live registration is shared. A mismatched source (impossible via
    // this helper, but possible if the host ever introduces collisions)
    // surfaces as `ServerNameTaken` verbatim.
    plugin.register(name, { rust: rustId });
  }
}
