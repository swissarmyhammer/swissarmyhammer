// Filter sub-domain — ports `perspective.filter`, `perspective.filter.focus`,
// `perspective.clearFilter` from perspective.yaml. Each command carries the
// YAML metadata 1:1 and makes exactly one `views` MCP call (filter set / focus
// / clear).

import {
  type Availability,
  type CommandContext,
  scopeId,
} from "@swissarmyhammer/plugin";

import {
  type CommandSpec,
  type ViewsDispatch,
  perspectiveId,
} from "./context.ts";

/** Build the three filter-sub-domain command registrations. */
export function filterCommands(views: ViewsDispatch): CommandSpec[] {
  return [
    // ─── perspective.filter.focus ───────────────────────────────────────────
    // YAML: scope entity:perspective, tab_button {icon: filter}; param
    // perspective_id(scope_chain, entity_type perspective). UI-only marker —
    // the backend `focus filter` op is a deliberate no-op, kept for the
    // YAML ↔ Rust completeness invariant.
    {
      id: "perspective.filter.focus",
      name: "Focus Filter",
      scope: ["entity:perspective"],
      tab_button: { icon: "filter" },
      params: [
        {
          name: "perspective_id",
          from: "scope_chain",
          entity_type: "perspective",
        },
      ],
      available: (rawCtx: unknown) => {
        const ctx = (rawCtx ?? {}) as CommandContext;
        if (scopeId(ctx, "perspective") === undefined) {
          return {
            ok: false,
            reason: "Select a perspective first",
          } satisfies Availability;
        }
        return { ok: true } satisfies Availability;
      },
      execute: async (rawCtx: unknown) => {
        const ctx = (rawCtx ?? {}) as CommandContext;
        const id = scopeId(ctx, "perspective");
        return await views.views.views.filter.focus({ perspective_id: id });
      },
    },

    // ─── perspective.filter ─────────────────────────────────────────────────
    // YAML: scope entity:perspective, undoable; params filter(args),
    // perspective_id(args). Routes to views `set filter`.
    {
      id: "perspective.filter",
      name: "Set Filter",
      scope: ["entity:perspective"],
      undoable: true,
      params: [
        { name: "filter", from: "args" },
        { name: "perspective_id", from: "args" },
      ],
      execute: async (rawCtx: unknown) => {
        const ctx = (rawCtx ?? {}) as CommandContext;
        const id = perspectiveId(ctx);
        const filter = ctx.args?.filter;
        return await views.views.views.filter.set({
          perspective_id: id,
          filter,
        });
      },
    },

    // ─── perspective.clearFilter ────────────────────────────────────────────
    // YAML: scope entity:perspective, undoable, context_menu; param
    // perspective_id(args). Routes to views `clear filter`.
    {
      id: "perspective.clearFilter",
      name: "Clear Filter",
      scope: ["entity:perspective"],
      undoable: true,
      context_menu: true,
      params: [{ name: "perspective_id", from: "args" }],
      execute: async (rawCtx: unknown) => {
        const ctx = (rawCtx ?? {}) as CommandContext;
        const id = perspectiveId(ctx);
        return await views.views.views.filter.clear({ perspective_id: id });
      },
    },
  ];
}
