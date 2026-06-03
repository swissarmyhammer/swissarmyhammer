// ui-commands — builtin plugin porting `ui.yaml` (10 commands) to the
// TypeScript plugin SDK. This is the last builtin-commands port.
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
//   ui.palette.open       → ui_state `open palette`        (...palette.open)
//   ui.palette.close      → ui_state `close palette`       (...palette.close)
//   ui.entity.startRename → ui_state `start rename`        (...rename.start)
//   ui.mode.set           → ui_state `set keymap`          (...keymap.set)
//   ui.setFocus           → focus    `set focus`           (...focus.set)
//   window.new            → window   `new window`          (...window.new)
//
// Memory `no-client-side-inspect`: `ui.inspect` dispatches through the backend
// (`ui_state`) like any other command — there is NO React-side shortcut. The
// plugin merely routes `ui.inspect` → ui_state `inspect inspector` on the
// context-menu target moniker. The regression e2e asserts this routes via the
// Command service.
//
// Spatial focus is owned by the SEPARATE `focus` MCP server (`SpatialRegistry`
// / `SpatialState`), NOT by `ui_state`. `ui.setFocus` routes to focus
// `set focus` — the `ui_state` server deliberately exposes no `set_focus` op.

import {
  Plugin,
  ensureServices,
  registerCommands,
} from "@swissarmyhammer/plugin";

// ───────────────────────────────────────────────────────────────────────────
// The command context the host hands every `execute` callback.
// ───────────────────────────────────────────────────────────────────────────

/**
 * The dispatch context the command service passes a command callback.
 *
 * Mirrors `swissarmyhammer_command_service::CommandContext`: the active scope
 * monikers, the optional context-menu target moniker, and a free-form args
 * bag the dispatching surface populates (the ui commands read the window the
 * action fired in out of `args.window_label`, plus per-command params).
 */
interface CommandContext {
  /** Active scope monikers, leaf-last (e.g. `["board:01A", "task:42"]`). */
  scope_chain?: string[];
  /** Context-menu target moniker (the entity the menu fired over). */
  target?: string;
  /** Free-form args bag populated by the dispatching surface. */
  args?: Record<string, unknown>;
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
    };
  };
}

/**
 * The dispatch surface for the `focus` operation tool — the spatial-nav focus
 * kernel `ui.setFocus` routes to.
 *
 * Verb/noun pair from `crates/swissarmyhammer-focus/src/operations.rs`:
 *   `set focus` → this.focus.focus.focus.set
 */
interface FocusDispatch {
  focus: {
    focus: {
      focus: {
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
        new(args: Record<string, unknown>): Promise<unknown>;
      };
    };
  };
}

/**
 * Resolve the `window_label` the ui_state / focus / window ops carry.
 *
 * The dispatching surface plants the active window's label in `args`; the
 * `ui_state` ops default a missing label to the empty string server-side, so
 * a `""` fallback is the faithful no-op-when-absent behavior.
 */
function windowLabel(ctx: CommandContext): string {
  const fromArgs = ctx.args?.window_label;
  return typeof fromArgs === "string" ? fromArgs : "";
}

/**
 * The ui-commands builtin plugin.
 *
 * Registers the ten UI commands ported from `ui.yaml`, routed across the
 * `ui_state`, `focus`, and `window` MCP servers. Identity is the bundle
 * directory name (`ui-commands`); `name` / `description` are descriptive
 * metadata only.
 */
export default class UiCommandsPlugin extends Plugin {
  /** Human-readable name — descriptive metadata only, not plugin identity. */
  readonly name = "UI Commands";

  /** One-line description — descriptive metadata only. */
  readonly description =
    "Builtin UI commands (inspector open/close, command palette open/close, perspective rename, keymap mode, spatial focus, and new window) routed to the ui_state, focus, and window servers.";

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
    await ensureServices(this, ["commands", "ui_state", "window", "focus"]);

    const uiState = this as unknown as UiStateDispatch;
    const focus = this as unknown as FocusDispatch;
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
            window_label: windowLabel(ctx),
            moniker: ctx.target ?? "",
          });
        },
      },

      // ─── ui.inspector.close ─────────────────────────────────────────────
      // ui.yaml: keys cua:Escape / vim:q. Routes to ui_state `close inspector`.
      {
        id: "ui.inspector.close",
        name: "Close Inspector",
        keys: { cua: "Escape", vim: "q" },
        execute: async (rawCtx: unknown) => {
          const ctx = (rawCtx ?? {}) as CommandContext;
          return await uiState.ui_state.ui_state.inspector.close({
            window_label: windowLabel(ctx),
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
            window_label: windowLabel(ctx),
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
            window_label: windowLabel(ctx),
            width: ctx.args?.width,
          });
        },
      },

      // ─── ui.palette.open ────────────────────────────────────────────────
      // ui.yaml: keys cua:Mod+K / vim:":". Routes to ui_state `open palette`.
      {
        id: "ui.palette.open",
        name: "Command Palette",
        keys: { cua: "Mod+K", vim: ":" },
        execute: async (rawCtx: unknown) => {
          const ctx = (rawCtx ?? {}) as CommandContext;
          return await uiState.ui_state.ui_state.palette.open({
            window_label: windowLabel(ctx),
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
            window_label: windowLabel(ctx),
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
            window_label: windowLabel(ctx),
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
      // ui.yaml: visible:false, undoable:false. Routes to the SEPARATE `focus`
      // server's `set focus` — spatial focus is owned by the focus kernel, NOT
      // ui_state. The op takes the target FQM (`fq`) plus the per-decision
      // `snapshot`; the dispatching surface plants both in args.
      {
        id: "ui.setFocus",
        name: "Set Focus",
        visible: false,
        undoable: false,
        execute: async (rawCtx: unknown) => {
          const ctx = (rawCtx ?? {}) as CommandContext;
          const args = ctx.args ?? {};
          const focusArgs: Record<string, unknown> = { fq: args.fq };
          if (args.snapshot !== undefined) focusArgs.snapshot = args.snapshot;
          return await focus.focus.focus.focus.set(focusArgs);
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
    ]);

    this.log.info(
      "ui-commands: registered 10 commands (ui.inspect / ui.inspector.* / ui.palette.* / ui.entity.startRename / ui.mode.set / ui.setFocus / window.new) across ui_state / focus / window",
    );
  }
}
