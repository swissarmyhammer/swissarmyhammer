// ui-commands — builtin plugin porting `ui.yaml` (10 commands) to the
// TypeScript plugin SDK (the last builtin-commands port), plus the four
// UI-surface commands Card D moved out of React (`UI_SURFACE_COMMANDS`
// below): `field.edit` / `field.editEnter` / `pressable.activate` /
// `pressable.activateSpace` — 14 commands total.
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
// Backend routing — 10 commands across 3 backends:
//   ui.inspect            → ui_state `inspect inspector`   (...inspector.inspect)
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
 * The four UI-surface commands, as a data table (Card D of the
 * ui-command-cleanup project — mirrors the `grid-commands` bundle's
 * `GRID_COMMANDS` pattern).
 *
 * `id` / `name` / `keys` are copied 1:1 from the retired client-side
 * `CommandDef`s: `field.edit` / `field.editEnter` from
 * `apps/kanban-app/ui/src/components/fields/field.tsx` and
 * `pressable.activate` / `pressable.activateSpace` from
 * `apps/kanban-app/ui/src/components/pressable.tsx` (`usePressCommands`).
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
];

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
 * Registers the ten UI commands ported from `ui.yaml`, routed across the
 * `ui_state`, `focus`, and `window` MCP servers, plus the four webview-bus
 * handled UI-surface commands (`UI_SURFACE_COMMANDS`). Identity is the
 * bundle directory name (`ui-commands`); `name` / `description` are
 * descriptive metadata only.
 */
export default class UiCommandsPlugin extends Plugin {
  /** Human-readable name — descriptive metadata only, not plugin identity. */
  readonly name = "UI Commands";

  /** One-line description — descriptive metadata only. */
  readonly description =
    "Builtin UI commands (inspector open/close, command palette open/close, perspective rename, keymap mode, spatial focus, and new window) routed to the ui_state, focus, and window servers, plus the webview-bus handled field-edit and pressable-activation commands.";

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

      // ─── UI-surface commands (Card D) ───────────────────────────────────
      // field.edit / field.editEnter / pressable.activate /
      // pressable.activateSpace from the data table above. Presentation-only:
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
      "ui-commands: registered 14 commands (ui.inspect / ui.inspector.* / app.palette.open / ui.palette.close / ui.entity.startRename / ui.mode.set / ui.setFocus / window.new across ui_state / focus / window; field.edit / field.editEnter / pressable.activate / pressable.activateSpace → webview bus)",
    );
  }
}
