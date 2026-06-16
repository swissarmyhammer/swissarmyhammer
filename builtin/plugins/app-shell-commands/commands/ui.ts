// UI sub-domain — the former `ui-commands` bundle, folded into
// `app-shell-commands` by the ui.*→app.* rename (mop-up card
// 01KTEBZSVGAZ881RAZZWWZXGPE): there is no `ui.*` command namespace —
// UI-surface commands are app commands. Every command here kept its
// pre-rename MCP routing verbatim; only the ids changed.
//
// The 18 registrations: the ported `ui.yaml` commands (every id now
// `app.*`), the four UI-surface commands Card D moved out of React, the
// three editor drill-in commands Card E moved out of React
// (`UI_SURFACE_COMMANDS` below), and the Card G consolidated
// `entity.inspect` (the global Space inspect command, formerly THREE
// client-side `CommandDef`s).
//
// Backend routing — 11 commands across 2 backends:
//   app.inspect             → ui_state `inspect inspector`   (...inspector.inspect)
//   entity.inspect          → ui_state `inspect inspector`   (...inspector.inspect)
//                             (Card G: target resolved server-side — explicit
//                             ctx.target, else the innermost inspectable
//                             scope-chain moniker, else an inert no-op)
//   app.inspector.close     → ui_state `close inspector`     (...inspector.close)
//   app.inspector.close_all → ui_state `close_all inspector` (...inspector.close_all)
//   app.inspector.set_width → ui_state `set_width inspector` (...inspector.set_width)
//   app.palette.open        → ui_state `open palette`        (...palette.open)
//   app.palette.close       → ui_state `close palette`       (...palette.close)
//   app.entity.startRename  → ui_state `start rename`        (...rename.start)
//   app.mode.set            → ui_state `set keymap`          (...keymap.set)
//   app.setFocus            → ui_state `set scope_chain`     (...scope_chain.set)
//   window.new              → window   `new window`          (...window.new)
//
// Memory `no-client-side-inspect`: `app.inspect` dispatches through the
// backend (`ui_state`) like any other command — there is NO React-side
// shortcut. The plugin merely routes `app.inspect` → ui_state
// `inspect inspector` on the context-menu target moniker. The regression e2e
// asserts this routes via the Command service.
//
// `app.setFocus` records the focus scope chain into `ui_state` via
// `set scope_chain`: the frontend sends the `scope_chain` it already computes
// (leaf-first), and the backend consumes it directly — no separate `fq`. The
// spatial focus KERNEL is still a separate `focus` MCP server
// (`SpatialRegistry` / `SpatialState`); the spatial-nav React layer drives it
// directly through the generic `command_tool_call` bridge, which is why the
// plugin's `load()` still ensures `focus`.

import { type Logger } from "@swissarmyhammer/plugin";

import {
  type CommandContext,
  type CommandSpec,
  type UiStateDispatch,
  type WindowDispatch,
} from "./context.ts";

/** One UI-surface command's identity + metadata. `scope` is the surface's
 * literal marker moniker (`ui:field` / `ui:pressable`).
 *
 * Most of these surfaces are keybinding-only — the four Card D/E drill-ins
 * carry no menu placement (none had one in their React defs). `field.edit` is
 * the exception: "Edit Field" is the ONE command that makes sense to offer on
 * a field from the command palette and context menu (the complement of the
 * sibling task that suppresses delete/archive/unarchive/inspect on fields), so
 * it additionally declares the visible-surface metadata below. The optional
 * fields stay absent for every keybinding-only entry, keeping the data table
 * the single source of truth interpreted by ONE registration `.map`. */
interface UiSurfaceCommandSpec {
  id: string;
  name: string;
  scope: string;
  keys: Record<string, string>;
  /**
   * When true, the command appears in the right-click context menu (and is
   * eligible for the palette). Modelled on `app.inspect`'s richer
   * registration. Absent → keybinding-only (the default for this table).
   */
  context_menu?: boolean;
  /** Context-menu group bucket. Lower groups sort first. */
  context_menu_group?: number;
  /** Sort order within the context-menu group. */
  context_menu_order?: number;
  /**
   * Param schema for a dispatch carrying an explicit target. `field.edit`
   * declares `moniker(target)` exactly like `app.inspect` so a context-menu
   * dispatch threads the right-clicked field's `field:` moniker through to the
   * webview handler, which focuses that target before opening its editor.
   */
  params?: ReadonlyArray<{ name: string; from: string }>;
}

/**
 * The seven UI-surface commands, as a data table (Cards D and E of the
 * ui-command-cleanup project — mirrors the `grid-commands` bundle's
 * `GRID_COMMANDS` pattern).
 *
 * `id` / `name` / `keys` are copied 1:1 from the retired client-side
 * `CommandDef`s: `field.edit` / `field.editEnter` from
 * `apps/kanban-app/ui/src/components/fields/field.tsx`,
 * `pressable.activate` / `pressable.activateSpace` from
 * `apps/kanban-app/ui/src/components/pressable.tsx` (`usePressCommands`),
 * and the Card E editor drill-ins: `filter_editor.drillIn` from
 * `apps/kanban-app/ui/src/components/perspective-tab-bar.tsx`
 * (`FilterFormulaBarFocusable`), `app.ai-panel.composer.drillIn` from
 * `apps/kanban-app/ui/src/components/ai-prompt-composer.tsx`, and
 * `app.ai-panel.elicitation.field.drillIn` from
 * `apps/kanban-app/ui/src/components/ai-elements/elicitation.tsx`
 * (`useFieldDrillIn` — formerly minted per field as `...drillIn:{key}`;
 * now ONE base id, with the per-field variation carried by the focus-gated
 * bus registration rather than N minted command ids).
 *
 * # Why every host `execute` is an inert no-op
 *
 * Each command's effect is pure presentation deep inside the React tree:
 * entering a field's edit mode (or drilling into its pill children via
 * `nav.focus`) and invoking a pressable's local `onPress` closure. The owning
 * component registers a webview-bus handler per id WHILE SPATIAL FOCUS IS
 * WITHIN ITS SUBTREE — the instance's zone itself or a descendant such as a
 * tag pill, matching the keymap's marker-in-chain gate (a pressable is a
 * spatial leaf, so containment degenerates to direct focus); see
 * `apps/kanban-app/ui/src/lib/use-focused-webview-command-handlers.ts`.
 * `useDispatchCommand` runs that handler and skips the
 * backend, exactly like the `grid.*` commands. The host `execute` registered
 * here exists only to satisfy the registration contract and to keep a direct
 * host-side dispatch (e.g. the plugin e2e where no webview is mounted) a
 * harmless success.
 *
 * # Scope gating
 *
 * Unlike the grid's singleton `ui:grid` zone, fields and pressables are
 * many-instance surfaces with dynamic spatial monikers
 * (`field:{type}:{id}.{name}`, arbitrary pressable leaf monikers). Each
 * component therefore mounts a constant MARKER moniker into the command
 * scope chain — a `CommandScopeProvider` with moniker `ui:field` /
 * `ui:pressable` directly above its `<FocusScope>` — and the command's
 * `scope` names that marker. While the focused chain contains the marker,
 * the keys bind via the keymap layer's depth-interleaved chain walk;
 * everywhere else the keys contribute nothing (Enter stays `nav.drillIn`,
 * Space stays `entity.inspect`).
 *
 * None of the four had a menu placement in the React defs, so none carries
 * a `menu` here — the OS menu bar is unchanged.
 */
const UI_SURFACE_COMMANDS: readonly UiSurfaceCommandSpec[] = [
  // ── Field edit-mode entry ─────────────────────────────────────────────────
  // The webview handler unifies "drill into pills" and "open editor": it
  // drills into the focused field zone first and falls through to the
  // component's `onEdit` when the kernel echoes the focused FQM (no spatial
  // children). vim's normal-mode `i` enters insert mode; cua `Enter` shadows
  // the global `nav.drillIn: Enter` only while the `ui:field` marker is in
  // the focused chain (the field zone itself or a pill inside it).
  //
  // `field.edit` also surfaces on the palette + context menu: "Edit Field" is
  // the one command that makes sense on a field. It declares `context_menu`
  // (an "Edit" entry — group 0 — above the entity Cut/Copy/Paste/Inspect
  // groups) and `params: [{ moniker(target) }]`, modelled on `app.inspect`.
  // Gating stays on the `scope: "ui:field"` marker — NOT `applies_to: ["field"]`:
  // a `field:` scope-chain moniker resolves through `focused_entity_type` to its
  // CONTAINING entity (e.g. `task`) for a palette focus, while a `field:`
  // explicit context-menu `target` resolves to `"field"`, so an `applies_to`
  // gate would behave differently across the two surfaces. The scope marker is
  // in the focused chain whenever a field surface is focused, so `list command`
  // offers the row for both palette and context-menu and nowhere else.
  {
    id: "field.edit",
    name: "Edit Field",
    scope: "ui:field",
    keys: { vim: "i", cua: "Enter" },
    context_menu: true,
    context_menu_group: 0,
    context_menu_order: 0,
    params: [{ name: "moniker", from: "target" }],
  },
  // vim parity for the cua `Enter` binding above — lets users press the same
  // key regardless of keymap.
  {
    id: "field.editEnter",
    name: "Edit Field (Enter)",
    scope: "ui:field",
    keys: { vim: "Enter" },
  },
  // ── Pressable activation ──────────────────────────────────────────────────
  // Two separate commands because each `keys` entry is one binding per
  // keymap, and the contract is Enter (vim + cua) AND Space (cua only —
  // Web/CUA convention is both, vim leaves Space free). The webview handler
  // invokes the focused pressable's local `onPress`, short-circuiting when
  // the pressable is disabled.
  {
    id: "pressable.activate",
    name: "Activate",
    scope: "ui:pressable",
    keys: { vim: "Enter", cua: "Enter" },
  },
  {
    id: "pressable.activateSpace",
    name: "Activate (Space)",
    scope: "ui:pressable",
    keys: { cua: "Space" },
  },
  // ── Editor drill-in (Card E) ──────────────────────────────────────────────
  // Enter on a focused editor scope drills DOM focus into the live editor
  // instance. Every keymap binds Enter, shadowing the global
  // `nav.drillIn: Enter` only while the surface's scope moniker is in the
  // focused chain. The webview handler is pure presentation: it calls the
  // owning component's editor-handle `.focus()` and nothing else.
  //
  // The filter formula bar's spatial moniker is dynamic
  // (`filter_editor:{perspectiveId}`), so — like `ui:field` — the component
  // mounts the constant `ui:filter_editor` marker above its `<FocusScope>`.
  {
    id: "filter_editor.drillIn",
    name: "Edit Filter",
    scope: "ui:filter_editor",
    keys: { cua: "Enter", vim: "Enter", emacs: "Enter" },
  },
  // The composer's `<FocusScope>` moniker IS the constant
  // `ui:ai-panel.composer`, so the scope gate names the zone moniker
  // directly — no marker needed.
  {
    id: "app.ai-panel.composer.drillIn",
    name: "Edit Prompt",
    scope: "ui:ai-panel.composer",
    keys: { cua: "Enter", vim: "Enter", emacs: "Enter" },
  },
  // ONE base id for every elicitation text-like field (formerly minted per
  // field as `app.ai-panel.elicitation.field.drillIn:{key}`). The field
  // monikers are dynamic (`ui:ai-panel.elicitation.field:{key}`), so each
  // field mounts the constant `ui:ai-panel.elicitation.field` marker; the
  // per-field variation is carried by the focus-gated bus registration —
  // the focused instance's closure owns its own input ref — NOT by N
  // minted command ids (and not by a dispatch arg: the keymap dispatches
  // bare ids, so an arg could never be supplied on the Enter path).
  {
    id: "app.ai-panel.elicitation.field.drillIn",
    name: "Edit Field",
    scope: "ui:ai-panel.elicitation.field",
    keys: { cua: "Enter", vim: "Enter", emacs: "Enter" },
  },
];

/**
 * Inspectable-entity moniker prefixes — the entity kinds `entity.inspect` /
 * `app.inspect` resolve from a dispatch's scope chain when no explicit
 * target is given. UI chrome (`ui:*`, `perspective_tab:`, `cell:*`,
 * `grid_cell:*`, `row_label:`, `window:`, …) is not inspectable, and
 * `field:` monikers (`field:{type}:{id}.{name}`) are NOT entities: they are
 * projections of their CONTAINING entity, deliberately namespaced so they
 * never masquerade as entity monikers in the scope chain (see the webview's
 * `fieldMoniker` and swissarmyhammer-kanban's `emit_scoped_commands`). A
 * focused field therefore resolves to its containing task — never to the
 * field (kanban card 01KTY6XTJQFCG9ENKTAMC6N3JV). Fields remain inspectable
 * via an explicit `ctx.target` (the double-click `<Inspectable>` route),
 * which always wins verbatim and bypasses this list.
 *
 * Card G moved this filter SERVER-SIDE: it was previously the React-side
 * `INSPECTABLE_ENTITY_PREFIXES` in `app-shell.tsx` (the root-scope Space
 * fallback). The webview's architectural guard
 * (`focus-architecture.guards.node.test.ts`, Guards B + C) pins the same
 * prefix set against the `<Inspectable>` JSX call sites via the shared
 * mirror in `kanban-app/ui/src/test/inspectable-entity-prefixes.ts`, and
 * `ui-plugin-inspectable-prefixes-mirror.spatial.node.test.ts` parses THIS
 * array out of the plugin source and asserts it equals that mirror — drift
 * between the two lists fails the suite, not a code review. The Rust
 * caption renderer (`swissarmyhammer-command-service/src/caption.rs`)
 * carries the third copy — the `{{entity.type}}` caption resolves entity
 * context with the SAME rule, pinned by
 * `tests/inspectable_prefixes_mirror.rs`, so a row's caption ("Inspect
 * Task") and what picking it inspects can never disagree.
 */
const INSPECTABLE_ENTITY_PREFIXES = [
  "task:",
  "tag:",
  "column:",
  "board:",
  "attachment:",
] as const;

/**
 * The entity types the VISIBLE inspect surface (`app.inspect`) declares as its
 * `applies_to` capability set — the INSPECTABLE set.
 *
 * Derived directly from {@link INSPECTABLE_ENTITY_PREFIXES} (the same prefixes
 * `entity.inspect` / `app.inspect` resolve a target from), stripping the
 * trailing `:` of each prefix so a `"task:"` prefix yields the `"task"` entity
 * type. There is no second hand-maintained literal: the inspectable entity-type
 * set is the projection of the inspectable prefix set, so the two can never
 * drift.
 *
 * `list command` lists `app.inspect` only when the focused object's entity type
 * is one of these. The set is DECOUPLED from the subject ops on purpose:
 *
 * - `board` — the board IS here, unlike the subject ops in the `entity-commands`
 *   bundle (cut/copy/delete/archive/unarchive). The root board can never be the
 *   SUBJECT of its own cut/copy/delete, but it CAN be inspected: "Inspect Board"
 *   is a meaningful root-board affordance (`board:` is in the inspectable prefix
 *   set), so Inspect must continue to surface on the root board.
 * - `field` — a `field:{type}:{id}.{name}` moniker is a PROJECTION of its
 *   containing entity, not an entity, so a focused field would otherwise show
 *   a nonsensical "Inspect Field" row. `field:` is deliberately ABSENT from the
 *   inspectable prefix set, so the projection is excluded here too, with NO UI
 *   special-casing.
 *
 * Pinned against the Rust inspectable set (the subject `COPYABLE_ENTITY_TYPES`
 * PLUS `board`) by the drift guard
 * `builtin_app_shell_commands_e2e::assert_inspect_applies_to`, so this list
 * cannot silently diverge from either the subject constant or the
 * board-inspectable intent.
 *
 * NOTE: this is the visible/context-menu surface only. `entity.inspect` (the
 * global Space gesture below) is intentionally UNGATED — it resolves its
 * target server-side via {@link resolveInspectTarget}, which already skips
 * `field:` monikers and lands on the containing entity, so it never inspects a
 * field and needs no `applies_to`.
 */
const INSPECTABLE_ENTITY_TYPES: readonly string[] = INSPECTABLE_ENTITY_PREFIXES.map(
  (prefix) => prefix.slice(0, -1),
);

/**
 * True when `moniker` names an INSPECTABLE entity — its prefix is in
 * {@link INSPECTABLE_ENTITY_PREFIXES} (`task:`/`tag:`/`column:`/`board:`/
 * `attachment:`). Chrome (`ui:*`) and field-projection (`field:`) monikers
 * are NOT inspectable and return false, mirroring the Rust caption renderer's
 * `focused_entity_type` rule so a row's caption and what inspecting it does
 * can never disagree.
 */
function isInspectableMoniker(moniker: string): boolean {
  return INSPECTABLE_ENTITY_PREFIXES.some((prefix) => moniker.startsWith(prefix));
}

/**
 * Resolve the moniker `entity.inspect` / `app.inspect` should inspect.
 *
 * An explicit `ctx.target` wins verbatim ONLY when it is an inspectable-entity
 * moniker (palette result rows, context-menu dispatches on a real entity, and
 * other programmatic dispatches name their entity directly). A NON-inspectable
 * explicit target — a chrome `ui:*` leaf or a `field:` projection — is IGNORED:
 * the toolbar (nav-bar) context menu sets `target` to its innermost scope-chain
 * moniker, which on the toolbar is a `ui:navbar.*` chrome leaf, not the
 * `board:{id}` ancestor the "Inspect Board" caption resolves. Inspecting that
 * chrome leaf no-ops, so we fall through instead — matching the caption renderer
 * (`focused_entity_type`) and the palette path (kanban card
 * 01KV5KYZT9J2BXFJ6H2X782E14).
 *
 * Otherwise the INNERMOST inspectable-entity moniker in the scope chain is
 * the target — the chain is leaf-first, so the first matching entry is the
 * closest enclosing ENTITY of the focused scope (a focused field's
 * `field:…` projection moniker is skipped, so the containing `task:…`
 * wins). Returns `undefined` when neither yields a target (inspect on
 * chrome / no focus) — the command then no-ops (warn-logged by the caller).
 */
function resolveInspectTarget(ctx: CommandContext): string | undefined {
  if (ctx.target !== undefined && isInspectableMoniker(ctx.target)) {
    return ctx.target;
  }
  return (ctx.scope_chain ?? []).find(isInspectableMoniker);
}

/**
 * Build the shared `execute` for the two inspect commands (`app.inspect` /
 * `entity.inspect`): resolve the target via {@link resolveInspectTarget}
 * (explicit `ctx.target`, else the innermost inspectable scope-chain
 * moniker), then route to ui_state `inspect inspector`.
 *
 * A dispatch that resolves NO target returns the inert `{ ok: true }` —
 * inspect on chrome / with no focus must not synthesize a bogus inspect —
 * but is warn-logged: a silent success on resolution failure is exactly how
 * the "palette Inspect does nothing" live bug stayed invisible (kanban card
 * 01KTY6XTJQFCG9ENKTAMC6N3JV — `app.inspect` inspected the empty string).
 */
function buildInspectExecute(
  commandId: string,
  uiState: UiStateDispatch,
  log: Logger,
): (rawCtx: unknown) => Promise<unknown> {
  return async (rawCtx: unknown) => {
    const ctx = (rawCtx ?? {}) as CommandContext;
    const moniker = resolveInspectTarget(ctx);
    if (moniker === undefined) {
      log.warn(
        `${commandId}: no inspectable target resolved — inspect is a no-op`,
        { scope_chain: ctx.scope_chain ?? [], target: ctx.target ?? null },
      );
      return { ok: true };
    }
    return await uiState.ui_state.ui_state.inspector.inspect({
      scope_chain: ctx.scope_chain ?? [],
      moniker,
    });
  };
}

/** Build the 18 ui-origin command registrations (the former `ui-commands`
 * bundle, every id now `app.*` — plus the unrenamed `window.new`,
 * `entity.inspect`, and the webview-bus UI-surface set). `log` is the
 * owning plugin's scoped logger — the inspect commands warn through it when
 * a dispatch resolves no inspectable target. */
export function uiCommands(
  uiState: UiStateDispatch,
  window: WindowDispatch,
  log: Logger,
): CommandSpec[] {
  return [
    // ─── app.inspect ────────────────────────────────────────────────────────
    // ui.yaml: context_menu (group 3, order 0); param moniker(target).
    // Routes to ui_state `inspect inspector` — via the Command service, NOT
    // a React shortcut (memory `no-client-side-inspect`).
    //
    // This is the VISIBLE "Inspect {{entity.type}}" surface (palette row +
    // context-menu entry). Context-menu dispatches carry an explicit target
    // (which wins verbatim); a palette pick carries only the focused scope
    // chain, so the execute resolves the target exactly like
    // `entity.inspect` — the shared `buildInspectExecute` — never the
    // literal `ctx.target ?? ""` (the live bug where a palette pick
    // inspected the empty string, kanban card 01KTY6XTJQFCG9ENKTAMC6N3JV).
    {
      id: "app.inspect",
      name: "Inspect {{entity.type}}",
      context_menu: true,
      context_menu_group: 3,
      context_menu_order: 0,
      // Gate the visible inspect surface to the INSPECTABLE entity types:
      // `list command` suppresses it on a `field:` focus (a field is a
      // projection, not an entity) so no "Inspect Field" row ever appears, but
      // KEEPS it on a `board:` focus — "Inspect Board" is a meaningful root-
      // board affordance. Decoupled from the subject set on purpose.
      applies_to: INSPECTABLE_ENTITY_TYPES,
      params: [{ name: "moniker", from: "target" }],
      execute: buildInspectExecute("app.inspect", uiState, log),
    },

    // ─── entity.inspect ─────────────────────────────────────────────────────
    // Card G: the SINGLE definition of the global Space inspect command,
    // consolidating the three retired client-side `CommandDef`s (the
    // `app-shell.tsx` root fallback, the per-`<Inspectable>` scope def,
    // and the keymap's Space routing). Keys are Space across all three
    // keymaps, GLOBAL (no scope) so the binding lands in the global key
    // table; a focused `<Pressable>`'s scope-gated
    // `pressable.activateSpace` still shadows it through the keymap's
    // chain walk (scope beats global).
    //
    // Target resolution is SERVER-SIDE (`resolveInspectTarget`, shared with
    // `app.inspect` via `buildInspectExecute`): explicit `ctx.target`
    // verbatim, else the innermost inspectable-entity moniker in the scope
    // chain (the chain is derived from the focused FQM, so this replaces
    // the React `INSPECTABLE_ENTITY_PREFIXES` filter), else an inert
    // `{ ok: true }` no-op — Space on chrome / with no focus must not
    // synthesize a bogus inspect (the no-op is warn-logged). The keybinding
    // handler still `preventDefault()`s on the binding match, so Space
    // never falls through to the browser's page scroll.
    //
    // Not palette-visible and no context_menu: `app.inspect` (above) owns
    // the visible "Inspect" affordances; this id owns only the Space
    // gesture and programmatic focus-relative inspects.
    {
      id: "entity.inspect",
      name: "Inspect",
      visible: false,
      undoable: false,
      keys: { vim: "Space", cua: "Space", emacs: "Space" },
      // Intentionally UNGATED (no `applies_to`): unlike the visible
      // `app.inspect` above, this Space gesture resolves its target server-side
      // via `resolveInspectTarget`, which already skips `field:` projection
      // monikers and lands on the containing entity — so Space on a field
      // inspects the task, never the field. Gating it would suppress the
      // gesture on a field focus and break that resolution.
      execute: buildInspectExecute("entity.inspect", uiState, log),
    },

    // ─── app.inspector.close ────────────────────────────────────────────────
    // Routes to ui_state `close inspector`.
    //
    // No longer keyed to cua:Escape (card `01KTPDTH772HSEV5F7R1DKYDNJ`):
    // Escape is owned globally by `nav.drillOut`, which drills out one focus
    // level inside the inspector and, at the inspector layer root, falls
    // through to `ui_state dismiss ui` — a layered close that pops the
    // topmost inspector entry. So Escape still closes the inspector, via
    // drill-out, without `app.inspector.close` competing for the Escape key.
    // The vim `q` binding stays (a direct close), and the inspector's x
    // button keeps dispatching this id via onClick.
    {
      id: "app.inspector.close",
      name: "Close Inspector",
      keys: { vim: "q" },
      execute: async (rawCtx: unknown) => {
        const ctx = (rawCtx ?? {}) as CommandContext;
        return await uiState.ui_state.ui_state.inspector.close({
          scope_chain: ctx.scope_chain ?? [],
        });
      },
    },

    // ─── app.inspector.close_all ────────────────────────────────────────────
    // ui.yaml: keys cua:Mod+Escape / vim:Q. Routes to ui_state
    // `close_all inspector`.
    {
      id: "app.inspector.close_all",
      name: "Close All Inspectors",
      keys: { cua: "Mod+Escape", vim: "Q" },
      execute: async (rawCtx: unknown) => {
        const ctx = (rawCtx ?? {}) as CommandContext;
        return await uiState.ui_state.ui_state.inspector.close_all({
          scope_chain: ctx.scope_chain ?? [],
        });
      },
    },

    // ─── app.inspector.set_width ────────────────────────────────────────────
    // ui.yaml: visible:false, undoable:false; param width(args). Dispatched
    // from the React drag-handle mouseup — no keybinding, no palette entry.
    // Routes to ui_state `set_width inspector`.
    {
      id: "app.inspector.set_width",
      name: "Set Inspector Width",
      visible: false,
      undoable: false,
      params: [{ name: "width", from: "args" }],
      execute: async (rawCtx: unknown) => {
        const ctx = (rawCtx ?? {}) as CommandContext;
        return await uiState.ui_state.ui_state.inspector.set_width({
          scope_chain: ctx.scope_chain ?? [],
          width: ctx.args?.width,
        });
      },
    },

    // ─── app.palette.open ───────────────────────────────────────────────────
    // The former `ui.palette.open` (renamed at move time by Card A). Routing
    // to ui_state `open palette` is unchanged. The `menu:{path:["App"]}`
    // gives the palette its OS-menu affordance (it previously carried keys
    // cua:Mod+K / vim:":" but NO menu, which is why the palette was absent
    // from the native menu bar); group 1 lands it between About (group 0)
    // and Quit (group 2).
    {
      id: "app.palette.open",
      name: "Command Palette",
      keys: { cua: "Mod+K", vim: ":" },
      menu: { path: ["App"], group: 1, order: 0 },
      execute: async (rawCtx: unknown) => {
        const ctx = (rawCtx ?? {}) as CommandContext;
        return await uiState.ui_state.ui_state.palette.open({
          scope_chain: ctx.scope_chain ?? [],
        });
      },
    },

    // ─── app.palette.close ──────────────────────────────────────────────────
    // ui.yaml: visible:false. Routes to ui_state `close palette`.
    {
      id: "app.palette.close",
      name: "Close Palette",
      visible: false,
      execute: async (rawCtx: unknown) => {
        const ctx = (rawCtx ?? {}) as CommandContext;
        return await uiState.ui_state.ui_state.palette.close({
          scope_chain: ctx.scope_chain ?? [],
        });
      },
    },

    // ─── app.entity.startRename ─────────────────────────────────────────────
    // Scope `entity:perspective`; keys cua/vim/emacs all F2 — rename is a
    // DELIBERATE gesture (card 01KTYQY0ZB62KHN6BPK3FBMBD7). Enter on a
    // focused tab is the primary action and ACTIVATES the perspective
    // (`perspective.switch`, via the tab's `nav.drillIn` shadow in
    // `perspective-tab-bar.tsx`); F2 is the platform-wide rename idiom, the
    // double-click on a tab stays as the pointer gesture, and `context_menu`
    // puts a "Rename Perspective" row on the tab's right-click menu. These
    // catalogue keys stay in lockstep with the per-tab React `CommandDef`'s
    // F2 keys (the live binding source — registry scope expressions like
    // `entity:perspective` never literal-match a `perspective:{id}` chain
    // moniker in `extractChainBindings`). The scope filter keeps F2 from
    // claiming a global binding. The command service's `scope` is a list
    // (`Option<Vec<String>>`), so the YAML's single string is passed as a
    // one-element list. Routes to ui_state `start rename` (backend no-op;
    // the frontend intercepts before it reaches the backend).
    {
      id: "app.entity.startRename",
      name: "Rename Perspective",
      scope: ["entity:perspective"],
      context_menu: true,
      keys: { cua: "F2", vim: "F2", emacs: "F2" },
      execute: async (rawCtx: unknown) => {
        const ctx = (rawCtx ?? {}) as CommandContext;
        return await uiState.ui_state.ui_state.rename.start({
          scope_chain: ctx.scope_chain ?? [],
        });
      },
    },

    // ─── app.mode.set ───────────────────────────────────────────────────────
    // ui.yaml: visible:false, undoable:false; param mode(args). Routes to
    // ui_state `set keymap` with the `mode` param.
    {
      id: "app.mode.set",
      name: "Set App Mode",
      visible: false,
      undoable: false,
      params: [{ name: "mode", from: "args" }],
      execute: async (rawCtx: unknown) => {
        const ctx = (rawCtx ?? {}) as CommandContext;
        return await uiState.ui_state.ui_state.keymap.set({
          mode: ctx.args?.mode,
        });
      },
    },

    // ─── app.setFocus ───────────────────────────────────────────────────────
    // ui.yaml: visible:false, undoable:false. Records the focus scope chain
    // into ui_state via `set scope_chain`. The frontend sends `scope_chain`
    // (leaf-first, the leaf is the focus target) on every focus change; the
    // backend consumes that chain directly — there is no separate `fq` to
    // supply. The recorded chain drives command gating's scope fallback and
    // the `scope_chain` UI-state echo the frontend listens for.
    {
      id: "app.setFocus",
      name: "Set Focus",
      visible: false,
      undoable: false,
      execute: async (rawCtx: unknown) => {
        const ctx = (rawCtx ?? {}) as CommandContext;
        const scopeChain = Array.isArray(ctx.args?.scope_chain)
          ? ctx.args.scope_chain
          : [];
        return await uiState.ui_state.ui_state.scope_chain.set({
          scope_chain: scopeChain,
        });
      },
    },

    // ─── window.new ─────────────────────────────────────────────────────────
    // ui.yaml: keys cua/vim/emacs all Mod+Shift+N, menu {path:[Window],
    // group:0, order:0}. Routes to window `new window`. NOT a former `ui.*`
    // command — it keeps `window.new` and its `window` server routing.
    {
      id: "window.new",
      name: "New Window",
      keys: { cua: "Mod+Shift+N", vim: "Mod+Shift+N", emacs: "Mod+Shift+N" },
      menu: { path: ["Window"], group: 0, order: 0 },
      execute: async (rawCtx: unknown) => {
        const ctx = (rawCtx ?? {}) as CommandContext;
        const args: Record<string, unknown> = {};
        const boardPath = ctx.args?.board_path;
        if (typeof boardPath === "string") args.board_path = boardPath;
        return await window.window.window.window.new(args);
      },
    },

    // ─── UI-surface commands (Cards D + E) ──────────────────────────────────
    // field.edit / field.editEnter / pressable.activate /
    // pressable.activateSpace / filter_editor.drillIn /
    // app.ai-panel.composer.drillIn / app.ai-panel.elicitation.field.drillIn
    // from the data table above. Presentation-only:
    // the webview bus handler (registered by the focused field / pressable
    // component) intercepts each id in `useDispatchCommand` before the
    // backend, so the host `execute` is never reached in production. It
    // exists as an inert no-op only to satisfy the registration contract
    // and to keep a direct host-side dispatch a harmless success (mirrors
    // the grid-commands bundle).
    ...UI_SURFACE_COMMANDS.map((spec) => ({
      id: spec.id,
      name: spec.name,
      undoable: false,
      // Gate to the surface's marker moniker: keys apply only while
      // `ui:field` / `ui:pressable` is in the focused scope chain; never
      // lifted into the global key table. The same marker also gates the
      // visible surfaces: `list command` matches a scoped command whenever
      // its marker is anywhere in the focused chain, so a `context_menu`
      // entry surfaces on both the palette and the context menu for a focused
      // field — and nowhere else.
      scope: [spec.scope],
      keys: spec.keys,
      // Visible-surface metadata (only `field.edit` declares it; absent on
      // the keybinding-only entries). Spread so the registration carries the
      // field only when the spec opted in, exactly like `app.inspect`.
      ...(spec.context_menu !== undefined
        ? {
            context_menu: spec.context_menu,
            context_menu_group: spec.context_menu_group,
            context_menu_order: spec.context_menu_order,
          }
        : {}),
      ...(spec.params !== undefined ? { params: spec.params } : {}),
      execute: async () => {
        return { ok: true };
      },
    })),
  ];
}
