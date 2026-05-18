/**
 * Spatial-nav focus wiring for the AI panel.
 *
 * The AI panel must participate in the app's focus / spatial-navigation /
 * jump-to systems exactly like the board, the nav-bar, and the inspector —
 * the user can "jump" into the panel and arrow-navigate between its
 * controls. This module holds the two thin wrappers that wire that up
 * without forcing every `AiPanel` unit test to stand up the full
 * spatial-nav provider stack.
 *
 * # Path-monikers: the panel is a ZONE, a child of the window layer
 *
 * The panel body registers as a `<FocusScope moniker="ui:ai-panel">` zone.
 * Because the panel is mounted inside `App.tsx`'s window-root
 * `<FocusLayer name="window">` (a sibling of `ViewsContainer`), the zone's
 * fully-qualified moniker is the PATH `/window/ui:ai-panel` — composed by
 * `<FocusScope>` from the ancestor `FullyQualifiedMonikerContext`. It is
 * **not** a flat leaf string: a flat moniker causes duplicate-registration
 * ambiguity in the kernel that surfaces as "nav crosses layers". Each
 * interactive control inside the panel is a `<FocusScope>` leaf whose FQM
 * is composed one level deeper — `/window/ui:ai-panel/ui:ai-panel.composer`
 * and so on.
 *
 * # Zone, not a separate layer
 *
 * The panel is a *zone* in the window layer, NOT its own `<FocusLayer>`.
 * A separate layer would make the kernel's layer-boundary guard refuse
 * cardinal navigation between the view area and the panel. The task
 * requires focus to cross the view-area ↔ panel boundary cleanly, with no
 * cross-layer jump — so the panel shares the `/window` layer with the
 * board, exactly like `ui:perspective`, `ui:navbar.*`, and `ui:left-nav`.
 *
 * # Conditional on the spatial-nav stack
 *
 * `<FocusScope>` and `<Pressable>` throw when mounted outside a
 * `<FocusLayer>`. Production (`App.tsx`) always mounts the window layer,
 * but the `AiPanel` View is also unit-tested standalone without the
 * spatial stack. Mirroring `perspective-container.tsx`'s
 * `PerspectiveSpatialZone`, these wrappers render the spatial primitive
 * only when an enclosing `<FocusLayer>` is present; otherwise they render
 * their children directly so the narrow unit-test provider tree keeps
 * working.
 */

import type { ButtonHTMLAttributes, MouseEvent, ReactNode } from "react";
import { Slot } from "radix-ui";
import { FocusScope } from "@/components/focus-scope";
import { Pressable, type PressableProps } from "@/components/pressable";
import { useOptionalEnclosingLayerFq } from "@/components/layer-fq-context";
import type { SegmentMoniker } from "@/types/spatial";

/** Props for {@link AiPanelFocusScope}. */
export interface AiPanelFocusScopeProps {
  /**
   * Relative `SegmentMoniker` for this scope — composed under the parent
   * FQM by the enclosing primitive. The panel zone passes `"ui:ai-panel"`;
   * inner controls pass `"ui:ai-panel.composer"` etc.
   */
  moniker: SegmentMoniker;
  /**
   * When false, suppresses the visible `<FocusIndicator>` and the
   * follow-the-focus `scrollIntoView`. Container zones (the panel body,
   * the scrollback region) pass `false`; bounded leaf controls leave it
   * `true` so they advertise focus the way a card or a field row does.
   */
  showFocus?: boolean;
  /** Extra classes merged onto the scope wrapper `<div>`. */
  className?: string;
  children: ReactNode;
}

/**
 * Wrap `children` in a `<FocusScope>` when the spatial-nav stack is
 * present, otherwise render them directly.
 *
 * Used for the panel zone, the conversation scrollback region, and the
 * composer — every AI-panel surface that is a container or a non-button
 * focus target. Actionable icon buttons use {@link AiPanelPressable}
 * instead so they also get keyboard activation.
 */
export function AiPanelFocusScope({
  moniker,
  showFocus = true,
  className,
  children,
}: AiPanelFocusScopeProps): ReactNode {
  const layerFq = useOptionalEnclosingLayerFq();
  if (!layerFq) {
    // No spatial-nav stack (standalone unit test) — render children with
    // no scope wrapper. `className` is dropped: the only non-test caller
    // passes layout classes the production tree applies via the scope's
    // wrapping `<div>`, and a stray wrapper here would change the DOM the
    // unit tests assert against.
    return <>{children}</>;
  }
  return (
    <FocusScope moniker={moniker} showFocus={showFocus} className={className}>
      {children}
    </FocusScope>
  );
}

/** Props for {@link AiPanelPressable} — identical to {@link PressableProps}. */
export type AiPanelPressableProps = PressableProps;

/**
 * Render a `<Pressable>` when the spatial-nav stack is present, otherwise
 * render the bare button host.
 *
 * `<Pressable>` registers a `<FocusScope>` leaf plus the Enter / Space
 * keyboard-activation CommandDefs — but it throws outside a
 * `<FocusLayer>`. In a standalone `AiPanel` unit test there is no layer,
 * so this falls back to the inner host element (typically a `<button>`)
 * with the `onClick`, `aria-label`, and `disabled` props applied, keeping
 * mouse activation and accessibility intact without the spatial leaf.
 */
export function AiPanelPressable({
  moniker,
  onPress,
  ariaLabel,
  asChild,
  disabled,
  children,
  ...rest
}: AiPanelPressableProps): ReactNode {
  const layerFq = useOptionalEnclosingLayerFq();
  if (!layerFq) {
    // No spatial-nav stack — render the actionable host directly, with no
    // spatial leaf. In `asChild` mode the caller already supplies the host
    // element; pass it through `<Slot.Root>` so any props an outer slot
    // (e.g. `DropdownMenuTrigger asChild`) injected via `cloneElement` are
    // merged onto that host — exactly what the real `<Pressable asChild>`
    // does. A bare fragment would swallow those props and leave the
    // trigger inert. Otherwise emit a plain `<button type="button">` so the
    // control stays clickable and labelled in the narrow unit-test tree.
    //
    // An outer slot may inject its own `onClick` via `...rest`; compose it
    // with `onPress` so the parent handler (e.g. the dropdown's open) still
    // fires alongside the activation — mirroring `<Pressable>`'s own
    // outer-handler-first ordering.
    const { onClick: outerOnClick, ...restWithoutClick } =
      rest as ButtonHTMLAttributes<HTMLButtonElement>;
    const handleClick = (e: MouseEvent<HTMLButtonElement>) => {
      outerOnClick?.(e);
      if (e.defaultPrevented) return;
      if (!disabled) onPress();
    };
    const Host = asChild ? Slot.Root : "button";
    const hostProps = asChild ? {} : { type: "button" as const };
    return (
      <Host
        aria-label={ariaLabel}
        disabled={disabled || undefined}
        {...hostProps}
        {...restWithoutClick}
        onClick={handleClick}
      >
        {children}
      </Host>
    );
  }
  return (
    <Pressable
      moniker={moniker}
      onPress={onPress}
      ariaLabel={ariaLabel}
      asChild={asChild}
      disabled={disabled}
      {...rest}
    >
      {children}
    </Pressable>
  );
}
