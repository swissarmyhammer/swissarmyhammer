/**
 * `<TabIconButton>` — the shared presentational shell for tab-bar icon
 * buttons.
 *
 * One `Pressable > button > Icon` shell, consumed by:
 *
 *   - `<CommandButton>` (`command-button.tsx`) — the generic registry-
 *     rendered tab-button affordance (dispatches its command, or opens a
 *     `<CommandPopover>` as the popover trigger).
 *   - `<FilterFocusCommandButton>` (`perspective-tab-bar.tsx`) — dispatches
 *     `nav.focus` against the formula bar's spatial-nav scope.
 *   - `<AddPerspectiveCommandButton>` (`perspective-tab-bar.tsx`) — creates
 *     a perspective immediately and arms inline rename.
 *
 * The adapters keep their distinct press semantics; the visual /
 * spatial-nav contract lives here ONCE. Extracted by the rule-of-three
 * (card 01KTYN8GB25ZFKSXWA0QA283PG review blocker B2): the shell was
 * hand-copied three times and the "matches `<CommandButton>` exactly"
 * invariant was enforced only by manual sync.
 *
 * # Contract
 *
 *   - Icon resolution from the YAML `tab_button.icon` name via
 *     `commandIconFor` (unknown names render the registry's fallback).
 *   - `isActive` paints the `text-primary` highlight and a filled icon;
 *     inactive renders muted with a hover affordance.
 *   - The inner `<button>` stops click propagation so the enclosing tab /
 *     bar click handlers never misfire alongside the press.
 *
 * # Slot composition
 *
 * Props and ref forward through to `<Pressable>` so the component composes
 * under Radix slots — `<CommandButton>`'s popover branch wraps it in
 * `<PopoverTrigger asChild>`, whose injected trigger props (onClick,
 * aria-haspopup, data-state, ref) must reach the underlying button.
 */

import { forwardRef, type ButtonHTMLAttributes } from "react";
import { Pressable } from "@/components/pressable";
import { cn } from "@/lib/utils";
import { commandIconFor } from "@/components/command-icon-registry";
import type { SegmentMoniker } from "@/types/spatial";

/** Props for `<TabIconButton>`. */
export interface TabIconButtonProps extends Omit<
  ButtonHTMLAttributes<HTMLButtonElement>,
  "onClick"
> {
  /** Spatial-nav leaf moniker, already composed by the adapter. */
  moniker: SegmentMoniker;
  /** Accessible label (the command's display name). */
  ariaLabel: string;
  /** YAML `tab_button.icon` name, resolved via `commandIconFor`. */
  icon: string;
  /**
   * Visual "active" indicator — when true, the icon renders in the primary
   * accent color with a filled glyph (e.g. an active filter or group).
   */
  isActive?: boolean;
  /** Activation callback — mouse click and keyboard Enter/Space alike. */
  onPress: () => void;
}

/**
 * Render the canonical tab icon-button shell. See the file-level docstring
 * for the contract and the consuming adapters.
 */
export const TabIconButton = forwardRef<HTMLButtonElement, TabIconButtonProps>(
  function TabIconButton(
    { moniker, ariaLabel, icon, isActive = false, onPress, ...rest },
    ref,
  ) {
    const Icon = commandIconFor(icon);
    return (
      <Pressable
        asChild
        moniker={moniker}
        ariaLabel={ariaLabel}
        onPress={onPress}
        ref={ref}
        {...rest}
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
  },
);
