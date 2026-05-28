// Group sub-domain — ports `perspective.group`, `perspective.clearGroup` from
// perspective.yaml. Each command carries the YAML metadata 1:1 (the `group`
// param's enum shape / options_from / clear_command annotations survive
// verbatim) and makes exactly one `views` MCP call.

import {
  type CommandContext,
  type CommandSpec,
  type ViewsDispatch,
  type Availability,
  perspectiveId,
  scopeId,
} from "./context.ts";

/** Build the two group-sub-domain command registrations. */
export function groupCommands(views: ViewsDispatch): CommandSpec[] {
  return [
    // ─── perspective.group ──────────────────────────────────────────────────
    // YAML: scope entity:perspective, tab_button {icon: group}, undoable;
    // params group(args, shape enum, options_from perspective.fields,
    // clear_command perspective.clearGroup), perspective_id(scope_chain,
    // entity_type perspective). Routes to views `set group`.
    {
      id: "perspective.group",
      name: "Group By",
      scope: ["entity:perspective"],
      tab_button: { icon: "group" },
      undoable: true,
      params: [
        {
          name: "group",
          from: "args",
          shape: "enum",
          options_from: "perspective.fields",
          clear_command: "perspective.clearGroup",
        },
        { name: "perspective_id", from: "scope_chain", entity_type: "perspective" },
      ],
      available: (rawCtx: unknown) => {
        const ctx = (rawCtx ?? {}) as CommandContext;
        if (scopeId(ctx, "perspective") === undefined) {
          return { ok: false, reason: "Select a perspective first" } satisfies Availability;
        }
        return { ok: true } satisfies Availability;
      },
      execute: async (rawCtx: unknown) => {
        const ctx = (rawCtx ?? {}) as CommandContext;
        const id = perspectiveId(ctx);
        const group = ctx.args?.group;
        return await views.views.views.group.set({ perspective_id: id, group });
      },
    },

    // ─── perspective.clearGroup ─────────────────────────────────────────────
    // YAML: scope entity:perspective, undoable, context_menu; param
    // perspective_id(args). Routes to views `clear group`.
    {
      id: "perspective.clearGroup",
      name: "Clear Group",
      scope: ["entity:perspective"],
      undoable: true,
      context_menu: true,
      params: [{ name: "perspective_id", from: "args" }],
      execute: async (rawCtx: unknown) => {
        const ctx = (rawCtx ?? {}) as CommandContext;
        const id = perspectiveId(ctx);
        return await views.views.views.group.clear({ perspective_id: id });
      },
    },
  ];
}
