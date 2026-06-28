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
  Availability,
  CommandContext,
  Plugin,
  ensureServices,
  registerCommands,
} from "@swissarmyhammer/plugin";

/** A parsed `"<type>:<id>"` moniker. */
interface ParsedMoniker {
  type: string;
  id: string;
}

/**
 * The entity types that can be the SUBJECT of a clipboard / CRUD operation —
 * the DECLARED CAPABILITY shared by the clipboard pair (`entity.cut` /
 * `entity.copy`) AND the CRUD trio (`entity.delete` / `entity.archive` /
 * `entity.unarchive`). These five commands act ON the focused entity AS the
 * subject (cut/copy/delete/archive/unarchive THIS entity).
 *
 * The command surface (`list command`) lists these five commands only when the
 * focused object's entity type is one of these. The set deliberately excludes:
 *
 * - `board` — the board is the ROOT, so it can never be the subject of its own
 *   cut/copy/delete/archive/unarchive. Right-clicking the root board background
 *   must NOT offer "Cut Board" / "Delete Board" etc. The board IS a valid PASTE
 *   TARGET (clipboard contents drop INTO it) — but that is the opposite
 *   direction, gated by {@link PASTE_TARGET_ENTITY_TYPES}, not this subject set.
 * - `field` — a `field:{type}:{id}.{name}` moniker is a PROJECTION of its
 *   containing entity, not an entity, so a focused field would otherwise show
 *   nonsensical "Delete Field" / "Archive Field" rows. The gate suppresses all
 *   of them, with no UI special-casing.
 * - `view` / `perspective` — they have their own `perspective.*` commands and
 *   no cut/copy or CRUD semantics here.
 *
 * The fine-grained, per-type DISPATCH gating (e.g. cut only deletes
 * task/tag/attachment) stays in the Rust `available()` impls in
 * `swissarmyhammer-kanban::commands::clipboard_commands`; this set is the
 * coarse list-level capability that keeps the commands off types they cannot
 * operate on at all.
 *
 * Mirrors `COPYABLE_ENTITY_TYPES` in
 * `crates/swissarmyhammer-kanban/src/commands/clipboard_commands.rs` — the two
 * lists must stay in lockstep. That lockstep is ENFORCED, not just documented:
 * the drift guard `support::assert_operable_applies_to` loads this plugin,
 * reads each surfaced `applies_to`, and asserts set-equality against the Rust
 * `COPYABLE_ENTITY_TYPES`, so this list and that constant cannot silently
 * diverge. (`app.inspect` in the `app-shell-commands` bundle declares the same
 * subject set, pinned by the matching guard in `builtin_app_shell_commands_e2e`.)
 */
const SUBJECT_OPERABLE_ENTITY_TYPES: readonly string[] = [
  "task",
  "tag",
  "column",
  "actor",
  "project",
  "attachment",
];

/**
 * The entity types that can RECEIVE a paste — the DECLARED CAPABILITY for
 * `entity.paste`. Paste is the OPPOSITE direction from the subject ops: it
 * drops the clipboard contents INTO the target, so the gate is "which entity
 * types can be a paste TARGET", NOT "which can be a subject".
 *
 * Derived from the registered `PasteMatrix` handlers' TARGET side
 * (`crates/swissarmyhammer-kanban/src/commands/paste_handlers/`):
 *
 *   - `task`    — `tag_onto_task`, `attachment_onto_task`, `actor_onto_task`
 *   - `attachment` — `attachment_onto_attachment`
 *   - `board`   — `task_into_board`, `column_into_board`
 *   - `column`  — `task_into_column`
 *   - `project` — `task_into_project`
 *
 * `board` IS here — the board is a legitimate paste target (e.g. dropping a
 * task or column onto an empty board background), which is exactly why
 * `entity.paste` STAYS on the root board even though the subject ops do not.
 *
 * Pinned against the Rust `register_paste_handlers()` target set by the drift
 * guard `support::assert_paste_target_applies_to`, so this list can never
 * silently drift from the handlers that actually exist.
 */
const PASTE_TARGET_ENTITY_TYPES: readonly string[] = [
  "task",
  "attachment",
  "board",
  "column",
  "project",
];

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
    return {
      ok: false,
      reason: "Select an entity first",
    } satisfies Availability;
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
export default class EntityCommandsPlugin extends Plugin {
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
        applies_to: SUBJECT_OPERABLE_ENTITY_TYPES,
        params: [{ name: "moniker", from: "target" }],
        available: (rawCtx: unknown) =>
          requireTarget((rawCtx ?? {}) as CommandContext),
        execute: async (rawCtx: unknown) => {
          const ctx = (rawCtx ?? {}) as CommandContext;
          const m = parseTarget(ctx)!;
          return await entity.entity.entity.entity.delete({
            type: m.type,
            id: m.id,
          });
        },
      },

      // ─── entity.archive ─────────────────────────────────────────────────
      // YAML: undoable, context_menu (group 2, order 1), keys vim:dd; param
      // moniker(target). Routes to entity `archive entity`. The vim binding
      // is the CHORD `d d` (Card J — canonical keystrokes separated by
      // single spaces), migrated from the retired webview SEQUENCE_TABLES.
      {
        id: "entity.archive",
        name: "Archive {{entity.type}}",
        undoable: true,
        context_menu: true,
        context_menu_group: 2,
        context_menu_order: 1,
        keys: { vim: "d d" },
        applies_to: SUBJECT_OPERABLE_ENTITY_TYPES,
        params: [{ name: "moniker", from: "target" }],
        available: (rawCtx: unknown) =>
          requireTarget((rawCtx ?? {}) as CommandContext),
        execute: async (rawCtx: unknown) => {
          const ctx = (rawCtx ?? {}) as CommandContext;
          const m = parseTarget(ctx)!;
          return await entity.entity.entity.entity.archive({
            type: m.type,
            id: m.id,
          });
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
        applies_to: SUBJECT_OPERABLE_ENTITY_TYPES,
        params: [{ name: "moniker", from: "target" }],
        available: (rawCtx: unknown) =>
          requireTarget((rawCtx ?? {}) as CommandContext),
        execute: async (rawCtx: unknown) => {
          const ctx = (rawCtx ?? {}) as CommandContext;
          const m = parseTarget(ctx)!;
          return await entity.entity.entity.entity.unarchive({
            type: m.type,
            id: m.id,
          });
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
        applies_to: SUBJECT_OPERABLE_ENTITY_TYPES,
        params: [{ name: "moniker", from: "target" }],
        available: (rawCtx: unknown) =>
          requireTarget((rawCtx ?? {}) as CommandContext),
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
        applies_to: SUBJECT_OPERABLE_ENTITY_TYPES,
        params: [{ name: "moniker", from: "target" }],
        available: (rawCtx: unknown) =>
          requireTarget((rawCtx ?? {}) as CommandContext),
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
      //
      // Caption is the clipboard-driven plain "Paste" — NOT
      // "Paste {{entity.type}}". Paste is about WHAT IS ON THE CLIPBOARD, not
      // the target entity, so rendering the target type would produce the
      // meaningless "Paste Board" on the root board. The list-time
      // CommandContext carries no clipboard, so a `{{clipboard.type}}` token is
      // not resolvable at this surface; plain "Paste" is the correct caption.
      //
      // Gated by PASTE_TARGET_ENTITY_TYPES (the paste-TARGET capability),
      // which INCLUDES `board` — unlike the subject ops, paste STAYS on the
      // root board because the board is a valid paste target.
      {
        id: "entity.paste",
        name: "Paste",
        undoable: true,
        context_menu: true,
        context_menu_group: 1,
        context_menu_order: 2,
        keys: { cua: "Mod+V", vim: "p" },
        menu: { path: ["Edit"], group: 1, order: 2 },
        applies_to: PASTE_TARGET_ENTITY_TYPES,
        params: [{ name: "moniker", from: "target" }],
        available: (rawCtx: unknown) => {
          const ctx = (rawCtx ?? {}) as CommandContext;
          if (ctx.target === undefined) {
            return {
              ok: false,
              reason: "Select a paste target first",
            } satisfies Availability;
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
