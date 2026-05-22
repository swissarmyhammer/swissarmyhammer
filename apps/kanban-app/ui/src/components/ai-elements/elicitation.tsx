/**
 * Presentational controls for an ACP/MCP elicitation form.
 *
 * {@link ElicitationFields} turns the pure {@link ElicitationField} descriptors
 * produced by `@/ai/elicitation` into shadcn `@/components/ui` controls. It is
 * deliberately a *controlled* component: value in, `onChange(key, value)` out.
 * It holds no internal form state and contains no submit / accept / decline /
 * ACP logic — that container wiring lives in the sibling ElicitationPrompt.
 *
 * Keeping the rendering pure (no promises, no effects) is what makes each field
 * kind testable in isolation and reusable as an AI element. The value emitted
 * for each kind matches {@link ElicitationFieldValue}: `string` for text /
 * number / integer / select, `boolean` for boolean, `string[]` for multiselect.
 * Numeric fields stay textual here — coercion to JSON numbers happens later in
 * `toAcceptResponse`, so partial input like `"-"` stays representable.
 */

import type { ChangeEvent } from "react";
import { useId } from "react";

import type {
  ElicitationField,
  ElicitationFieldValue,
  ElicitationOption,
  FormErrors,
  FormValues,
} from "@/ai/elicitation";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";

/** Props for {@link ElicitationFields}. */
export interface ElicitationFieldsProps {
  /** The fields to render, in schema declaration order. */
  fields: ElicitationField[];
  /** The current value of every field, keyed by {@link ElicitationField.key}. */
  values: FormValues;
  /**
   * Report an edit to one field.
   *
   * @param key - The edited field's {@link ElicitationField.key}.
   * @param value - The new value in the field's natural editing type.
   */
  onChange: (key: string, value: ElicitationFieldValue) => void;
  /** Validation errors keyed by field key; a present entry renders inline. */
  errors?: FormErrors;
}

/**
 * Render a controlled labeled control for every elicitation field.
 *
 * Each field becomes a labeled row: a {@link Label} (with a required marker and
 * optional helper description), the kind-specific control from
 * {@link FieldControl}, and any matching `errors` entry shown beneath it.
 *
 * @param props - See {@link ElicitationFieldsProps}.
 * @returns The rendered form-field group.
 */
export function ElicitationFields({
  fields,
  values,
  onChange,
  errors,
}: ElicitationFieldsProps) {
  return (
    <div data-slot="elicitation-fields" className="flex flex-col gap-4">
      {fields.map((field) => (
        <ElicitationFieldRow
          key={field.key}
          field={field}
          value={values[field.key]}
          onChange={onChange}
          error={errors?.[field.key]}
        />
      ))}
    </div>
  );
}

/** Props for one rendered field row. */
interface ElicitationFieldRowProps {
  field: ElicitationField;
  value: ElicitationFieldValue | undefined;
  onChange: (key: string, value: ElicitationFieldValue) => void;
  error?: string;
}

/**
 * Render one field as a label + control + helper/error stack.
 *
 * The control is associated with the label via a generated id (the `boolean`
 * kind is the exception — its checkbox sits inline with its own label, so the
 * row renders no separate `Label`). Required fields show a `*` marker; a
 * `description` renders as muted helper text and an `error` as destructive text.
 */
function ElicitationFieldRow({
  field,
  value,
  onChange,
  error,
}: ElicitationFieldRowProps) {
  const controlId = useId();
  const isInlineLabel = field.kind === "boolean";

  return (
    <div data-slot="elicitation-field" className="flex flex-col gap-1.5">
      {!isInlineLabel && (
        <Label htmlFor={controlId}>
          {field.label}
          {field.required && <RequiredMarker />}
        </Label>
      )}
      <FieldControl
        field={field}
        value={value}
        onChange={onChange}
        controlId={controlId}
        invalid={error !== undefined}
      />
      {field.description && (
        <p className="text-xs text-muted-foreground">{field.description}</p>
      )}
      {error && (
        <p data-slot="elicitation-error" className="text-xs text-destructive">
          {error}
        </p>
      )}
    </div>
  );
}

/** The `*` shown after a required field's label. */
function RequiredMarker() {
  return (
    <span aria-hidden className="text-destructive">
      *
    </span>
  );
}

/** Props shared by every kind-specific control. */
interface FieldControlProps {
  field: ElicitationField;
  value: ElicitationFieldValue | undefined;
  onChange: (key: string, value: ElicitationFieldValue) => void;
  controlId: string;
  invalid: boolean;
}

/**
 * Render the control for a single field, dispatched on {@link ElicitationField.kind}.
 *
 * Each branch renders the documented shadcn control and reports edits through
 * `onChange` in the field's natural editing type. The switch is exhaustive over
 * {@link ElicitationFieldKind}.
 */
function FieldControl({
  field,
  value,
  onChange,
  controlId,
  invalid,
}: FieldControlProps) {
  switch (field.kind) {
    case "textarea":
      return (
        <Textarea
          id={controlId}
          value={asText(value)}
          aria-invalid={invalid}
          onChange={(event: ChangeEvent<HTMLTextAreaElement>) =>
            onChange(field.key, event.target.value)
          }
        />
      );
    case "select":
      return (
        <SelectControl
          field={field}
          value={asText(value)}
          onChange={onChange}
          controlId={controlId}
          invalid={invalid}
        />
      );
    case "boolean":
      return (
        <BooleanControl
          field={field}
          value={value === true}
          onChange={onChange}
          controlId={controlId}
          invalid={invalid}
        />
      );
    case "multiselect":
      return (
        <MultiselectControl
          field={field}
          value={asList(value)}
          onChange={onChange}
          invalid={invalid}
        />
      );
    case "number":
    case "integer":
      return (
        <Input
          id={controlId}
          type="number"
          step={field.kind === "integer" ? 1 : "any"}
          value={asText(value)}
          aria-invalid={invalid}
          onChange={(event: ChangeEvent<HTMLInputElement>) =>
            onChange(field.key, event.target.value)
          }
        />
      );
    case "text":
      return (
        <Input
          id={controlId}
          type="text"
          value={asText(value)}
          aria-invalid={invalid}
          onChange={(event: ChangeEvent<HTMLInputElement>) =>
            onChange(field.key, event.target.value)
          }
        />
      );
  }
}

/** Props shared by the composite (Select / boolean / multiselect) controls. */
interface SelectControlProps {
  field: ElicitationField;
  value: string;
  onChange: (key: string, value: ElicitationFieldValue) => void;
  controlId: string;
  invalid: boolean;
}

/**
 * A single-select rendered as a shadcn {@link Select}.
 *
 * The trigger is labeled by the field's {@link Label} via `controlId`; picking
 * an option reports the chosen option's `value` string through `onChange`.
 */
function SelectControl({
  field,
  value,
  onChange,
  controlId,
  invalid,
}: SelectControlProps) {
  return (
    <Select
      value={value === "" ? undefined : value}
      onValueChange={(next) => onChange(field.key, next)}
    >
      <SelectTrigger id={controlId} aria-invalid={invalid} className="w-full">
        <SelectValue placeholder={`Select ${field.label}`} />
      </SelectTrigger>
      <SelectContent>
        {(field.options ?? []).map((option) => (
          <SelectItem key={option.value} value={option.value}>
            {option.label}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}

/** Props for the boolean checkbox control. */
interface BooleanControlProps {
  field: ElicitationField;
  value: boolean;
  onChange: (key: string, value: ElicitationFieldValue) => void;
  controlId: string;
  invalid: boolean;
}

/**
 * A boolean rendered as a shadcn {@link Checkbox} with an inline label.
 *
 * The checkbox carries its own label (so the whole row is one clickable target)
 * and shows the required marker after the text. Toggling reports the new boolean
 * through `onChange`; Radix may emit an `"indeterminate"` state, which is
 * coerced to `false`.
 */
function BooleanControl({
  field,
  value,
  onChange,
  controlId,
  invalid,
}: BooleanControlProps) {
  return (
    <div className="flex items-center gap-2">
      <Checkbox
        id={controlId}
        checked={value}
        aria-invalid={invalid}
        onCheckedChange={(next) => onChange(field.key, next === true)}
      />
      <Label htmlFor={controlId}>
        {field.label}
        {field.required && <RequiredMarker />}
      </Label>
    </div>
  );
}

/** Props for the multiselect checkbox-group control. */
interface MultiselectControlProps {
  field: ElicitationField;
  value: string[];
  onChange: (key: string, value: ElicitationFieldValue) => void;
  invalid: boolean;
}

/**
 * A multiselect rendered as a vertical group of shadcn {@link Checkbox}es.
 *
 * Each option toggles its membership in the selected `string[]` (preserving the
 * options' declaration order), and the whole next array is reported through
 * `onChange`.
 */
function MultiselectControl({
  field,
  value,
  onChange,
  invalid,
}: MultiselectControlProps) {
  return (
    <div role="group" aria-invalid={invalid} className="flex flex-col gap-2">
      {(field.options ?? []).map((option) => (
        <MultiselectOption
          key={option.value}
          option={option}
          checked={value.includes(option.value)}
          onToggle={(checked) =>
            onChange(
              field.key,
              nextSelection(field.options, value, option.value, checked),
            )
          }
        />
      ))}
    </div>
  );
}

/** Props for one multiselect option row. */
interface MultiselectOptionProps {
  option: ElicitationOption;
  checked: boolean;
  onToggle: (checked: boolean) => void;
}

/** A single labeled checkbox within a {@link MultiselectControl}. */
function MultiselectOption({
  option,
  checked,
  onToggle,
}: MultiselectOptionProps) {
  const id = useId();
  return (
    <div className="flex items-center gap-2">
      <Checkbox
        id={id}
        checked={checked}
        onCheckedChange={(next) => onToggle(next === true)}
      />
      <Label htmlFor={id}>{option.label}</Label>
    </div>
  );
}

/**
 * Compute the next multiselect array after toggling one option.
 *
 * Adding re-derives the array from the option order so the result stays in
 * declaration order regardless of click sequence; removing filters the option
 * out. Returns a fresh array — the input is never mutated.
 *
 * @param options - The field's options, in declaration order.
 * @param current - The currently selected option values.
 * @param toggled - The option value being toggled.
 * @param checked - Whether the option is now checked.
 * @returns The next selected-values array.
 */
function nextSelection(
  options: ElicitationOption[] | undefined,
  current: string[],
  toggled: string,
  checked: boolean,
): string[] {
  if (!checked) {
    return current.filter((value) => value !== toggled);
  }
  const selected = new Set([...current, toggled]);
  return (options ?? [])
    .map((option) => option.value)
    .filter((value) => selected.has(value));
}

/** Coerce a possibly-undefined field value to its text editing form. */
function asText(value: ElicitationFieldValue | undefined): string {
  return typeof value === "string" ? value : "";
}

/** Coerce a possibly-undefined field value to its list editing form. */
function asList(value: ElicitationFieldValue | undefined): string[] {
  return Array.isArray(value) ? value : [];
}
