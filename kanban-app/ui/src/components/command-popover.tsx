/**
 * `<CommandPopover>` — the picker form rendered inside the Radix Popover
 * anchored to a `<CommandButton>`.
 *
 * Iterates the command's `params` and renders one input per `shape`-bearing
 * entry. Submitting collects the picked values into a `{ [name]: value }`
 * bag and calls `onCommit` — the parent `<CommandButton>` then dispatches
 * the command with those args plus the resolved scope params.
 *
 * # Shape -> input mapping
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
 * Build the initial values bag for a command's pickable params at mount.
 *
 * Only `shape`-bearing params contribute slots — params with no `shape`
 * are resolved by the backend and never appear in the picker bag.
 */
function buildInitialValues(
  params: readonly ParamDef[] | undefined,
): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  if (!params) return out;
  for (const p of params) {
    if (p.shape === undefined) continue;
    out[p.name] = initialValueFor(p);
  }
  return out;
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
 * Picker form for one command. See file-level doc for the shape→input
 * mapping and the empty-options disable rule.
 */
export function CommandPopover({
  command,
  onCommit,
  onCancel,
}: CommandPopoverProps) {
  // Slot the picker bag once at mount; subsequent param edits update the
  // bag in place. We deliberately do not reset on command identity change
  // — the popover is mounted fresh from the parent on each open, and the
  // <CommandButton> re-keys via mount-on-open so a stale bag cannot
  // survive across activations.
  const [values, setValues] = useState<Record<string, unknown>>(() =>
    buildInitialValues(command.params),
  );

  // Capture the pickable subset once per command instance for stable iteration.
  const pickableParams = useMemo(
    () => (command.params ?? []).filter((p) => p.shape !== undefined),
    [command.params],
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
