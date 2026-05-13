/**
 * `<CommandPopover>` — the picker rendered inside the Radix Popover
 * anchored to a `<CommandButton>`.
 *
 * Two render shapes, picked by `params`:
 *
 *   - **Single-enum-param commands** — when the only pickable param is
 *     `shape: enum`, the popover body is a vertical list of clickable
 *     option buttons. Clicking an option commits immediately (`onCommit`
 *     is called with `{ [paramName]: pickedValue }`) — picking IS the
 *     action. No Submit button is rendered. This is the common case for
 *     Group By, single-arg pickers, and the clear-command sentinel.
 *
 *   - **Multi-param / mixed-shape commands** — when the command exposes
 *     two or more pickable params, or a single non-enum pickable param,
 *     the popover renders the legacy form (one input per param + Submit).
 *     Multi-param submission requires gathering N values before dispatch,
 *     so single-click-IS-submit doesn't apply.
 *
 * # Shape -> input mapping (form branch)
 *
 *   - `enum`       → `<select>` populated from `param.options`. When the
 *                    list is empty or absent the select is rendered as
 *                    disabled — the backend resolver did not have enough
 *                    information to supply choices, and forcing a submit
 *                    against an empty picker would dispatch garbage.
 *   - `text`       → `<input type="text">`.
 *   - `number`     → `<input type="number">`.
 *   - `date`       → `<input type="date">`.
 *   - `boolean`    → `<input type="checkbox">`.
 *   - `expression` → {@link FilterExpressionEditor} (filter DSL editor).
 *
 * Params with no `shape` are skipped — their values are resolved by the
 * backend from `from: scope_chain | target | args | default`. A command
 * whose every param has no `shape` doesn't reach this popover at all
 * (the button dispatches immediately).
 *
 * # Clear sentinel ("(none)")
 *
 * When a param declares `clear_command`, the popover prepends a "(none)"
 * entry — in the one-click menu it's the first button; in the form
 * branch it's the first `<option>` of the select. Picking it commits the
 * empty-string sentinel for that param; `<CommandButton>`'s commit
 * handler then redirects the dispatch to `clear_command`.
 */

import { useCallback, useMemo, useState } from "react";
import { cn } from "@/lib/utils";
import { FilterExpressionEditor } from "./filter-editor";
import type { CommandDef, ParamDef } from "@/types/kanban";

/** Props for `<CommandPopover>`. */
export interface CommandPopoverProps {
  /** The command whose pickable params drive the form fields. */
  command: CommandDef;
  /**
   * Called when the form is submitted with the picked values keyed by
   * param name. Params with no `shape` are absent from the bag.
   */
  onCommit: (args: Record<string, unknown>) => void;
  /** Called when the user cancels the popover (Escape, click-outside). */
  onCancel?: () => void;
}

/**
 * Initial value for a param's state slot.
 *
 * Falls back to a shape-appropriate zero value so the input renders in a
 * controlled state from first mount.
 */
function initialValueFor(param: ParamDef): unknown {
  if (param.default !== undefined) return param.default;
  switch (param.shape) {
    case "number":
      return 0;
    case "boolean":
      return false;
    case "enum":
    case "text":
    case "expression":
    case "date":
    default:
      return "";
  }
}

/**
 * Render an `<input>` / `<select>` / expression-editor for one
 * pickable param. Each field calls `setValue` to update the picker bag.
 */
function ParamField({
  param,
  value,
  setValue,
}: {
  param: ParamDef;
  value: unknown;
  setValue: (v: unknown) => void;
}) {
  switch (param.shape) {
    case "enum":
      return <EnumField param={param} value={value} setValue={setValue} />;
    case "text":
      return (
        <input
          aria-label={param.name}
          type="text"
          value={value as string}
          onChange={(e) => setValue(e.target.value)}
          className="h-8 px-2 text-sm border rounded-md bg-background"
        />
      );
    case "number":
      return (
        <input
          aria-label={param.name}
          type="number"
          value={value as number}
          onChange={(e) => setValue(Number(e.target.value))}
          className="h-8 px-2 text-sm border rounded-md bg-background"
        />
      );
    case "date":
      return (
        <input
          aria-label={param.name}
          type="date"
          value={value as string}
          onChange={(e) => setValue(e.target.value)}
          className="h-8 px-2 text-sm border rounded-md bg-background"
        />
      );
    case "boolean":
      return (
        <input
          aria-label={param.name}
          type="checkbox"
          checked={value as boolean}
          onChange={(e) => setValue(e.target.checked)}
          className="h-4 w-4"
        />
      );
    case "expression":
      // The CM6 editor mounts a contenteditable `<div>` rather than a
      // native form control, so the wrapping `<label>`/`<span>` text in
      // the form doesn't reach assistive tech via the normal control
      // association. The explicit `aria-label` on this wrapper supplies
      // an accessible name to AT clients that walk the CM6 editor's
      // role=textbox child. Other branches put the `aria-label` on the
      // native input itself; for `expression` it lives one level up.
      return (
        <div
          aria-label={param.name}
          className="border rounded-md bg-background px-2 py-1 min-w-[16rem]"
        >
          <FilterExpressionEditor
            value={value as string}
            onChange={setValue}
            autoFocus={false}
          />
        </div>
      );
    default:
      return null;
  }
}

/**
 * `<select>` for an enum-shaped param. Disabled when the option list is
 * empty or undefined (backend resolver did not supply choices).
 *
 * # Clear sentinel ("None" affordance)
 *
 * When the param declares `clear_command`, the placeholder option
 * (`value=""`) is relabelled from "Pick…" to "(none)" and is treated as
 * a real, submittable choice rather than the "no selection yet" stub.
 * Picking it dispatches the param's `clear_command` instead of the
 * parent command (the redirection lives in `<CommandButton>`'s commit
 * handler) — this restores the legacy `<GroupSelector>` "None" entry
 * the command-driven-ui migration would otherwise drop.
 *
 * Surfaces without `clear_command` keep the original behavior: the
 * empty-string placeholder is a "must pick something" stub and the
 * submit button stays disabled until the user picks a real option.
 */
function EnumField({
  param,
  value,
  setValue,
}: {
  param: ParamDef;
  value: unknown;
  setValue: (v: unknown) => void;
}) {
  const options = param.options ?? [];
  const disabled = options.length === 0;
  const hasClear = param.clear_command !== undefined;
  return (
    <select
      aria-label={param.name}
      value={value as string}
      onChange={(e) => setValue(e.target.value)}
      disabled={disabled}
      className={cn(
        "h-8 px-2 text-sm border rounded-md bg-background",
        disabled && "opacity-50 cursor-not-allowed",
      )}
    >
      <option value="">{hasClear ? "(none)" : "Pick…"}</option>
      {options.map((o) => (
        <option key={o.value} value={o.value}>
          {o.label}
        </option>
      ))}
    </select>
  );
}

/**
 * One-click menu body for a single-enum-param command.
 *
 * Renders the param's options (plus the "(none)" clear sentinel when
 * `clear_command` is declared) as a vertical list of buttons. Clicking
 * any button commits the picker bag — `{ [param.name]: option.value }`
 * for a real option, `{ [param.name]: "" }` for the clear sentinel —
 * with no intermediate Submit step.
 *
 * Empty / undefined `param.options` renders the empty-state placeholder
 * — the backend resolver did not have enough information to supply
 * choices, and forcing the user to click an absent option would be
 * meaningless. The "(none)" sentinel still renders when `clear_command`
 * is set, because clearing state is still a valid action even when the
 * resolver supplied no real options.
 */
function EnumMenu({
  param,
  onPick,
}: {
  param: ParamDef;
  onPick: (value: string) => void;
}) {
  const options = param.options ?? [];
  const hasClear = param.clear_command !== undefined;
  const isEmpty = options.length === 0 && !hasClear;

  if (isEmpty) {
    // No real options AND no clear sentinel — render the disabled
    // placeholder so the user can see the popover opened but knows
    // nothing is pickable. Mirrors the legacy "disabled <select>"
    // affordance from the form branch.
    return (
      <div
        aria-label={param.name}
        className="text-xs text-muted-foreground italic px-2 py-1"
      >
        No options available
      </div>
    );
  }

  // Use native `<ul><li><button>` markup rather than Radix Menu / explicit
  // ARIA `menu` roles. The plain-button shape keeps each option focusable
  // and clickable (one click = commit) without dragging in the Radix Menu
  // primitives' roving-tabindex / arrow-key navigation, which would
  // conflict with the spatial-nav graph that already governs focus order
  // for the surrounding tab bar.
  return (
    <ul
      aria-label={param.name}
      className="flex flex-col gap-0.5 min-w-[12rem] list-none p-0 m-0"
    >
      {hasClear && (
        <li>
          <button
            type="button"
            onClick={() => onPick("")}
            className="w-full text-left px-2 py-1 text-sm rounded-md hover:bg-muted/60 transition-colors italic text-muted-foreground"
          >
            (none)
          </button>
        </li>
      )}
      {options.map((o) => (
        <li key={o.value}>
          <button
            type="button"
            onClick={() => onPick(o.value)}
            className="w-full text-left px-2 py-1 text-sm rounded-md hover:bg-muted/60 transition-colors"
          >
            {o.label}
          </button>
        </li>
      ))}
    </ul>
  );
}

/**
 * True when the command's pickable params reduce to exactly one
 * enum-shaped entry — the trigger for the one-click menu render.
 *
 * Mixed-shape commands (e.g. one enum + one text) and multi-enum
 * commands stay in the form branch because picking a single value is
 * not enough to commit.
 */
function isSingleEnumMenuCommand(pickableParams: readonly ParamDef[]): boolean {
  return pickableParams.length === 1 && pickableParams[0].shape === "enum";
}

/**
 * Picker for one command. Renders either the one-click menu (single
 * enum param) or the multi-field form (everything else). See file-level
 * doc for the branching rule and shape→input mapping.
 */
export function CommandPopover({
  command,
  onCommit,
  onCancel,
}: CommandPopoverProps) {
  // Capture the pickable subset once per command instance for stable iteration.
  const pickableParams = useMemo(
    () => (command.params ?? []).filter((p) => p.shape !== undefined),
    [command.params],
  );

  if (isSingleEnumMenuCommand(pickableParams)) {
    const param = pickableParams[0];
    return (
      <div
        className="flex flex-col gap-2 min-w-[12rem]"
        data-testid="command-popover"
      >
        <EnumMenu
          param={param}
          onPick={(value) => onCommit({ [param.name]: value })}
        />
      </div>
    );
  }

  return (
    <CommandPopoverForm
      pickableParams={pickableParams}
      onCommit={onCommit}
      onCancel={onCancel}
    />
  );
}

/**
 * Multi-param form body — one input per pickable param plus a Submit
 * button. Used for commands with two or more pickable params, or a
 * single non-enum pickable param. Single-enum commands take the
 * one-click `EnumMenu` branch instead.
 */
function CommandPopoverForm({
  pickableParams,
  onCommit,
  onCancel,
}: {
  pickableParams: readonly ParamDef[];
  onCommit: (args: Record<string, unknown>) => void;
  onCancel?: () => void;
}) {
  // Slot the picker bag once at mount; subsequent param edits update the
  // bag in place. We deliberately do not reset on command identity change
  // — the popover is mounted fresh from the parent on each open, and the
  // <CommandButton> re-keys via mount-on-open so a stale bag cannot
  // survive across activations.
  const [values, setValues] = useState<Record<string, unknown>>(() =>
    buildInitialValuesFor(pickableParams),
  );

  // Submit is disabled when any enum param's slot is still the empty
  // placeholder AND that param does NOT carry `clear_command`. The
  // backend would reject `{ field: "" }` against a no-clear param
  // anyway; gating here gives a better UX than dispatching garbage and
  // surfacing the failure to console.
  //
  // When `clear_command` is set, the empty-string value is a legitimate
  // "clear" submission (handled in <CommandButton>'s commit handler),
  // so it must NOT disable submit. Other shapes (text, number, etc.)
  // accept any value the user can type, so we don't gate on them.
  const submitDisabled = useMemo(
    () =>
      pickableParams.some(
        (p) =>
          p.shape === "enum" &&
          p.clear_command === undefined &&
          (values[p.name] === "" || values[p.name] === undefined),
      ),
    [pickableParams, values],
  );

  const handleSubmit = useCallback(
    (e?: React.FormEvent) => {
      e?.preventDefault();
      if (submitDisabled) return;
      onCommit(values);
    },
    [onCommit, values, submitDisabled],
  );

  return (
    <form
      onSubmit={handleSubmit}
      className="flex flex-col gap-3 min-w-[14rem]"
      data-testid="command-popover"
    >
      {pickableParams.map((p) => (
        <label key={p.name} className="flex flex-col gap-1 text-xs">
          <span className="text-muted-foreground">{p.name}</span>
          <ParamField
            param={p}
            value={values[p.name]}
            setValue={(v) => setValues((prev) => ({ ...prev, [p.name]: v }))}
          />
        </label>
      ))}
      <div className="flex justify-end gap-2 pt-1">
        {onCancel && (
          <button
            type="button"
            onClick={onCancel}
            className="h-7 px-3 text-xs rounded-md border bg-background hover:bg-muted/50 transition-colors"
          >
            Cancel
          </button>
        )}
        <button
          type="submit"
          disabled={submitDisabled}
          className={cn(
            "h-7 px-3 text-xs rounded-md bg-primary text-primary-foreground transition-colors",
            submitDisabled
              ? "opacity-50 cursor-not-allowed"
              : "hover:bg-primary/90",
          )}
        >
          Submit
        </button>
      </div>
    </form>
  );
}

/**
 * Build the initial values bag for the form branch's pickable params at
 * mount. One slot per param, seeded by {@link initialValueFor}. The form
 * branch's caller pre-filters the param list (drops the no-`shape`
 * entries the backend resolves), so this helper iterates the input as
 * given without re-filtering.
 */
function buildInitialValuesFor(
  params: readonly ParamDef[],
): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  for (const p of params) {
    out[p.name] = initialValueFor(p);
  }
  return out;
}
