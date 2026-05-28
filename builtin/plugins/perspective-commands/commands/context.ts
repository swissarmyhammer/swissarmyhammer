// Shared command-context types + scope helpers for the perspective-commands
// plugin's sub-domain modules. Mirrors the helpers in the task-commands /
// kanban-misc-commands templates 1:1 so every sub-file resolves params the
// same way.

/** The plugin `this` proxy exposes `this.views.views.<noun>.<verb>(...)`. */
export interface ViewsDispatch {
  views: {
    views: {
      perspective: {
        load: (args: Record<string, unknown>) => Promise<unknown>;
        save: (args: Record<string, unknown>) => Promise<unknown>;
        delete: (args: Record<string, unknown>) => Promise<unknown>;
        rename: (args: Record<string, unknown>) => Promise<unknown>;
        list: (args: Record<string, unknown>) => Promise<unknown>;
        next: (args: Record<string, unknown>) => Promise<unknown>;
        prev: (args: Record<string, unknown>) => Promise<unknown>;
        goto: (args: Record<string, unknown>) => Promise<unknown>;
        switch: (args: Record<string, unknown>) => Promise<unknown>;
      };
      filter: {
        set: (args: Record<string, unknown>) => Promise<unknown>;
        clear: (args: Record<string, unknown>) => Promise<unknown>;
        focus: (args: Record<string, unknown>) => Promise<unknown>;
      };
      group: {
        set: (args: Record<string, unknown>) => Promise<unknown>;
        clear: (args: Record<string, unknown>) => Promise<unknown>;
      };
      sort: {
        set: (args: Record<string, unknown>) => Promise<unknown>;
        clear: (args: Record<string, unknown>) => Promise<unknown>;
        toggle: (args: Record<string, unknown>) => Promise<unknown>;
      };
    };
  };
}

/**
 * The dispatch context the command service passes a command callback.
 *
 * Mirrors `swissarmyhammer_command_service::CommandContext`: the active scope
 * monikers, the optional context-menu target moniker, and a free-form args
 * bag the dispatching surface populates. A moniker is an `"<entity_type>:<id>"`
 * pair (e.g. `"perspective:01ABC"`), which is what a YAML `from: scope_chain`
 * param resolves against.
 */
export interface CommandContext {
  /** Active scope monikers, leaf-last (e.g. `["board:01A", "perspective:42"]`). */
  scope_chain?: string[];
  /** Context-menu target moniker (the entity the menu fired over). */
  target?: string;
  /** Free-form args bag populated by the dispatching surface. */
  args?: Record<string, unknown>;
}

/** An `available` callback result: ok, or not-ok with a user-facing reason. */
export type Availability = { ok: true } | { ok: false; reason: string };

/** A registration row, as `registerCommands` accepts. */
export type CommandSpec = Record<string, unknown>;

/**
 * Resolve the id of the first scope-chain moniker of `entityType`.
 *
 * A `from: scope_chain` param with `entity_type: <t>` resolves to the id half
 * of the nearest `"<t>:<id>"` moniker in the chain. Returns `undefined` when no
 * such moniker is in scope — the signal an `available` precondition is unmet.
 */
export function scopeId(
  ctx: CommandContext,
  entityType: string,
): string | undefined {
  const prefix = `${entityType}:`;
  const chain = ctx.scope_chain ?? [];
  // Scope chains are leaf-last; scan from the leaf so the nearest entity wins.
  for (let i = chain.length - 1; i >= 0; i -= 1) {
    const moniker = chain[i];
    if (moniker.startsWith(prefix)) {
      return moniker.slice(prefix.length);
    }
  }
  return undefined;
}

/**
 * Resolve a `perspective_id` param: the YAML uses two sources for it —
 * `from: scope_chain` (filter.focus / group / sort.set) or `from: args`
 * (filter / clearFilter / clearGroup / sort.clear / sort.toggle / switch).
 *
 * This collapses both into the actual id the backend wants: prefer the
 * explicit args value, then fall back to the nearest perspective scope
 * moniker — matching the dispatcher's `resolve_perspective_id` cascade.
 */
export function perspectiveId(ctx: CommandContext): string | undefined {
  const fromArgs = ctx.args?.perspective_id;
  if (typeof fromArgs === "string" && fromArgs.length > 0) return fromArgs;
  return scopeId(ctx, "perspective");
}
