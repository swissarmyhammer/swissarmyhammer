// Lifecycle sub-domain — ports `perspective.load`, `perspective.save`,
// `perspective.delete`, `perspective.rename`, `perspective.list` from
// perspective.yaml. CRUD over perspective definitions; each carries the YAML
// metadata 1:1 and makes one `views` MCP call.

import {
  type Availability,
  type CommandContext,
  scopeId,
  unwrapResult,
} from "@swissarmyhammer/plugin";

import {
  type CommandSpec,
  type EntityPerspectiveDispatch,
  type ViewsDispatch,
} from "./context.ts";

/** Build the five lifecycle-sub-domain command registrations. */
export function lifecycleCommands(
  views: ViewsDispatch,
  entity: EntityPerspectiveDispatch,
): CommandSpec[] {
  return [
    // ─── perspective.load ───────────────────────────────────────────────────
    // YAML: param name(args). Routes to views `load perspective`.
    {
      id: "perspective.load",
      name: "Load Perspective",
      params: [{ name: "name", from: "args" }],
      execute: async (rawCtx: unknown) => {
        const ctx = (rawCtx ?? {}) as CommandContext;
        const name = ctx.args?.name;
        return await views.views.views.perspective.load({ name });
      },
    },

    // ─── perspective.save ───────────────────────────────────────────────────
    // YAML: tab_button {icon: plus}, undoable; params name(args, shape text),
    // view_id(scope_chain, entity_type view). Routes to views `save
    // perspective`, threading any other perspective fields (id / view /
    // filter / group) the dispatching surface pre-fills in args.
    //
    // The command result is the op's JSON payload
    // (`{ ok, perspective, entry_id }`), not the raw `CallToolResult` wire
    // envelope — the frontend's `+` flow (`useCreatePerspective` in
    // `perspective-tab-bar.tsx`) reads the created `perspective.id` off the
    // dispatch result to arm inline rename on the new entity. Same
    // `unwrapResult` precedent as `perspective.list` below.
    {
      id: "perspective.save",
      name: "Save Perspective",
      tab_button: { icon: "plus" },
      undoable: true,
      params: [
        { name: "name", from: "args", shape: "text" },
        { name: "view_id", from: "scope_chain", entity_type: "view" },
      ],
      execute: async (rawCtx: unknown) => {
        const ctx = (rawCtx ?? {}) as CommandContext;
        const viewId = scopeId(ctx, "view");
        const args: Record<string, unknown> = { ...(ctx.args ?? {}) };
        if (viewId !== undefined) args.view_id = viewId;
        return unwrapResult(await views.views.views.perspective.save(args));
      },
    },

    // ─── perspective.delete ─────────────────────────────────────────────────
    // YAML: scope entity:perspective, undoable, context_menu; param
    // name(args). Routes to the `entity` server's `delete perspective` (NOT
    // the views server): deleting the ACTIVE perspective must fall the
    // dispatching window's selection back to a survivor, and only the
    // board-bundle `entity` server holds the per-window `UIState` that write
    // needs (same backend split as switch/next/prev — the views server only
    // mutates STORAGE; the selection fallback is the entity server's job).
    //
    // The id is resolved off the `perspective:{id}` scope moniker (the tab's
    // context-menu dispatch) or the `name` arg, and the full `scope_chain` is
    // threaded through so the backend resolves both the perspective target and
    // the `window:<label>` moniker the selection fallback reads.
    {
      id: "perspective.delete",
      name: "Delete Perspective",
      scope: ["entity:perspective"],
      undoable: true,
      context_menu: true,
      params: [{ name: "name", from: "args" }],
      available: (rawCtx: unknown) => {
        const ctx = (rawCtx ?? {}) as CommandContext;
        const id = scopeId(ctx, "perspective") ?? ctx.args?.name;
        if (id === undefined) {
          return {
            ok: false,
            reason: "Select a perspective first",
          } satisfies Availability;
        }
        return { ok: true } satisfies Availability;
      },
      execute: async (rawCtx: unknown) => {
        const ctx = (rawCtx ?? {}) as CommandContext;
        const id = scopeId(ctx, "perspective") ?? ctx.args?.name;
        return await entity.entity.entity.perspective.delete({
          id,
          scope: ctx.scope_chain ?? [],
        });
      },
    },

    // ─── perspective.rename ─────────────────────────────────────────────────
    // YAML: visible:false, undoable; params id(args), new_name(args). Routes
    // to views `rename perspective`.
    {
      id: "perspective.rename",
      name: "Rename Perspective",
      visible: false,
      undoable: true,
      params: [
        { name: "id", from: "args" },
        { name: "new_name", from: "args" },
      ],
      execute: async (rawCtx: unknown) => {
        const ctx = (rawCtx ?? {}) as CommandContext;
        const id = ctx.args?.id;
        const new_name = ctx.args?.new_name;
        return await views.views.views.perspective.rename({ id, new_name });
      },
    },

    // ─── perspective.list ───────────────────────────────────────────────────
    // YAML: visible:false, no params. Routes to views `list perspective`.
    //
    // The command result must be the op's JSON payload, not the raw
    // `CallToolResult` wire envelope of the `views` call: the frontend
    // (`usePerspectivesFetch` in perspective-context.tsx) reads
    // `result.perspectives` off the dispatch result. Returning the envelope
    // verbatim left every window's perspectives state permanently empty —
    // the "Default perspectives gone missing" live regression
    // (01KTY6T1GPY94VYWANE9X41SKJ).
    {
      id: "perspective.list",
      name: "List Perspectives",
      visible: false,
      execute: async () => {
        return unwrapResult(await views.views.views.perspective.list({}));
      },
    },
  ];
}
