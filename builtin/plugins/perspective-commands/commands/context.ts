// Perspective-specific command-context surfaces for the perspective-commands
// plugin's sub-domain modules. The shared command-context type
// (`CommandContext`), availability shape (`Availability`), and scope-moniker
// resolver (`scopeId`) now live in `@swissarmyhammer/plugin`; this module hosts
// only what is unique to perspective-commands — the `views` dispatch surface,
// the registration-row alias, and the `perspective_id` cascade resolver.

import { type CommandContext, scopeId } from "@swissarmyhammer/plugin";

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

/** A registration row, as `registerCommands` accepts. */
export type CommandSpec = Record<string, unknown>;

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
