// Navigation sub-domain — ports `perspective.next`, `perspective.prev`,
// `perspective.goto`, `perspective.switch` from perspective.yaml. Each carries
// the YAML metadata 1:1 (keybindings on next/prev, visible:false on goto/
// switch) and makes one `views` MCP call.
//
// The YAML params are named `view_kind` / `view_id` / `id`; the views ops name
// the corresponding fields `view` / `view_id` / `id` (+ `current`). The
// `execute` closures translate the YAML arg names onto the op field names; the
// registered `params` metadata stays the YAML's names so the port is 1:1.

import { type CommandContext } from "@swissarmyhammer/plugin";

import { type CommandSpec, type ViewsDispatch } from "./context.ts";

/** Build the four nav-sub-domain command registrations. */
export function navCommands(views: ViewsDispatch): CommandSpec[] {
  return [
    // ─── perspective.next ───────────────────────────────────────────────────
    // YAML: keys cua Mod+] / vim gt; params view_kind(args), view_id(args).
    // Routes to views `next perspective`.
    {
      id: "perspective.next",
      name: "Next Perspective",
      keys: { cua: "Mod+]", vim: "gt" },
      params: [
        { name: "view_kind", from: "args" },
        { name: "view_id", from: "args" },
      ],
      execute: async (rawCtx: unknown) => {
        const ctx = (rawCtx ?? {}) as CommandContext;
        const args: Record<string, unknown> = {};
        if (ctx.args?.view_kind !== undefined) args.view = ctx.args.view_kind;
        if (ctx.args?.view_id !== undefined) args.view_id = ctx.args.view_id;
        return await views.views.views.perspective.next(args);
      },
    },

    // ─── perspective.prev ───────────────────────────────────────────────────
    // YAML: keys cua Mod+[ / vim gT; params view_kind(args), view_id(args).
    // Routes to views `prev perspective`.
    {
      id: "perspective.prev",
      name: "Previous Perspective",
      keys: { cua: "Mod+[", vim: "gT" },
      params: [
        { name: "view_kind", from: "args" },
        { name: "view_id", from: "args" },
      ],
      execute: async (rawCtx: unknown) => {
        const ctx = (rawCtx ?? {}) as CommandContext;
        const args: Record<string, unknown> = {};
        if (ctx.args?.view_kind !== undefined) args.view = ctx.args.view_kind;
        if (ctx.args?.view_id !== undefined) args.view_id = ctx.args.view_id;
        return await views.views.views.perspective.prev(args);
      },
    },

    // ─── perspective.goto ───────────────────────────────────────────────────
    // YAML: visible:false; params id(args), view_kind(args), view_id(args).
    // Routes to views `goto perspective`.
    {
      id: "perspective.goto",
      name: "Go to Perspective",
      visible: false,
      params: [
        { name: "id", from: "args" },
        { name: "view_kind", from: "args" },
        { name: "view_id", from: "args" },
      ],
      execute: async (rawCtx: unknown) => {
        const ctx = (rawCtx ?? {}) as CommandContext;
        const args: Record<string, unknown> = { id: ctx.args?.id };
        if (ctx.args?.view_kind !== undefined) args.view = ctx.args.view_kind;
        if (ctx.args?.view_id !== undefined) args.view_id = ctx.args.view_id;
        return await views.views.views.perspective.goto(args);
      },
    },

    // ─── perspective.switch ─────────────────────────────────────────────────
    // YAML: visible:false; param perspective_id(args). Routes to views
    // `switch perspective`.
    {
      id: "perspective.switch",
      name: "Switch Perspective",
      visible: false,
      params: [{ name: "perspective_id", from: "args" }],
      execute: async (rawCtx: unknown) => {
        const ctx = (rawCtx ?? {}) as CommandContext;
        const perspective_id = ctx.args?.perspective_id;
        return await views.views.views.perspective.switch({ perspective_id });
      },
    },
  ];
}
