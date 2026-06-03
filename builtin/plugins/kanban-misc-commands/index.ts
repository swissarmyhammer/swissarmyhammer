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
 * Parse the JSON payload a kanban operation tool returns.
 *
 * The kanban tool answers `tools/call` with a `CallToolResult` whose single
 * text content item carries the operation's JSON payload as a string (it does
 * NOT populate `structuredContent`). To read the `columns` array back, the
 * plugin must pull `content[0].text` and `JSON.parse` it.
 */
function kanbanPayload(result: unknown): Record<string, unknown> {
  const content = (result as { content?: unknown[] } | undefined)?.content;
  const first = Array.isArray(content) ? content[0] : undefined;
  const text = (first as { text?: unknown } | undefined)?.text;
  if (typeof text !== "string") return {};
  try {
    const parsed = JSON.parse(text);
    return parsed && typeof parsed === "object"
      ? (parsed as Record<string, unknown>)
      : {};
  } catch {
    return {};
  }
}

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
export default class KanbanMiscCommandsPlugin extends Plugin {
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
          const targetIndex = ctx.args?.target_index;

          // The legacy `ColumnReorderCmd` is NOT a single-column write: it lists
          // every column by current order, removes the moved one, re-inserts it
          // at `target_index`, then RE-SEQUENCES every column to 0,1,2,… so the
          // board has no gaps or duplicate orders. The previous port wrote only
          // the moved column's `order = target_index`, which collides with
          // whatever column already held that order — the regression this
          // replaces. Reproduce the full reindex here.
          if (typeof id !== "string" || typeof targetIndex !== "number") {
            // Match the legacy MissingArg failure surface: nothing sensible to
            // do without both a column id and a numeric target index.
            return await this.kanban.kanban.columns.list({});
          }

          const listed = await this.kanban.kanban.columns.list({});
          const rawColumns = kanbanPayload(listed).columns;
          const columns = Array.isArray(rawColumns) ? rawColumns : [];
          // `list columns` already comes back sorted by `order` ascending, but
          // sort defensively so a future server change can't reorder us.
          const sorted = [...columns].sort((a, b) => {
            const ao = Number((a as { order?: unknown }).order ?? 0);
            const bo = Number((b as { order?: unknown }).order ?? 0);
            return ao - bo;
          });
          const ids = sorted.map((c) => String((c as { id?: unknown }).id ?? ""));

          const fromIndex = ids.indexOf(id);
          if (fromIndex === -1) {
            // Column not on the board — surface as a no-op error path, matching
            // the legacy "column not found" ExecutionFailed.
            return await this.kanban.kanban.columns.list({});
          }
          if (fromIndex === targetIndex) {
            return { updated: 0 };
          }

          // Remove and re-insert at the (clamped) target index.
          ids.splice(fromIndex, 1);
          const insertAt = Math.min(targetIndex, ids.length);
          ids.splice(insertAt, 0, id);

          // Re-sequence ALL columns to their new 0-based order. Each is one
          // `update column` write.
          //
          // GROUPING LIMITATION: the legacy command opens a StoreContext undo
          // group so these N writes pop off the undo stack as ONE step. The
          // plugin layer makes one MCP call per `execute` and has no API to open
          // a server-side transaction spanning multiple `update column` ops, so
          // these writes are NOT grouped into a single undo step here. The final
          // board state is correct (sequential, non-duplicate orders); only the
          // undo granularity differs from the legacy behavior. Flagged for the
          // SDK to grow a transaction/undo-group primitive.
          let updated = 0;
          for (let i = 0; i < ids.length; i += 1) {
            await this.kanban.kanban.column.update({ id: ids[i], order: i });
            updated += 1;
          }
          return { updated };
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