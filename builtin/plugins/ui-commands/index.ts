// ui-commands — builtin plugin porting `ui.yaml` (10 commands) to the
// TypeScript plugin SDK (the last builtin-commands port), plus the four
// UI-surface commands Card D moved out of React and the three editor
// drill-in commands Card E moved out of React (`UI_SURFACE_COMMANDS`
// below): `field.edit` / `field.editEnter` / `pressable.activate` /
// `pressable.activateSpace` / `filter_editor.drillIn` /
// `ui.ai-panel.composer.drillIn` / `ui.ai-panel.elicitation.field.drillIn`
// — plus the Card G consolidated `entity.inspect` (the global Space inspect
// command, formerly THREE client-side `CommandDef`s) — 18 commands total.
//
// Like `app-shell-commands`, this bundle fans out across MULTIPLE backends by
// concern — but here the three backends are `ui_state`, `focus`, and `window`:
//
//   1. A `Plugin` subclass carries `name` / `description` as descriptive class
//      props (plugin identity is the bundle directory name — `ui-commands`).
//   2. `load()` calls `ensureServices(this, ["commands", "ui_state", "window",
//      "focus"])` FIRST — so the `commands` registry and all three backends are
//      live before any registration — THEN `registerCommands`.
//   3. Each registration carries the FULL UI metadata from `ui.yaml` — `name`,
//      `keys`, `menu`, `scope`, `context_menu*`, `visible`, `undoable`,
//      `params` — 1:1, so each command behaves identically to the YAML-driven
//      version.
//   4. The plugin holds NO business logic: each `execute` makes exactly ONE
//      MCP call into its backend.
//
// Backend routing — 11 commands across 3 backends:
//   ui.inspect            → ui_state `inspect inspector`   (...inspector.inspect)
//   entity.inspect        → ui_state `inspect inspector`   (...inspector.inspect)
//                           (Card G: target resolved server-side — explicit
//                           ctx.target, else the innermost inspectable
//                           scope-chain moniker, else an inert no-op)
//   ui.inspector.close    → ui_state `close inspector`     (...inspector.close)
//   ui.inspector.close_all→ ui_state `close_all inspector` (...inspector.close_all)
//   ui.inspector.set_width→ ui_state `set_width inspector` (...inspector.set_width)
//   app.palette.open      → ui_state `open palette`        (...palette.open)
//   ui.palette.close      → ui_state `close palette`       (...palette.close)
//   ui.entity.startRename → ui_state `start rename`        (...rename.start)
//   ui.mode.set           → ui_state `set keymap`          (...keymap.set)
//   ui.setFocus           → ui_state `set scope_chain`     (...scope_chain.set)
//   window.new            → window   `new window`          (...window.new)
//
// Memory `no-client-side-inspect`: `ui.inspect` dispatches through the backend
// (`ui_state`) like any other command — there is NO React-side shortcut. The
// plugin merely routes `ui.inspect` → ui_state `inspect inspector` on the
// context-menu target moniker. The regression e2e asserts this routes via the
// Command service.
//
// `ui.setFocus` records the focus scope chain into `ui_state` via
// `set scope_chain`: the frontend sends the `scope_chain` it already computes
// (leaf-first), and the backend consumes it directly — no separate `fq`. The
// spatial focus KERNEL is still a separate `focus` MCP server (`SpatialRegistry`
// / `SpatialState`); the spatial-nav React layer drives it directly through the
// generic `command_tool_call` bridge, which is why `focus` is still ensured
// above.

import {
  CommandContext,
  Plugin,
  ensureServices,
  registerCommands,
} from "@swissarmyhammer/plugin";

/** One UI-surface command's identity + metadata. `scope` is the surface's
 * literal marker moniker (`ui:field` / `ui:pressable`). */
interface UiSurfaceCommandSpec {
  id: string;
  name: string;
  scope: string;
  keys: Record<string, string>;
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
 * (`FilterFormulaBarFocusable`), `ui.ai-panel.composer.drillIn` from
 * `apps/kanban-app/ui/src/components/ai-prompt-composer.tsx`, and
 * `ui.ai-panel.elicitation.field.drillIn` from
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
  {
    id: "field.edit",
    name: "Edit Field",
    scope: "ui:field",
    keys: { vim: "i", cua: "Enter" },
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
    id: "ui.ai-panel.composer.drillIn",
    name: "Edit Prompt",
    scope: "ui:ai-panel.composer",
    keys: { cua: "Enter", vim: "Enter", emacs: "Enter" },
  },
  // ONE base id for every elicitation text-like field (formerly minted per
  // field as `ui.ai-panel.elicitation.field.drillIn:{key}`). The field
  // monikers are dynamic (`ui:ai-panel.elicitation.field:{key}`), so each
  // field mounts the constant `ui:ai-panel.elicitation.field` marker; the
  // per-field variation is carried by the focus-gated bus registration —
  // the focused instance's closure owns its own input ref — NOT by N
  // minted command ids (and not by a dispatch arg: the keymap dispatches
  // bare ids, so an arg could never be supplied on the Enter path).
  {
    id: "ui.ai-panel.elicitation.field.drillIn",
    name: "Edit Field",
    scope: "ui:ai-panel.elicitation.field",
    keys: { cua: "Enter", vim: "Enter", emacs: "Enter" },
  },
];

/**
 * Inspectable-entity moniker prefixes — the entity kinds `entity.inspect`
 * resolves from a dispatch's scope chain when no explicit target is given.
 * UI chrome (`ui:*`, `perspective_tab:`, `cell:*`, `grid_cell:*`,
 * `row_label:`, `window:`, …) is not inspectable.
 *
 * Card G moved this filter SERVER-SIDE: it was previously the React-side
 * `INSPECTABLE_ENTITY_PREFIXES` in `app-shell.tsx` (the root-scope Space
 * fallback). The webview's architectural guard
 * (`focus-architecture.guards.node.test.ts`, Guards B + C) pins the same
 * prefix set against the `<Inspectable>` JSX call sites via the shared
 * mirror in `kanban-app/ui/src/test/inspectable-entity-prefixes.ts`, and
 * `ui-plugin-inspectable-prefixes-mirror.spatial.node.test.ts` parses THIS
 * array out of the plugin source and asserts it equals that mirror — drift
 * between the two lists fails the suite, not a code review.
 */
const INSPECTABLE_ENTITY_PREFIXES = [
  "task:",
  "tag:",
  "column:",
  "board:",
  "field:",
  "attachment:",
] as const;

/**
 * Resolve the moniker `entity.inspect` should inspect.
 *
 * An explicit `ctx.target` wins verbatim (palette result rows and other
 * programmatic dispatches name their entity directly). Otherwise the
 * INNERMOST inspectable-entity moniker in the scope chain is the target —
 * the chain is leaf-first, so the first matching entry is the closest
 * enclosing entity of the focused scope (a focused inspector field resolves
 * to its `field:…` moniker, not the enclosing card's `task:…`). Returns
 * `undefined` when neither yields a target (Space on chrome / no focus) —
 * the command then no-ops.
 */
function resolveInspectTarget(ctx: CommandContext): string | undefined {
  if (ctx.target !== undefined) return ctx.target;
  return (ctx.scope_chain ?? []).find((moniker) =>
    INSPECTABLE_ENTITY_PREFIXES.some((prefix) => moniker.startsWith(prefix)),
  );
}

/**
 * The dispatch surface for the `ui_state` operation tool — the inspector /
 * palette / keymap / rename ops the ui commands route to.
 *
 * The dispatch Proxy turns a property path into an MCP `tools/call`:
 * `server.tool.noun.verb`. For the `ui_state` server the server name and the
 * single tool name are both `"ui_state"`; the noun/verb pairs come straight
 * from `crates/swissarmyhammer-ui-state/src/operations.rs`:
 *   `inspect inspector`   → this.ui_state.ui_state.inspector.inspect
 *   `close inspector`     → this.ui_state.ui_state.inspector.close
 *   `close_all inspector` → this.ui_state.ui_state.inspector.close_all
 *   `set_width inspector` → this.ui_state.ui_state.inspector.set_width
 *   `open palette`        → this.ui_state.ui_state.palette.open
 *   `close palette`       → this.ui_state.ui_state.palette.close
 *   `start rename`        → this.ui_state.ui_state.rename.start
 *   `set keymap`          → this.ui_state.ui_state.keymap.set
 *   `set scope_chain`     → this.ui_state.ui_state.scope_chain.set
 */
interface UiStateDispatch {
  ui_state: {
    ui_state: {
      inspector: {
        inspect(args: Record<string, unknown>): Promise<unknown>;
        close(args: Record<string, unknown>): Promise<unknown>;
        close_all(args: Record<string, unknown>): Promise<unknown>;
        set_width(args: Record<string, unknown>): Promise<unknown>;
      };
      palette: {
        open(args: Record<string, unknown>): Promise<unknown>;
        close(args: Record<string, unknown>): Promise<unknown>;
      };
      rename: {
        start(args: Record<string, unknown>): Promise<unknown>;
      };
      keymap: {
        set(args: Record<string, unknown>): Promise<unknown>;
      };
      scope_chain: {
        set(args: Record<string, unknown>): Promise<unknown>;
      };
    };
  };
}

/**
 * The dispatch surface for the `window` operation tool — the genuine
 * window-manager action `window.new` routes to.
 *
 * Verb/noun pair from `crates/swissarmyhammer-window-service/src/operations.rs`:
 *   `new window` → this.window.window.window.new
 */
interface WindowDispatch {
  window: {
    window: {
      window: {
        new (args: Record<string, unknown>): Promise<unknown>;
      };
    };
  };
}

/**
 * The ui-commands builtin plugin.
 *
 * Registers the ten UI commands ported from `ui.yaml` plus the Card G
 * `entity.inspect`, routed across the `ui_state`, `focus`, and `window` MCP
 * servers, plus the seven webview-bus handled UI-surface commands
 * (`UI_SURFACE_COMMANDS`). Identity is the
 * bundle directory name (`ui-commands`); `name` / `description` are
 * descriptive metadata only.
 */
export default class UiCommandsPlugin extends Plugin {
  /** Human-readable name — descriptive metadata only, not plugin identity. */
  readonly name = "UI Commands";

  /** One-line description — descriptive metadata only. */
  readonly description =
    "Builtin UI commands (inspector open/close, command palette open/close, perspective rename, keymap mode, spatial focus, and new window) routed to the ui_state, focus, and window servers, plus the webview-bus handled field-edit, pressable-activation, and editor drill-in commands.";

  /**
   * Activate the services these commands route to, then register the commands.
   *
   * The convention every command-registering plugin follows: `ensureServices`
   * FIRST — so the `commands` registry and all three backends (`ui_state`,
   * `window`, `focus`) are live before any registration — then
   * `registerCommands`. The metadata on each registration is `ui.yaml`'s
   * metadata, 1:1.
   */
  async load(): Promise<void> {
    // `focus` is ensured (and thereby activated into the live registry) even
    // though no command here routes to it: the spatial-nav React layer reaches
    // the focus kernel through the `focus` MCP module via the generic
    // `command_tool_call` bridge, and module activation is what registers it.
    await ensureServices(this, ["commands", "ui_state", "window", "focus"]);

    const uiState = this as unknown as UiStateDispatch;
    const window = this as unknown as WindowDispatch;

    await registerCommands(this, [
      // ─── ui.inspect ─────────────────────────────────────────────────────
      // ui.yaml: context_menu (group 3, order 0); param moniker(target).
      // Routes to ui_state `inspect inspector` on the context-menu target
      // moniker — via the Command service, NOT a React shortcut
      // (memory `no-client-side-inspect`).
      {
        id: "ui.inspect",
        name: "Inspect {{entity.type}}",
        context_menu: true,
        context_menu_group: 3,
        context_menu_order: 0,
        params: [{ name: "moniker", from: "target" }],
        execute: async (rawCtx: unknown) => {
          const ctx = (rawCtx ?? {}) as CommandContext;
          return await uiState.ui_state.ui_state.inspector.inspect({
            scope_chain: ctx.scope_chain ?? [],
            moniker: ctx.target ?? "",
          });
        },
      },

      // ─── entity.inspect ─────────────────────────────────────────────────
      // Card G: the SINGLE definition of the global Space inspect command,
      // consolidating the three retired client-side `CommandDef`s (the
      // `app-shell.tsx` root fallback, the per-`<Inspectable>` scope def,
      // and the keymap's Space routing). Keys are Space across all three
      // keymaps, GLOBAL (no scope) so the binding lands in the global key
      // table; a focused `<Pressable>`'s scope-gated
      // `pressable.activateSpace` still shadows it through the keymap's
      // chain walk (scope beats global).
      //
      // Target resolution is SERVER-SIDE (`resolveInspectTarget`): explicit
      // `ctx.target` verbatim, else the innermost inspectable-entity moniker
      // in the scope chain (the chain is derived from the focused FQM, so
      // this replaces the React `INSPECTABLE_ENTITY_PREFIXES` filter), else
      // an inert `{ ok: true }` no-op — Space on chrome / with no focus must
      // not synthesize a bogus inspect. The keybinding handler still
      // `preventDefault()`s on the binding match, so Space never falls
      // through to the browser's page scroll.
      //
      // Not palette-visible and no context_menu: `ui.inspect` (above) owns
      // the visible "Inspect" affordances; this id owns only the Space
      // gesture and programmatic focus-relative inspects.
      {
        id: "entity.inspect",
        name: "Inspect",
        visible: false,
        undoable: false,
        keys: { vim: "Space", cua: "Space", emacs: "Space" },
        execute: async (rawCtx: unknown) => {
          const ctx = (rawCtx ?? {}) as CommandContext;
          const moniker = resolveInspectTarget(ctx);
          if (moniker === undefined) {
            return { ok: true };
          }
          return await uiState.ui_state.ui_state.inspector.inspect({
            scope_chain: ctx.scope_chain ?? [],
            moniker,
          });
        },
      },

      // ─── ui.inspector.close ─────────────────────────────────────────────
      // Routes to ui_state `close inspector`.
      //
      // No longer keyed to cua:Escape (card `01KTPDTH772HSEV5F7R1DKYDNJ`):
      // Escape is owned globally by `nav.drillOut`, which drills out one focus
      // level inside the inspector and, at the inspector layer root, falls
      // through to `ui_state dismiss ui` — a layered close that pops the
      // topmost inspector entry. So Escape still closes the inspector, via
      // drill-out, without `ui.inspector.close` competing for the Escape key.
      // The vim `q` binding stays (a direct close), and the inspector's x
      // button keeps dispatching this id via onClick.
      {
        id: "ui.inspector.close",
        name: "Close Inspector",
        keys: { vim: "q" },
        execute: async (rawCtx: unknown) => {
          const ctx = (rawCtx ?? {}) as CommandContext;
          return await uiState.ui_state.ui_state.inspector.close({
            scope_chain: ctx.scope_chain ?? [],
          });
        },
      },

      // ─── ui.inspector.close_all ─────────────────────────────────────────
      // ui.yaml: keys cua:Mod+Escape / vim:Q. Routes to ui_state
      // `close_all inspector`.
      {
        id: "ui.inspector.close_all",
        name: "Close All Inspectors",
        keys: { cua: "Mod+Escape", vim: "Q" },
        execute: async (rawCtx: unknown) => {
          const ctx = (rawCtx ?? {}) as CommandContext;
          return await uiState.ui_state.ui_state.inspector.close_all({
            scope_chain: ctx.scope_chain ?? [],
          });
        },
      },

      // ─── ui.inspector.set_width ─────────────────────────────────────────
      // ui.yaml: visible:false, undoable:false; param width(args). Dispatched
      // from the React drag-handle mouseup — no keybinding, no palette entry.
      // Routes to ui_state `set_width inspector`.
      {
        id: "ui.inspector.set_width",
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

      // ─── app.palette.open ───────────────────────────────────────────────
      // Folds the ui.*→app.* rename: this is the former `ui.palette.open`,
      // now `app.palette.open` (the palette opener IS a `ui.*` command, so it
      // adopts its final `app.*` name at move time). Routing to ui_state
      // `open palette` is unchanged — only the id and the added `menu`
      // placement change. The `menu:{path:["App"]}` gives the palette its OS-
      // menu affordance (it previously carried keys cua:Mod+K / vim:":" but NO
      // menu, which is why the palette was absent from the native menu bar);
      // group 1 lands it between About (group 0) and Quit (group 2).
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

      // ─── ui.palette.close ───────────────────────────────────────────────
      // ui.yaml: visible:false. Routes to ui_state `close palette`.
      {
        id: "ui.palette.close",
        name: "Close Palette",
        visible: false,
        execute: async (rawCtx: unknown) => {
          const ctx = (rawCtx ?? {}) as CommandContext;
          return await uiState.ui_state.ui_state.palette.close({
            scope_chain: ctx.scope_chain ?? [],
          });
        },
      },

      // ─── ui.entity.startRename ──────────────────────────────────────────
      // ui.yaml: scope `entity:perspective`; keys cua/vim/emacs all Enter. The
      // scope filter keeps Enter from claiming nav.drillIn on board/column/card
      // focus. The command service's `scope` is a list (`Option<Vec<String>>`),
      // so the YAML's single string is passed as a one-element list. Routes to
      // ui_state `start rename` (backend no-op; the frontend intercepts before
      // it reaches the backend).
      {
        id: "ui.entity.startRename",
        name: "Rename Perspective",
        scope: ["entity:perspective"],
        keys: { cua: "Enter", vim: "Enter", emacs: "Enter" },
        execute: async (rawCtx: unknown) => {
          const ctx = (rawCtx ?? {}) as CommandContext;
          return await uiState.ui_state.ui_state.rename.start({
            scope_chain: ctx.scope_chain ?? [],
          });
        },
      },

      // ─── ui.mode.set ────────────────────────────────────────────────────
      // ui.yaml: visible:false, undoable:false; param mode(args). Routes to
      // ui_state `set keymap` with the `mode` param.
      {
        id: "ui.mode.set",
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

      // ─── ui.setFocus ────────────────────────────────────────────────────
      // ui.yaml: visible:false, undoable:false. Records the focus scope chain
      // into ui_state via `set scope_chain`. The frontend sends `scope_chain`
      // (leaf-first, the leaf is the focus target) on every focus change; the
      // backend consumes that chain directly — there is no separate `fq` to
      // supply. The recorded chain drives command gating's scope fallback and
      // the `scope_chain` UI-state echo the frontend listens for.
      {
        id: "ui.setFocus",
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

      // ─── window.new ─────────────────────────────────────────────────────
      // ui.yaml: keys cua/vim/emacs all Mod+Shift+N, menu {path:[Window],
      // group:0, order:0}. Routes to window `new window`.
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

      // ─── UI-surface commands (Cards D + E) ──────────────────────────────
      // field.edit / field.editEnter / pressable.activate /
      // pressable.activateSpace / filter_editor.drillIn /
      // ui.ai-panel.composer.drillIn / ui.ai-panel.elicitation.field.drillIn
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
        // lifted into the global key table.
        scope: [spec.scope],
        keys: spec.keys,
        execute: async () => {
          return { ok: true };
        },
      })),
    ]);

    this.log.info(
      "ui-commands: registered 18 commands (ui.inspect / entity.inspect / ui.inspector.* / app.palette.open / ui.palette.close / ui.entity.startRename / ui.mode.set / ui.setFocus / window.new across ui_state / focus / window; field.edit / field.editEnter / pressable.activate / pressable.activateSpace / filter_editor.drillIn / ui.ai-panel.composer.drillIn / ui.ai-panel.elicitation.field.drillIn → webview bus)",
    );
  }
}
