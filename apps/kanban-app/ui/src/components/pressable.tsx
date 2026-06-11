/**
 * `<Pressable>` — the canonical primitive for an actionable icon button.
 *
 * Bundles the three concerns every actionable icon button must satisfy
 * into a single primitive:
 *
 *   1. Mounts a `<FocusScope>` leaf so the spatial-nav graph can
 *      navigate to it.
 *   2. Renders a `<button type="button">` (or, via `asChild`, an
 *      arbitrary host like a Radix `<TooltipTrigger asChild>` slot).
 *   3. Wires the plugin-defined activation commands
 *      (`pressable.activate` — Enter vim/cua, `pressable.activateSpace`
 *      — Space cua, both defined by the `ui-commands` builtin plugin)
 *      to the same `onPress` callback as the button's `onClick`, by
 *      registering webview command-bus handlers while the leaf is the
 *      spatial focus.
 *
 * # Command split (Card D, ui-command-cleanup)
 *
 * The activation command DEFINITIONS (id / name / keys / scope) live in
 * `builtin/plugins/ui-commands/index.ts::UI_SURFACE_COMMANDS` — this
 * component defines no `CommandDef`. Their `scope: ["ui:pressable"]`
 * names the constant marker moniker this component mounts via a
 * `CommandScopeProvider` directly above its `<FocusScope>`
 * ({@link PRESSABLE_COMMAND_SCOPE}), so the keymap layer binds
 * Enter / Space only while a pressable leaf is in the focused chain.
 * The live behavior — the local `onPress` closure — registers on the
 * webview command bus per id while spatial focus is within the leaf's
 * subtree (`useFocusedWebviewCommandHandlers`; for a pressable that
 * means direct focus — it is a spatial LEAF, and a registered
 * `<FocusScope>` cannot contain another, so nothing ever nests inside
 * it), so a dispatch always reaches the focused instance. The handler
 * is pure presentation: it calls the caller's `onPress`, which
 * re-dispatches through `useDispatchCommand` when it needs a durable
 * effect.
 *
 * # Why a primitive, not a hand-rolled per-site shape
 *
 * Before this primitive, icon buttons across the UI were inconsistently
 * wired into the spatial-nav and keyboard-activation contracts. Some
 * sites wrapped a `<button onClick={…}>` in a `<FocusScope>` — keyboard
 * focusable but Enter did NOTHING (the kernel's `nav.drillIn` returns
 * the focused FQM for a leaf, `setFocus` is idempotent, the visible
 * effect is a no-op). Other sites were bare `<button>` with no spatial
 * registration at all — keyboard users could not focus them.
 *
 * `<Pressable>` enforces the contract at the component level: the only
 * way to render an actionable icon button is through this primitive.
 *
 * # Exception list
 *
 * Sites that legitimately do NOT use `<Pressable>`:
 *
 *   - **Purely decorative** affordances with no `onPress` (icons-as-content,
 *     status indicators).
 *   - **Mouse-only-by-design** affordances such as the card drag handle
 *     (see task `01KQM9478XFMCBBWHQN6ARE524` — the drag handle has no
 *     keyboard activation story because dnd-kit on the board uses
 *     `PointerSensor` only, no `KeyboardSensor`).
 *
 * Every other actionable icon button MUST migrate to `<Pressable>`.
 *
 * # asChild composition
 *
 * When wrapped in a Radix slot like `<TooltipTrigger asChild>`, pass
 * `asChild` so the trigger's slot becomes the host element rather than
 * `<Pressable>` rendering its own `<button>`. The chain
 * `<TooltipTrigger asChild><Pressable asChild>...</Pressable></TooltipTrigger>`
 * renders exactly one `<button>` in the DOM — the host child carries
 * the trigger props, the press handlers, and the aria-label.
 *
 * # Pointer-event composition (when to use `e.stopPropagation()`)
 *
 * `onPress` is the activation contract — pointer (`onClick`) and
 * keyboard (Enter / Space via the scope-level CommandDefs) both route
 * through it. Pressable itself deliberately does NOT call
 * `e.stopPropagation()` on the underlying click event: many call sites
 * rely on benign click bubbling (e.g. `column-view.tsx::AddTaskButton`
 * lets the click bubble up to the enclosing column `<FocusZone>`'s own
 * onClick because both handlers seed focus to the same column FQM, so
 * the duplication is a no-op).
 *
 * When a call site needs to suppress click bubble — typically when the
 * host wraps `<Pressable asChild>` with another clickable container
 * (e.g. a card zone) whose own onClick would steal focus or otherwise
 * misfire — add `onClick={(e) => e.stopPropagation()}` directly on the
 * inner `<button>` in `asChild` mode. Radix Slot's `mergeProps`
 * composes inner-then-slot ordering: the inner `<button>`'s
 * `stopPropagation()` runs FIRST, then Pressable's own `handleClick`
 * fires `onPress`. Both run synchronously inside the same React event
 * handler invocation, so the activation dispatch via `onPress` still
 * lands, but parent containers' click handlers do not see the event.
 *
 * Canonical reference site: `entity-card.tsx::InspectButton` — the (i)
 * Info button on a task card sits inside a `<FocusZone>` for the card,
 * and the inner `<button onClick={(e) => e.stopPropagation()}>` keeps
 * the card zone's own click-to-focus handler from firing alongside the
 * inspect dispatch.
 *
 * Five additional sites slated for migration in follow-up task
 * `01KQPZAFSPJEMHMKRSQGPD0JM6` will need the same guidance — consult
 * this section before deciding whether a given site needs the inner
 * `stopPropagation()` shim.
 */

import {
  forwardRef,
  useCallback,
  useMemo,
  useRef,
  type ButtonHTMLAttributes,
  type ForwardedRef,
  type MouseEvent as ReactMouseEvent,
  type ReactNode,
} from "react";
import { Slot } from "radix-ui";
import { FocusScope } from "@/components/focus-scope";
import { CommandScopeProvider } from "@/lib/command-scope";
import { useFocusedWebviewCommandHandlers } from "@/lib/use-focused-webview-command-handlers";
import type { WebviewCommandHandler } from "@/lib/webview-command-bus";
import type { SegmentMoniker } from "@/types/spatial";

/**
 * The constant marker moniker every `<Pressable>` mounts into the command
 * scope chain, directly above its `<FocusScope>` leaf.
 *
 * Pressable leaves carry arbitrary per-site monikers
 * (`"ui:navbar.inspect"`, `"card.inspect:T1"`, …), so the plugin-defined
 * activation commands cannot be scope-gated on a literal leaf moniker the
 * way the grid's `ui:grid` zone is. The marker gives every pressable one
 * shared literal moniker; the `ui-commands` plugin's
 * `pressable.activate` / `pressable.activateSpace` declare
 * `scope: ["ui:pressable"]` against it, so their Enter / Space keys bind
 * exactly while a pressable leaf is the spatial focus — and nowhere else.
 */
export const PRESSABLE_COMMAND_SCOPE = "ui:pressable";

/** Public props for `<Pressable>`. */
export interface PressableProps extends Omit<
  ButtonHTMLAttributes<HTMLButtonElement>,
  "onClick"
> {
  /**
   * Relative `SegmentMoniker` for the spatial-nav leaf this Pressable
   * registers — composed under the parent FQM by the enclosing
   * `<FocusScope>`. Examples: `"ui:navbar.inspect"`,
   * `"card.inspect:T1"`, `"ui:perspective-bar.add"`.
   */
  moniker: SegmentMoniker;
  /**
   * Activation callback — fires identically through both paths
   * (mouse / pointer click on the host button, and Enter / Space on
   * the focused leaf via the scope-level CommandDefs).
   */
  onPress: () => void;
  /**
   * Required accessible label. Every icon button needs one — the
   * primitive enforces this so it cannot be forgotten.
   */
  ariaLabel: string;
  /**
   * When true, render via Radix `<Slot.Root>` so a parent slot like
   * `<TooltipTrigger asChild>` becomes the host element. The slot
   * still receives the `onClick`, `aria-label`, and `disabled` props
   * so any underlying `<button>` participates in activation.
   *
   * Default false — render an internal `<button type="button">`.
   */
  asChild?: boolean;
  /**
   * When true, suppresses both `onClick` activation and the keyboard
   * activation CommandDefs (the `execute` closures short-circuit).
   * The host button also receives `disabled`, so it gains the native
   * disabled affordances (no focus ring, pointer-events:none, etc.).
   */
  disabled?: boolean;
  /** Children rendered inside the host element (typically an icon). */
  children?: ReactNode;
}

/**
 * Wire the plugin-defined activation ids (`pressable.activate` /
 * `pressable.activateSpace`) to this pressable's `onPress` while its leaf
 * is the spatial focus.
 *
 * Both ids share one guarded behavior: when `disabled` is true the closure
 * short-circuits so keyboard activation matches the suppressed `onClick`.
 * The latest `onPress` / `disabled` are read through a ref at invocation
 * time, so the bus registration never churns on prop identity. Two separate
 * command ids exist (rather than one with two keys) because each `keys`
 * entry in the plugin's command metadata is one binding per keymap — Enter
 * binds in vim + cua, Space in cua only.
 */
function usePressActivationHandlers(
  moniker: SegmentMoniker,
  onPress: () => void,
  disabled: boolean,
): void {
  const pressRef = useRef({ onPress, disabled });
  pressRef.current = { onPress, disabled };
  const handlers = useMemo<
    Readonly<Record<string, WebviewCommandHandler>>
  >(() => {
    const guarded = () => {
      const { onPress: press, disabled: isDisabled } = pressRef.current;
      if (!isDisabled) press();
    };
    return {
      "pressable.activate": guarded,
      "pressable.activateSpace": guarded,
    };
  }, []);
  useFocusedWebviewCommandHandlers(moniker, handlers);
}

/**
 * The canonical primitive for an actionable icon button — see the
 * file-level docstring for the contract and exception list.
 */
export const Pressable = forwardRef<HTMLButtonElement, PressableProps>(
  function Pressable(
    {
      moniker,
      onPress,
      ariaLabel,
      asChild = false,
      disabled = false,
      children,
      ...rest
    }: PressableProps,
    ref: ForwardedRef<HTMLButtonElement>,
  ) {
    usePressActivationHandlers(moniker, onPress, disabled);

    // Pull a passthrough `onClick` out of `rest` so we can compose it
    // with our own activation handler. The contract document says
    // `<Pressable>` omits the `onClick` prop in its TS surface (see
    // `PressableProps`), but in `asChild` mode a parent slot like
    // `<TooltipTrigger asChild>` injects its own `onClick` via
    // `cloneElement` at runtime — and that prop has to fire alongside
    // ours, not replace it. Composing here keeps both the parent
    // slot's handler (e.g. tooltip dismiss-on-click) and our own
    // `onPress` firing on every click.
    const { onClick: rawOuterOnClick, ...restWithoutClick } =
      rest as ButtonHTMLAttributes<HTMLButtonElement>;
    const outerOnClick = rawOuterOnClick as
      | ((e: ReactMouseEvent<HTMLButtonElement>) => void)
      | undefined;

    const handleClick = useCallback(
      (e: ReactMouseEvent<HTMLButtonElement>) => {
        // Run any outer-slotted handler first so its `e.preventDefault()`
        // / `e.stopPropagation()` calls land before our activation —
        // mirrors radix's `composeEventHandlers` ordering.
        outerOnClick?.(e);
        if (e.defaultPrevented) return;
        if (!disabled) onPress();
      },
      [onPress, disabled, outerOnClick],
    );

    const Host = asChild ? Slot.Root : "button";
    // The button-only `type` attribute is unsafe to forward through
    // Slot.Root (it would land on whatever underlying element a parent
    // slot picks). Render `<button type="button">` only on the
    // non-asChild path.
    const buttonProps = asChild === true ? {} : { type: "button" as const };

    // The marker `CommandScopeProvider` sits directly above the
    // `<FocusScope>` so the leaf's command-scope chain contains the
    // literal `ui:pressable` moniker — the gate the plugin-defined
    // activation commands' `scope` names (see PRESSABLE_COMMAND_SCOPE).
    return (
      <CommandScopeProvider moniker={PRESSABLE_COMMAND_SCOPE}>
        <FocusScope moniker={moniker}>
          <Host
            ref={ref}
            aria-label={ariaLabel}
            disabled={disabled || undefined}
            {...buttonProps}
            {...restWithoutClick}
            onClick={handleClick}
          >
            {children}
          </Host>
        </FocusScope>
      </CommandScopeProvider>
    );
  },
);
