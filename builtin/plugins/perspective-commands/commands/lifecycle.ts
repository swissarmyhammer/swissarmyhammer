// Lifecycle sub-domain — ports `perspective.load`, `perspective.save`,
// `perspective.delete`, `perspective.rename`, `perspective.list` from
// perspective.yaml. CRUD over perspective definitions; each carries the YAML
// metadata 1:1 and makes one `views` MCP call.

import {
  type Availability,
  type CommandContext,
  scopeId,
} from "@swissarmyhammer/plugin";

import { type CommandSpec, type ViewsDispatch } from "./context.ts";

/** Build the five lifecycle-sub-domain command registrations. */
export function lifecycleCommands(views: ViewsDispatch): CommandSpec[] {
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
        return await views.views.views.perspective.save(args);
      },
    },

    // ─── perspective.delete ─────────────────────────────────────────────────
    // YAML: scope entity:perspective, undoable, context_menu; param
    // name(args). Routes to views `delete perspective` (which takes an `id`):
    // prefer the in-scope perspective moniker, else the `name` arg.
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
        return await views.views.views.perspective.delete({ id });
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
    {
      id: "perspective.list",
      name: "List Perspectives",
      visible: false,
      execute: async () => {
        return await views.views.views.perspective.list({});
      },
    },
  ];
}
