// kanban-misc-commands — builtin plugin porting the four small kanban-domain
// command YAMLs (`column.yaml`, `attachment.yaml`, `tag.yaml`, `view.yaml`) to
// the TypeScript plugin SDK as one bundle of five commands.
//
// This mirrors the `task-commands` template exactly:
//
//   1. A `Plugin` subclass carries `name` / `description` as descriptive class
//      props (plugin identity is the bundle directory name —
//      `kanban-misc-commands`).
//   2. `load()` calls `ensureServices(this, [...])` FIRST to activate every
//      host service the plugin's commands route to (`commands`, `kanban`,
//      `window`, `views`), THEN `registerCommands`.
//   3. Each registration carries the FULL UI metadata from the source YAML —
//      `scope`, `undoable`, `visible`, `context_menu`, `params` — 1:1, so the
//      command behaves identically to the YAML-driven version.
//   4. The plugin holds NO business logic: each `execute` makes exactly ONE
//      MCP call into the backing server, and each `available` only encodes the
//      YAML's preconditions over the command context.
//
// Backend routing (one MCP call per command):
//   column.reorder   → kanban `update column`   (this.kanban.kanban.column.update)
//   tag.update       → kanban `update tag`       (this.kanban.kanban.tag.update)
//   attachment.open  → window `open path`        (this.window.window.path.open)
//   attachment.reveal→ window `reveal path`      (this.window.window.path.reveal)
//   view.set         → views  `set view`         (this.views.views.view.set)

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
 * pair (e.g. `"tag:01ABC"`), which is what a YAML `from: scope_chain` param
 * resolves against.
 */
interface CommandContext {
  /** Active scope monikers, leaf-last (e.g. `["board:01A", "tag:42"]`). */
  scope_chain?: string[];
  /** Context-menu target moniker (the entity the menu fired over). */
  target?: string;
  /** Free-form args bag populated by the dispatching surface. */
  args?: Record<string, unknown>;
}

/** An `available` callback result: ok, or not-ok with a user-facing reason. */
type Availability = { ok: true } | { ok: false; reason: string };

/**
 * Resolve the id of the first scope-chain moniker of `entityType`.
 *
 * A `from: scope_chain` param with `entity_type: <t>` resolves to the id half
 * of the nearest `"<t>:<id>"` moniker in the chain. Returns `undefined` when no
 * such moniker is in scope — the signal an `available` precondition is unmet.
 *
 * For `attachment` monikers the "id" half is the attachment's file path
 * (`attachment:{path}`), which is exactly the value the `window` open / reveal
 * verbs take — mirroring the legacy `AttachmentOpenCmd` / `AttachmentRevealCmd`
 * path resolution.
 */
function scopeId(ctx: CommandContext, entityType: string): string | undefined {
  const prefix = `${entityType}:`;
  // Scope chains are leaf-last; scan from the leaf so the nearest entity wins.
  const chain = ctx.scope_chain ?? [];
  for (let i = chain.length - 1; i >= 0; i -= 1) {
    const moniker = chain[i];
    if (moniker.startsWith(prefix)) {
      return moniker.slice(prefix.length);
    }
  }
  return undefined;
}

/**
 * The kanban-misc-commands builtin plugin.
 *
 * Registers the five kanban-domain commands ported from `column.yaml`,
 * `attachment.yaml`, `tag.yaml`, and `view.yaml`, each wired to its backing
 * MCP server (`kanban`, `window`, or `views`). Identity is the bundle
 * directory name (`kanban-misc-commands`); `name` / `description` are
 * descriptive metadata only.
 */
class KanbanMiscCommandsPlugin extends Plugin {
  /** Human-readable name — descriptive metadata only, not plugin identity. */
  readonly name = "Kanban Misc Commands";

  /** One-line description — descriptive metadata only. */
  readonly description =
    "Builtin kanban-domain commands (column reorder / attachment open+reveal / tag update / view switch) routed to the kanban, window, and views servers.";

  /**
   * Activate the services these commands route to, then register the commands.
   *
   * The convention every command-registering plugin follows: `ensureServices`
   * FIRST — so the `commands` registry and the `kanban` / `window` / `views`
   * backends are all live before any registration — then `registerCommands`.
   * The metadata on each registration is the source YAML's metadata, 1:1.
   */
  async load(): Promise<void> {
    await ensureServices(this, ["commands", "kanban", "window", "views"]);

    await registerCommands(this, [
      // ─── column.reorder ─────────────────────────────────────────────────
      // YAML (column.yaml): undoable, visible:false, params id(args) /
      // target_index(args). No scope. Routes to kanban `update column`,
      // writing the moved column's new order from `target_index`.
      {
        id: "column.reorder",
        name: "Reorder Columns",
        undoable: true,
        visible: false,
        params: [
          { name: "id", from: "args" },
          { name: "target_index", from: "args" },
        ],
        execute: async (rawCtx) => {
          const ctx = (rawCtx ?? {}) as CommandContext;
          const id = ctx.args?.id;
          const order = ctx.args?.target_index;
          return await this.kanban.kanban.column.update({ id, order });
        },
      },

      // ─── attachment.open ────────────────────────────────────────────────
      // YAML (attachment.yaml): scope "attachment", context_menu. The
      // attachment moniker's id half is the file path; route it to the
      // window server's `open path` verb (the relocated `attachment.open`).
      {
        id: "attachment.open",
        name: "Open",
        scope: ["attachment"],
        context_menu: true,
        available: (rawCtx) => {
          const ctx = (rawCtx ?? {}) as CommandContext;
          if (scopeId(ctx, "attachment") === undefined) {
            return { ok: false, reason: "Select an attachment first" } satisfies Availability;
          }
          return { ok: true } satisfies Availability;
        },
        execute: async (rawCtx) => {
          const ctx = (rawCtx ?? {}) as CommandContext;
          const path = scopeId(ctx, "attachment");
          return await this.window.window.path.open({ path });
        },
      },

      // ─── attachment.reveal ──────────────────────────────────────────────
      // YAML (attachment.yaml): scope "attachment", context_menu. Routes to
      // the window server's `reveal path` verb (the relocated
      // `attachment.reveal`).
      {
        id: "attachment.reveal",
        name: "Show in Finder",
        scope: ["attachment"],
        context_menu: true,
        available: (rawCtx) => {
          const ctx = (rawCtx ?? {}) as CommandContext;
          if (scopeId(ctx, "attachment") === undefined) {
            return { ok: false, reason: "Select an attachment first" } satisfies Availability;
          }
          return { ok: true } satisfies Availability;
        },
        execute: async (rawCtx) => {
          const ctx = (rawCtx ?? {}) as CommandContext;
          const path = scopeId(ctx, "attachment");
          return await this.window.window.path.reveal({ path });
        },
      },

      // ─── tag.update ─────────────────────────────────────────────────────
      // YAML (tag.yaml): scope entity:tag, undoable, visible:false, param
      // id(scope_chain, entity_type tag). Routes to kanban `update tag`,
      // threading any tag fields (name / color / description) from args.
      {
        id: "tag.update",
        name: "Update Tag",
        scope: ["entity:tag"],
        undoable: true,
        visible: false,
        params: [{ name: "id", from: "scope_chain", entity_type: "tag" }],
        available: (rawCtx) => {
          const ctx = (rawCtx ?? {}) as CommandContext;
          if (scopeId(ctx, "tag") === undefined) {
            return { ok: false, reason: "Select a tag first" } satisfies Availability;
          }
          return { ok: true } satisfies Availability;
        },
        execute: async (rawCtx) => {
          const ctx = (rawCtx ?? {}) as CommandContext;
          const id = scopeId(ctx, "tag");
          const args: Record<string, unknown> = { ...(ctx.args ?? {}), id };
          return await this.kanban.kanban.tag.update(args);
        },
      },

      // ─── view.set ───────────────────────────────────────────────────────
      // YAML (view.yaml): visible:false, param view_id(args). Routes to the
      // views server's `set view` verb, writing the ViewDef identified by the
      // `view_id` arg the palette pre-fills.
      {
        id: "view.set",
        name: "Switch View",
        visible: false,
        params: [{ name: "view_id", from: "args" }],
        execute: async (rawCtx) => {
          const ctx = (rawCtx ?? {}) as CommandContext;
          const id = ctx.args?.view_id;
          return await this.views.views.view.set({ id });
        },
      },
    ]);

    this.log.info(
      "kanban-misc-commands: registered column.reorder, attachment.open, attachment.reveal, tag.update, view.set",
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
  const plugin = makePluginThis(new KanbanMiscCommandsPlugin()) as KanbanMiscCommandsPlugin;
  await plugin.load();
  return null;
}
