// Sort sub-domain — ports `perspective.sort.set`, `perspective.sort.clear`,
// `perspective.sort.toggle` from perspective.yaml. Each carries the YAML
// metadata 1:1 (the grid-only `view_kinds`, the enum-shaped field/direction
// params with their options_from resolvers) and makes one `views` MCP call.

import {
  type CommandContext,
  type CommandSpec,
  type ViewsDispatch,
  type Availability,
  perspectiveId,
  scopeId,
} from "./context.ts";

/** Build the three sort-sub-domain command registrations. */
export function sortCommands(views: ViewsDispatch): CommandSpec[] {
  return [
    // ─── perspective.sort.set ───────────────────────────────────────────────
    // YAML: scope entity:perspective, view_kinds [grid], tab_button
    // {icon: arrow-up-down}, undoable; params field(args, shape enum,
    // options_from perspective.fields), direction(args, shape enum,
    // options_from sort.directions), perspective_id(scope_chain, entity_type
    // perspective). Routes to views `set sort`.
    {
      id: "perspective.sort.set",
      name: "Sort Field",
      scope: ["entity:perspective"],
      view_kinds: ["grid"],
      tab_button: { icon: "arrow-up-down" },
      undoable: true,
      params: [
        { name: "field", from: "args", shape: "enum", options_from: "perspective.fields" },
        { name: "direction", from: "args", shape: "enum", options_from: "sort.directions" },
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
        const field = ctx.args?.field;
        const direction = ctx.args?.direction;
        return await views.views.views.sort.set({
          perspective_id: id,
          field,
          direction,
        });
      },
    },

    // ─── perspective.sort.clear ─────────────────────────────────────────────
    // YAML: scope entity:perspective, view_kinds [grid], undoable,
    // context_menu; param perspective_id(args). Routes to views `clear sort`.
    {
      id: "perspective.sort.clear",
      name: "Clear Sort",
      scope: ["entity:perspective"],
      view_kinds: ["grid"],
      undoable: true,
      context_menu: true,
      params: [{ name: "perspective_id", from: "args" }],
      execute: async (rawCtx: unknown) => {
        const ctx = (rawCtx ?? {}) as CommandContext;
        const id = perspectiveId(ctx);
        return await views.views.views.sort.clear({ perspective_id: id });
      },
    },

    // ─── perspective.sort.toggle ────────────────────────────────────────────
    // YAML: scope entity:perspective, view_kinds [grid], undoable; params
    // field(args), perspective_id(args). Routes to views `toggle sort`.
    {
      id: "perspective.sort.toggle",
      name: "Toggle Sort",
      scope: ["entity:perspective"],
      view_kinds: ["grid"],
      undoable: true,
      params: [
        { name: "field", from: "args" },
        { name: "perspective_id", from: "args" },
      ],
      execute: async (rawCtx: unknown) => {
        const ctx = (rawCtx ?? {}) as CommandContext;
        const id = perspectiveId(ctx);
        const field = ctx.args?.field;
        return await views.views.views.sort.toggle({ perspective_id: id, field });
      },
    },
  ];
}
