// Shared command-context types + dispatch surfaces for the
// app-shell-commands plugin's sub-domain modules (`app.ts`, `settings.ts`,
// `drag.ts`, `ui.ts`). Mirrors the `perspective-commands` template's
// `context.ts` 1:1 so every sub-file resolves params and dispatches the same
// way.

/**
 * The dispatch context the command service passes a command callback.
 *
 * Mirrors `swissarmyhammer_command_service::CommandContext`: the active scope
 * monikers, the optional context-menu target moniker, and a free-form args
 * bag the dispatching surface populates. The app-shell UI toggles resolve the
 * window the action fired in from the `window:` moniker in `scope_chain` — the
 * single structured parameter — not a denormalized `window_label`.
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
 * keymap mode, drag state machine, inspector / palette stacks, perspective
 * rename, and focus scope-chain record the app-shell commands route to.
 *
 * Verb/noun pairs from `crates/swissarmyhammer-ui-state/src/operations.rs`:
 *   `show command`    → this.ui_state.ui_state.command.show    (app.command)
 *   `show palette`    → this.ui_state.ui_state.palette.show    (app.palette)
 *   `show search`     → this.ui_state.ui_state.search.show     (app.search)
 *   `dismiss ui`      → this.ui_state.ui_state.ui.dismiss       (app.dismiss)
 *   `set keymap`      → this.ui_state.ui_state.keymap.set       (settings.keymap.*, app.mode.set)
 *   `start drag`      → this.ui_state.ui_state.drag.start       (drag.start)
 *   `cancel drag`     → this.ui_state.ui_state.drag.cancel      (drag.cancel)
 *   `complete drag`   → this.ui_state.ui_state.drag.complete    (drag.complete)
 *   `inspect inspector`   → this.ui_state.ui_state.inspector.inspect   (app.inspect, entity.inspect)
 *   `close inspector`     → this.ui_state.ui_state.inspector.close     (app.inspector.close)
 *   `close_all inspector` → this.ui_state.ui_state.inspector.close_all (app.inspector.close_all)
 *   `set_width inspector` → this.ui_state.ui_state.inspector.set_width (app.inspector.set_width)
 *   `open palette`    → this.ui_state.ui_state.palette.open     (app.palette.open)
 *   `close palette`   → this.ui_state.ui_state.palette.close    (app.palette.close)
 *   `start rename`    → this.ui_state.ui_state.rename.start     (app.entity.startRename)
 *   `set scope_chain` → this.ui_state.ui_state.scope_chain.set  (app.setFocus)
 */
export interface UiStateDispatch {
  ui_state: {
    ui_state: {
      command: {
        show(args: Record<string, unknown>): Promise<unknown>;
      };
      palette: {
        show(args: Record<string, unknown>): Promise<unknown>;
        open(args: Record<string, unknown>): Promise<unknown>;
        close(args: Record<string, unknown>): Promise<unknown>;
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
      inspector: {
        inspect(args: Record<string, unknown>): Promise<unknown>;
        close(args: Record<string, unknown>): Promise<unknown>;
        close_all(args: Record<string, unknown>): Promise<unknown>;
        set_width(args: Record<string, unknown>): Promise<unknown>;
      };
      rename: {
        start(args: Record<string, unknown>): Promise<unknown>;
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
export interface WindowDispatch {
  window: {
    window: {
      window: {
        new (args: Record<string, unknown>): Promise<unknown>;
      };
    };
  };
}
