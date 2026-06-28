// Navigation sub-domain — ports `perspective.next`, `perspective.prev`,
// `perspective.goto`, `perspective.switch` from perspective.yaml. Each carries
// the YAML metadata 1:1 and makes one MCP call.
//
// Backend split (card 01KTYQY0ZB62KHN6BPK3FBMBD7 — "perspectives can't be
// SELECTED"): `goto` still routes to the `views` server, which only RESOLVES
// a perspective. `switch` / `next` / `prev` are ACTIVATIONS — they must
// evaluate the target's filter and write the dispatching window's
// `active_perspective_id` + `filtered_task_ids` — so they route to the
// `entity` server's perspective ops, the board-bundle module that holds both
// the `KanbanContext` and the shared `UIState`. (The earlier port routed all
// four to `views`, which made clicking a tab a no-op: the resolution result
// was discarded and no UIState write ever happened.)
//
// Each activation execute passes `ctx.scope_chain` through as the op's
// `scope` so the backend resolves the dispatching `window:<label>` moniker
// (per-window state, no silent main fallback) and — for next/prev — the
// `view:{id}` moniker that scopes which perspectives are visible to cycle.
//
// The activation executes return the entity call's raw `CallToolResult`
// (NOT `unwrapResult`) so the `UIStateChange` lands where the host's
// `ui-state-changed` emit unwraps it: `structuredContent.change` — the same
// envelope contract `view.set` rides via the ui_state server.

import { type CommandContext } from "@swissarmyhammer/plugin";

import {
  type CommandSpec,
  type EntityPerspectiveDispatch,
  type ViewsDispatch,
} from "./context.ts";

/** Build the four nav-sub-domain command registrations. */
export function navCommands(
  views: ViewsDispatch,
  entity: EntityPerspectiveDispatch,
): CommandSpec[] {
  return [
    // ─── perspective.next ───────────────────────────────────────────────────
    // YAML: keys cua Mod+] / vim gt; params view_kind(args), view_id(args).
    // Routes to entity `next perspective` — cycle forward through the
    // window's visible perspectives (wrapping; no-op when fewer than two
    // match) and ACTIVATE the target. The vim binding is the CHORD `g t`
    // (Card J — canonical keystrokes separated by single spaces), migrated
    // from the retired webview SEQUENCE_TABLES. Menu: the View menu's
    // perspective-cycling group, alongside the AI-panel toggle's View
    // placement convention.
    {
      id: "perspective.next",
      name: "Next Perspective",
      keys: { cua: "Mod+]", vim: "g t" },
      menu: { path: ["View"], group: 1, order: 0 },
      params: [
        { name: "view_kind", from: "args" },
        { name: "view_id", from: "args" },
      ],
      execute: async (rawCtx: unknown) => {
        const ctx = (rawCtx ?? {}) as CommandContext;
        const args: Record<string, unknown> = {
          scope: ctx.scope_chain ?? [],
        };
        if (ctx.args?.view_kind !== undefined)
          args.view_kind = ctx.args.view_kind;
        if (ctx.args?.view_id !== undefined) args.view_id = ctx.args.view_id;
        return await entity.entity.entity.perspective.next(args);
      },
    },

    // ─── perspective.prev ───────────────────────────────────────────────────
    // YAML: keys cua Mod+[ / vim gT; params view_kind(args), view_id(args).
    // Routes to entity `prev perspective` — the reverse cycle, same
    // activation semantics. The vim binding is the CHORD `g Shift+T` — the
    // canonical chord form of the old `gT` (its second step carries the
    // Shift modifier).
    {
      id: "perspective.prev",
      name: "Previous Perspective",
      keys: { cua: "Mod+[", vim: "g Shift+T" },
      menu: { path: ["View"], group: 1, order: 1 },
      params: [
        { name: "view_kind", from: "args" },
        { name: "view_id", from: "args" },
      ],
      execute: async (rawCtx: unknown) => {
        const ctx = (rawCtx ?? {}) as CommandContext;
        const args: Record<string, unknown> = {
          scope: ctx.scope_chain ?? [],
        };
        if (ctx.args?.view_kind !== undefined)
          args.view_kind = ctx.args.view_kind;
        if (ctx.args?.view_id !== undefined) args.view_id = ctx.args.view_id;
        return await entity.entity.entity.perspective.prev(args);
      },
    },

    // ─── perspective.goto ───────────────────────────────────────────────────
    // YAML: visible:false; params id(args), view_kind(args), view_id(args).
    // Routes to views `goto perspective` — pure RESOLUTION (look up by id,
    // optionally validate the view), no activation.
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
    // YAML: visible:false; param perspective_id(args). Routes to entity
    // `switch perspective` — the canonical ACTIVATION: tab clicks, Enter on
    // a focused tab, and the palette's "Switch to Perspective …" rows all
    // dispatch this id.
    {
      id: "perspective.switch",
      name: "Switch Perspective",
      visible: false,
      params: [{ name: "perspective_id", from: "args" }],
      execute: async (rawCtx: unknown) => {
        const ctx = (rawCtx ?? {}) as CommandContext;
        const perspective_id = ctx.args?.perspective_id;
        return await entity.entity.entity.perspective.switch({
          perspective_id,
          scope: ctx.scope_chain ?? [],
        });
      },
    },
  ];
}
