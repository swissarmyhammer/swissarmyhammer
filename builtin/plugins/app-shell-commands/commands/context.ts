// Shared command-context types + dispatch surfaces for the
// app-shell-commands plugin's sub-domain modules (`app.ts`, `settings.ts`,
// `drag.ts`). Mirrors the `perspective-commands` template's `context.ts` 1:1
// so every sub-file resolves params and dispatches the same way.

/**
 * The dispatch context the command service passes a command callback.
 *
 * Mirrors `swissarmyhammer_command_service::CommandContext`: the active scope
 * monikers, the optional context-menu target moniker, and a free-form args
 * bag the dispatching surface populates (the app-shell commands read the
 * window the action fired in out of `args.window_label`).
 */
export interface CommandContext {
  /** Active scope monikers, leaf-last (e.g. `["board:01A", "task:42"]`). */
  scope_chain?: string[];
  /** Context-menu target moniker (the entity the menu fired over). */
  target?: string;
  /** Free-form args bag populated by the dispatching surface. */
  args?: Record<string, unknown>;
}

/** A registration row, as `registerCommands` accepts. */
export type CommandSpec = Record<string, unknown>;

/**
 * The dispatch surface for the `app` operation tool — the genuine
 * window-manager / OS-chrome actions.
 *
 * The dispatch Proxy turns a property path into an MCP `tools/call`:
 * `server.tool.noun.verb`. For the `app` server the server name and the single
 * tool name are both `"app"`, and the noun/verb pairs come straight from
 * `crates/swissarmyhammer-app-service/src/operations.rs`:
 *   `quit app`   → this.app.app.app.quit
 *   `show about` → this.app.app.about.show
 *   `show help`  → this.app.app.help.show
 */
export interface AppDispatch {
  app: {
    app: {
      app: {
        quit(args: Record<string, unknown>): Promise<unknown>;
      };
      about: {
        show(args: Record<string, unknown>): Promise<unknown>;
      };
      help: {
        show(args: Record<string, unknown>): Promise<unknown>;
      };
    };
  };
}

/**
 * The dispatch surface for the `store` operation tool — stack-wide undo/redo
 * over the one unified `StoreContext`.
 *
 * Verb/noun pairs from `crates/swissarmyhammer-store/src/operations.rs`:
 *   `undo stack` → this.store.store.stack.undo
 *   `redo stack` → this.store.store.stack.redo
 */
export interface StoreDispatch {
  store: {
    store: {
      stack: {
        undo(args: Record<string, unknown>): Promise<unknown>;
        redo(args: Record<string, unknown>): Promise<unknown>;
      };
    };
  };
}

/**
 * The dispatch surface for the `ui_state` operation tool — the UI toggles,
 * keymap mode, and drag state machine the app-shell commands route to.
 *
 * Verb/noun pairs from `crates/swissarmyhammer-ui-state/src/operations.rs`:
 *   `show command`  → this.ui_state.ui_state.command.show   (app.command)
 *   `show palette`  → this.ui_state.ui_state.palette.show   (app.palette)
 *   `show search`   → this.ui_state.ui_state.search.show    (app.search)
 *   `dismiss ui`    → this.ui_state.ui_state.ui.dismiss      (app.dismiss)
 *   `set keymap`    → this.ui_state.ui_state.keymap.set      (settings.keymap.*)
 *   `start drag`    → this.ui_state.ui_state.drag.start      (drag.start)
 *   `cancel drag`   → this.ui_state.ui_state.drag.cancel     (drag.cancel)
 *   `complete drag` → this.ui_state.ui_state.drag.complete   (drag.complete)
 */
export interface UiStateDispatch {
  ui_state: {
    ui_state: {
      command: {
        show(args: Record<string, unknown>): Promise<unknown>;
      };
      palette: {
        show(args: Record<string, unknown>): Promise<unknown>;
      };
      search: {
        show(args: Record<string, unknown>): Promise<unknown>;
      };
      ui: {
        dismiss(args: Record<string, unknown>): Promise<unknown>;
      };
      keymap: {
        set(args: Record<string, unknown>): Promise<unknown>;
      };
      drag: {
        start(args: Record<string, unknown>): Promise<unknown>;
        cancel(args: Record<string, unknown>): Promise<unknown>;
        complete(args: Record<string, unknown>): Promise<unknown>;
      };
    };
  };
}

/**
 * Resolve the `window_label` the UI toggles / drag ops need.
 *
 * The dispatching surface plants the active window's label in `args`; the
 * `ui_state` ops default a missing label to the empty string server-side, so
 * a `""` fallback is the faithful no-op-when-absent behavior.
 */
export function windowLabel(ctx: CommandContext): string {
  const fromArgs = ctx.args?.window_label;
  return typeof fromArgs === "string" ? fromArgs : "";
}
