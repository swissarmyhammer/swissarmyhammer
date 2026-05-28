// entity-commands — builtin plugin porting `entity.yaml` (the cross-cutting
// entity CRUD + clipboard commands) to the TypeScript plugin SDK.
//
// Unlike the per-type bundles (`task-commands`, `perspective-commands`), every
// command here is CROSS-CUTTING: its primary param declares `from: target`, so
// it operates on whatever entity the context menu fired over, regardless of
// type. All eight route to the one generic, type-agnostic `entity` MCP server
// (`crates/swissarmyhammer-entity-mcp`) — NOT the domain `kanban` server.
//
// This mirrors the `task-commands` / `kanban-misc-commands` template exactly:
//
//   1. A `Plugin` subclass carries `name` / `description` as descriptive class
//      props (plugin identity is the bundle directory name — `entity-commands`).
//   2. `load()` calls `ensureServices(this, ["commands", "entity"])` FIRST to
//      activate the host services the commands route to, THEN `registerCommands`.
//   3. Each registration carries the FULL UI metadata from `entity.yaml` —
//      `name`, `undoable`, `visible`, `context_menu`, `context_menu_group`,
//      `context_menu_order`, `keys`, `menu`, `params` — 1:1, so the command
//      behaves identically to the YAML-driven version.
//   4. The plugin holds NO business logic: each `execute` makes exactly ONE
//      MCP call into the `entity` server.
//
// Backend routing — all eight target the `entity` server's operation tool
// (verb/noun pairs from `crates/swissarmyhammer-entity-mcp/src/operations.rs`):
//   entity.add          → entity `add entity`      (this.entity.entity.entity.add)
//   entity.update_field → entity `update field`    (this.entity.entity.field.update)
//   entity.delete       → entity `delete entity`   (this.entity.entity.entity.delete)
//   entity.archive      → entity `archive entity`  (this.entity.entity.entity.archive)
//   entity.unarchive    → entity `unarchive entity`(this.entity.entity.entity.unarchive)
//   entity.cut          → entity `cut entity`      (this.entity.entity.entity.cut)
//   entity.copy         → entity `copy entity`     (this.entity.entity.entity.copy)
//   entity.paste        → entity `paste entity`    (this.entity.entity.entity.paste)
//
// Drag-vs-paste: `entity.paste` routes to the entity server's `Paste`, which
// dispatches through the shared `PasteMatrix` (the external/clipboard paste
// path that CREATES). Internal-drag repositioning is a property mutation
// handled elsewhere — never here. The distinction is enforced server-side; the
// plugin only routes `entity.paste` → entity `paste`.

import {
  Plugin,
  ensureServices,
  registerCommands,
  makePluginThis,
} from "@swissarmyhammer/plugin";

// ───────────────────────────────────────────────────────────────────────────
// The command context the host hands every `execute` / `available` callback.
// ───────────────────────────────────────────────────────────────────────────

/**
 * The dispatch context the command service passes a command callback.
 *
 * Mirrors `swissarmyhammer_command_service::CommandContext`: the active scope
 * monikers, the optional context-menu target moniker, and a free-form args
 * bag the dispatching surface populates. A moniker is an `"<entity_type>:<id>"`
 * pair (e.g. `"task:01ABC"`), which is what a YAML `from: target` param
 * resolves against.
 */
interface CommandContext {
  /** Active scope monikers, leaf-last (e.g. `["board:01A", "task:42"]`). */
  scope_chain?: string[];
  /** Context-menu target moniker (the entity the menu fired over). */
  target?: string;
  /** Free-form args bag populated by the dispatching surface. */
  args?: Record<string, unknown>;
}

/** An `available` callback result: ok, or not-ok with a user-facing reason. */
type Availability = { ok: true } | { ok: false; reason: string };

/** A parsed `"<type>:<id>"` moniker. */
interface ParsedMoniker {
  type: string;
  id: string;
}

/**
 * The dispatch surface for the generic `entity` operation tool.
 *
 * The dispatch Proxy turns a property path into an MCP `tools/call`:
 * `server.tool.noun.verb`. For the `entity` server the server name and the
 * single tool name are both `"entity"`, and the noun/verb pairs come straight
 * from `operations.rs` (`add entity`, `update field`, …).
 */
interface EntityDispatch {
  entity: {
    entity: {
      entity: {
        add(args: Record<string, unknown>): Promise<unknown>;
        delete(args: Record<string, unknown>): Promise<unknown>;
        archive(args: Record<string, unknown>): Promise<unknown>;
        unarchive(args: Record<string, unknown>): Promise<unknown>;
        copy(args: Record<string, unknown>): Promise<unknown>;
        cut(args: Record<string, unknown>): Promise<unknown>;
        paste(args: Record<string, unknown>): Promise<unknown>;
      };
      field: {
        update(args: Record<string, unknown>): Promise<unknown>;
      };
    };
  };
}

/**
 * Split a `"<type>:<id>"` target moniker into its `type` / `id` halves.
 *
 * A `from: target` param resolves to the context-menu target moniker. The
 * entity server's per-id ops (`delete` / `archive` / `cut` / …) take `type`
 * and `id` separately, so the plugin splits the moniker at the FIRST colon
 * (ids never contain a colon; the type prefix never does either). Returns
 * `undefined` when there is no target or it is not a `"<type>:<id>"` pair.
 */
function parseTarget(ctx: CommandContext): ParsedMoniker | undefined {
  const target = ctx.target;
  if (target === undefined) return undefined;
  const colon = target.indexOf(":");
  if (colon <= 0 || colon === target.length - 1) return undefined;
  return { type: target.slice(0, colon), id: target.slice(colon + 1) };
}

/** Require a target moniker for an `available` precondition. */
function requireTarget(ctx: CommandContext): Availability {
  if (parseTarget(ctx) === undefined) {
    return { ok: false, reason: "Select an entity first" } satisfies Availability;
  }
  return { ok: true } satisfies Availability;
}

/**
 * The entity-commands builtin plugin.
 *
 * Registers the eight cross-cutting entity commands ported from `entity.yaml`,
 * each wired to the generic `entity` MCP server. Identity is the bundle
 * directory name (`entity-commands`); `name` / `description` are descriptive
 * metadata only.
 */
class EntityCommandsPlugin extends Plugin {
  /** Human-readable name — descriptive metadata only, not plugin identity. */
  readonly name = "Entity Commands";

  /** One-line description — descriptive metadata only. */
  readonly description =
    "Builtin cross-cutting entity commands (add / update-field / delete / archive / unarchive / cut / copy / paste) routed to the generic entity server.";

  /**
   * Activate the services these commands route to, then register the commands.
   *
   * The convention every command-registering plugin follows: `ensureServices`
   * FIRST — so the `commands` registry and the `entity` backend are both live
   * before any registration — then `registerCommands`. The metadata on each
   * registration is `entity.yaml`'s metadata, 1:1.
   */
  async load(): Promise<void> {
    await ensureServices(this, ["commands", "entity"]);

    const entity = this as unknown as EntityDispatch;

    await registerCommands(this, [
      // ─── entity.add ─────────────────────────────────────────────────────
      // YAML: undoable, visible:false; param entity_type(args). Routes to
      // entity `add entity`, taking the entity `type` plus any field map the
      // dispatching surface pre-fills in args.
      {
        id: "entity.add",
        name: "New Entity",
        undoable: true,
        visible: false,
        params: [{ name: "entity_type", from: "args" }],
        execute: async (rawCtx: unknown) => {
          const ctx = (rawCtx ?? {}) as CommandContext;
          const args = ctx.args ?? {};
          const type = args.entity_type;
          const fields = (args.fields ?? {}) as Record<string, unknown>;
          return await entity.entity.entity.entity.add({ type, fields });
        },
      },

      // ─── entity.update_field ────────────────────────────────────────────
      // YAML: undoable, visible:false; params entity_type / id / field_name /
      // value (all args). Routes to entity `update field` ({ type, id, field,
      // value }) — the YAML's `field_name` arg maps to the op's `field`.
      {
        id: "entity.update_field",
        name: "Update Field",
        undoable: true,
        visible: false,
        params: [
          { name: "entity_type", from: "args" },
          { name: "id", from: "args" },
          { name: "field_name", from: "args" },
          { name: "value", from: "args" },
        ],
        execute: async (rawCtx: unknown) => {
          const ctx = (rawCtx ?? {}) as CommandContext;
          const args = ctx.args ?? {};
          return await entity.entity.entity.field.update({
            type: args.entity_type,
            id: args.id,
            field: args.field_name,
            value: args.value,
          });
        },
      },

      // ─── entity.delete ──────────────────────────────────────────────────
      // YAML: undoable, context_menu (group 2, order 0), keys cua:Mod+Backspace;
      // param moniker(target). Routes to entity `delete entity` on the parsed
      // target `type`/`id`.
      {
        id: "entity.delete",
        name: "Delete {{entity.type}}",
        undoable: true,
        context_menu: true,
        context_menu_group: 2,
        context_menu_order: 0,
        keys: { cua: "Mod+Backspace" },
        params: [{ name: "moniker", from: "target" }],
        available: (rawCtx: unknown) => requireTarget((rawCtx ?? {}) as CommandContext),
        execute: async (rawCtx: unknown) => {
          const ctx = (rawCtx ?? {}) as CommandContext;
          const m = parseTarget(ctx)!;
          return await entity.entity.entity.entity.delete({ type: m.type, id: m.id });
        },
      },

      // ─── entity.archive ─────────────────────────────────────────────────
      // YAML: undoable, context_menu (group 2, order 1), keys vim:dd; param
      // moniker(target). Routes to entity `archive entity`.
      {
        id: "entity.archive",
        name: "Archive {{entity.type}}",
        undoable: true,
        context_menu: true,
        context_menu_group: 2,
        context_menu_order: 1,
        keys: { vim: "dd" },
        params: [{ name: "moniker", from: "target" }],
        available: (rawCtx: unknown) => requireTarget((rawCtx ?? {}) as CommandContext),
        execute: async (rawCtx: unknown) => {
          const ctx = (rawCtx ?? {}) as CommandContext;
          const m = parseTarget(ctx)!;
          return await entity.entity.entity.entity.archive({ type: m.type, id: m.id });
        },
      },

      // ─── entity.unarchive ───────────────────────────────────────────────
      // YAML: undoable, context_menu (group 2, order 2); param moniker(target).
      // Routes to entity `unarchive entity`.
      {
        id: "entity.unarchive",
        name: "Unarchive {{entity.type}}",
        undoable: true,
        context_menu: true,
        context_menu_group: 2,
        context_menu_order: 2,
        params: [{ name: "moniker", from: "target" }],
        available: (rawCtx: unknown) => requireTarget((rawCtx ?? {}) as CommandContext),
        execute: async (rawCtx: unknown) => {
          const ctx = (rawCtx ?? {}) as CommandContext;
          const m = parseTarget(ctx)!;
          return await entity.entity.entity.entity.unarchive({ type: m.type, id: m.id });
        },
      },

      // ─── entity.cut ─────────────────────────────────────────────────────
      // YAML: undoable, context_menu (group 1, order 0), keys cua:Mod+X /
      // vim:x, menu {path:[Edit], group:1, order:0}; param moniker(target).
      // Routes to entity `cut entity` — pass the scope chain so a tag /
      // attachment cut can find its owning task in scope.
      {
        id: "entity.cut",
        name: "Cut {{entity.type}}",
        undoable: true,
        context_menu: true,
        context_menu_group: 1,
        context_menu_order: 0,
        keys: { cua: "Mod+X", vim: "x" },
        menu: { path: ["Edit"], group: 1, order: 0 },
        params: [{ name: "moniker", from: "target" }],
        available: (rawCtx: unknown) => requireTarget((rawCtx ?? {}) as CommandContext),
        execute: async (rawCtx: unknown) => {
          const ctx = (rawCtx ?? {}) as CommandContext;
          const m = parseTarget(ctx)!;
          const args: Record<string, unknown> = { type: m.type, id: m.id };
          const scope = ctx.scope_chain ?? [];
          if (scope.length > 0) args.scope = scope;
          return await entity.entity.entity.entity.cut(args);
        },
      },

      // ─── entity.copy ────────────────────────────────────────────────────
      // YAML: undoable:false, context_menu (group 1, order 1), keys cua:Mod+C /
      // vim:y, menu {path:[Edit], group:1, order:1}; param moniker(target).
      // Routes to entity `copy entity` — non-destructive snapshot to clipboard.
      {
        id: "entity.copy",
        name: "Copy {{entity.type}}",
        undoable: false,
        context_menu: true,
        context_menu_group: 1,
        context_menu_order: 1,
        keys: { cua: "Mod+C", vim: "y" },
        menu: { path: ["Edit"], group: 1, order: 1 },
        params: [{ name: "moniker", from: "target" }],
        available: (rawCtx: unknown) => requireTarget((rawCtx ?? {}) as CommandContext),
        execute: async (rawCtx: unknown) => {
          const ctx = (rawCtx ?? {}) as CommandContext;
          const m = parseTarget(ctx)!;
          const args: Record<string, unknown> = { type: m.type, id: m.id };
          const scope = ctx.scope_chain ?? [];
          if (scope.length > 0) args.scope = scope;
          return await entity.entity.entity.entity.copy(args);
        },
      },

      // ─── entity.paste ───────────────────────────────────────────────────
      // YAML: undoable, context_menu (group 1, order 2), keys cua:Mod+V /
      // vim:p, menu {path:[Edit], group:1, order:2}; param moniker(target).
      // Routes to entity `paste entity` — the external/clipboard paste path
      // that CREATES via the shared PasteMatrix (NOT internal-drag mutation).
      // The op takes the destination `target` moniker (verbatim) plus the
      // scope chain for association-shaped paste handlers.
      {
        id: "entity.paste",
        name: "Paste {{entity.type}}",
        undoable: true,
        context_menu: true,
        context_menu_group: 1,
        context_menu_order: 2,
        keys: { cua: "Mod+V", vim: "p" },
        menu: { path: ["Edit"], group: 1, order: 2 },
        params: [{ name: "moniker", from: "target" }],
        available: (rawCtx: unknown) => {
          const ctx = (rawCtx ?? {}) as CommandContext;
          if (ctx.target === undefined) {
            return { ok: false, reason: "Select a paste target first" } satisfies Availability;
          }
          return { ok: true } satisfies Availability;
        },
        execute: async (rawCtx: unknown) => {
          const ctx = (rawCtx ?? {}) as CommandContext;
          const args: Record<string, unknown> = { target: ctx.target };
          const scope = ctx.scope_chain ?? [];
          if (scope.length > 0) args.scope = scope;
          return await entity.entity.entity.entity.paste(args);
        },
      },
    ]);

    this.log.info(
      "entity-commands: registered entity.add, entity.update_field, entity.delete, entity.archive, entity.unarchive, entity.cut, entity.copy, entity.paste",
    );
  }
}

/**
 * The plugin entry point.
 *
 * The host calls this once when the bundle is discovered: build the plugin,
 * wrap it with `makePluginThis` so `this.<server>` dispatch works, and run
 * `load()`.
 *
 * @returns `null` — this plugin's only effect is its load-time registrations.
 */
export async function load(): Promise<unknown> {
  const plugin = makePluginThis(new EntityCommandsPlugin()) as EntityCommandsPlugin;
  await plugin.load();
  return null;
}
