/**
 * `<CommandButton>` — generic tab-button affordance for any YAML CommandDef
 * tagged with `tab_button`.
 *
 * One component handles every shape of command:
 *
 *   - **No-arg / scope-only dispatch.** A command whose every param has
 *     `from: scope_chain | target | args | default` (i.e. no `shape`) needs
 *     no UI to collect values — the click immediately dispatches with
 *     scope-resolved args.
 *
 *   - **Single-param dropdown / multi-param form / expression editor.** Any
 *     param with a `shape` opens a {@link CommandPopover} so the user can
 *     pick values before dispatch.
 *
 * The component is the React-side mirror of the per-surface migration: the
 * tab bar (and future surfaces) iterates over `commands_for_scope` filtered
 * for `tab_button`-tagged commands and renders one `<CommandButton>` each.
 *
 * # Spatial-nav moniker
 *
 * The leaf moniker is built deterministically from `surface`, `command.id`,
 * and `surfaceId` so two `<CommandButton>`s for two different commands on
 * the same surface register as distinct leaves (e.g.
 * `perspective_tab.perspective.focusFilter:p1` and
 * `perspective_tab.perspective.setGroup:p1`).
 *
 * # Style
 *
 * Visually matches the existing `<FilterFocusButton>` / `<GroupPopoverButton>`
 * tab-button affordances so a mixed render (legacy + generic) looks uniform.
 */

import { useCallback, useState } from "react";
import {
  Popover,
  PopoverTrigger,
  PopoverContent,
} from "@/components/ui/popover";
import { Pressable } from "@/components/pressable";
import { useDispatchCommand } from "@/lib/command-scope";
import { asSegment } from "@/types/spatial";
import { cn } from "@/lib/utils";
import { commandIconFor } from "./command-icon-registry";
import { CommandPopover } from "./command-popover";
import type { CommandDef, ParamDef } from "@/types/kanban";

/** Props for `<CommandButton>`. */
export interface CommandButtonProps {
  /** The YAML-shape command definition to render. */
  command: CommandDef;
  /**
   * Surface identifier — namespaces the spatial-nav moniker so two surfaces
   * (e.g. the perspective tab bar vs. a future toolbar) hosting the same
   * command id register as distinct leaves. Conventionally a short kebab-case
   * string like `"perspective_tab"`.
   */
  surface: string;
  /**
   * Per-surface instance id (e.g. perspective id when rendered on the tab
   * for that perspective). Concatenated into the moniker as a suffix so each
   * tab's buttons live at distinct spatial-nav coordinates.
   */
  surfaceId: string;
  /**
   * Visual "active" indicator — when true, the icon renders in the primary
   * accent color, matching the existing `<FilterFocusButton>` /
   * `<GroupPopoverButton>` highlight pattern for an active filter or group.
   *
   * Orthogonal to picker logic — a command can be active (highlighted) and
   * still need a popover to pick new values.
   */
  isActive?: boolean;
}

/**
 * True when at least one of the command's params has a `shape` set —
 * i.e. the runtime must collect a value from the user before dispatch.
 *
 * A param without a `shape` is fully resolved by its `from` source
 * (scope_chain, target, args, default) and contributes nothing to the UI.
 */
function hasPickableParam(params: readonly ParamDef[] | undefined): boolean {
  if (!params) return false;
  return params.some((p) => p.shape !== undefined);
}

/**
 * Render a tab-button affordance for the given command.
 *
 * On click the component decides whether to dispatch immediately (no
 * pickable params) or open a popover (one or more pickable params). The
 * popover's `onCommit` collects user-picked values and the subsequent
 * dispatch carries them as `args`.
 */
export function CommandButton({
  command,
  surface,
  surfaceId,
  isActive = false,
}: CommandButtonProps) {
  // Defensive: an empty `surfaceId` is type-legal but would produce the
  // moniker `${surface}.${command.id}:`, which collides with every other
  // empty-surfaceId button on the same surface. Convert that silent
  // collision into an immediate crash so the offending caller is obvious.
  if (!surfaceId) {
    throw new Error(
      `CommandButton: surfaceId must be a non-empty string (command=${command.id}, surface=${surface})`,
    );
  }
  if (!surface) {
    throw new Error(
      `CommandButton: surface must be a non-empty string (command=${command.id})`,
    );
  }
  const Icon = commandIconFor(command.tab_button?.icon ?? "");
  const dispatch = useDispatchCommand();
  const needsPicker = hasPickableParam(command.params);
  const [open, setOpen] = useState(false);

  const dispatchWith = useCallback(
    (commandId: string, args: Record<string, unknown>) => {
      // Errors surface to console — there is no user-visible toast for
      // command failures yet (the wider error-surface story is a separate
      // concern), so swallow rejection here to match the rest of the
      // codebase's dispatch sites (see e.g. AddPerspectiveButton).
      dispatch(commandId, { args }).catch(console.error);
    },
    [dispatch],
  );

  const handlePress = useCallback(() => {
    if (needsPicker) {
      setOpen(true);
      return;
    }
    dispatchWith(command.id, {});
  }, [needsPicker, dispatchWith, command.id]);

  // On commit, look for a `clear_command` redirection: when any param
  // declares `clear_command` AND the user picked the empty-string
  // sentinel for that param, dispatch the redirection target instead
  // of the parent command. The redirected dispatch carries the args
  // bag with the empty-string entry stripped — the clear command's
  // contract is that it takes no value (just scope-resolved
  // `perspective_id` etc.). This restores the legacy single-popover
  // "None to clear" affordance for the Group migration without
  // pushing the redirection into the YAML.
  //
  // Only the FIRST param with a matching clear submission redirects.
  // Multi-param commit-and-clear is not a shape we expect to see (the
  // YAML pattern is one enum param per popover), so we keep the
  // resolution rule simple and deterministic.
  const handleCommit = useCallback(
    (args: Record<string, unknown>) => {
      setOpen(false);
      const params = command.params ?? [];
      for (const p of params) {
        if (p.clear_command !== undefined && args[p.name] === "") {
          // Strip the sentinel from the args so the clear command sees
          // an empty bag for the redirected param.
          const { [p.name]: _omitted, ...rest } = args;
          dispatchWith(p.clear_command, rest);
          return;
        }
      }
      dispatchWith(command.id, args);
    },
    [dispatchWith, command.id, command.params],
  );

  const moniker = asSegment(`${surface}.${command.id}:${surfaceId}`);
  const pressable = (
    <Pressable
      asChild
      moniker={moniker}
      ariaLabel={command.name}
      onPress={handlePress}
    >
      <button
        type="button"
        className={cn(
          "inline-flex items-center justify-center h-5 w-5 rounded transition-colors -ml-1",
          isActive
            ? "text-primary"
            : "text-muted-foreground/50 hover:text-muted-foreground",
        )}
        onClick={(e) => e.stopPropagation()}
      >
        <Icon className="h-3 w-3" fill={isActive ? "currentColor" : "none"} />
      </button>
    </Pressable>
  );

  // No popover branch — render the bare Pressable.
  if (!needsPicker) return pressable;

  // Popover branch — the Pressable acts as the trigger and the popover
  // hosts the CommandPopover form.
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>{pressable}</PopoverTrigger>
      <PopoverContent
        align="start"
        sideOffset={4}
        className="p-3 w-auto"
        onOpenAutoFocus={(e) => e.preventDefault()}
      >
        <CommandPopover
          command={command}
          onCommit={handleCommit}
          onCancel={() => setOpen(false)}
        />
      </PopoverContent>
    </Popover>
  );
}
