// Filter sub-domain — ports `perspective.filter`, `perspective.filter.focus`,
// `perspective.clearFilter` from perspective.yaml. Each command carries the
// YAML metadata 1:1.
//
// Backend split (card 01KV0MJYA58GW5PRXGVXWHQK32 — "filter change doesn't
// refresh until click-away/back"): `perspective.filter` (set) is an
// ACTIVATION, not a pure storage write — when the edited perspective is the
// window's active selection, the view must re-filter immediately. So it routes
// to the `entity` server's `filter perspective` op, the board-bundle module
// that holds both the `KanbanContext` and the shared `UIState`: it persists
// the new filter AND recomputes the window's `filtered_task_ids`, returning a
// `{ ok, change }` envelope whose `PerspectiveSwitch` change the host's
// `ui-state-changed` emit unwraps. (The earlier port routed it to the `views`
// server's storage-only `set filter`, which discarded any UIState recompute —
// so a filter edit only re-filtered once a later `perspective.switch` (the
// click-away/back) re-evaluated it, exactly the dead-path bug switch/next/prev
// (01KTYQY0ZB62KHN6BPK3FBMBD7) and delete (01KTYVSA68WDFGXCEJ44T4VFNW) had.)
//
// `perspective.filter.focus` and `perspective.clearFilter` stay on the `views`
// server: focus is a deliberate backend no-op (the formula-bar focus is a
// presentation concern), and clear has its own reconciliation path in
// `filter-editor.tsx`. Only the FILTER SET path drove the live regression.

import {
  type Availability,
  type CommandContext,
  scopeId,
} from "@swissarmyhammer/plugin";

import {
  type CommandSpec,
  type EntityPerspectiveDispatch,
  type ViewsDispatch,
  perspectiveId,
} from "./context.ts";

/** Build the three filter-sub-domain command registrations. */
export function filterCommands(
  views: ViewsDispatch,
  entity: EntityPerspectiveDispatch,
): CommandSpec[] {
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
    // perspective_id(args). Routes to entity `filter perspective` — set the
    // filter AND refresh the dispatching window when it is the active
    // selection (see the backend-split note at the top of this file).
    //
    // Threads `ctx.scope_chain` through as the op's `scope` so the backend
    // resolves the dispatching `window:<label>` moniker (the active-perspective
    // comparison that decides whether to recompute `filtered_task_ids`), and
    // returns the entity call's RAW result (NOT `unwrapResult`) so the
    // `UIStateChange` lands where the host's `ui-state-changed` emit unwraps
    // it: `structuredContent.change` — mirroring `perspective.switch`.
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
        return await entity.entity.entity.perspective.filter({
          perspective_id: id,
          filter,
          scope: ctx.scope_chain ?? [],
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
