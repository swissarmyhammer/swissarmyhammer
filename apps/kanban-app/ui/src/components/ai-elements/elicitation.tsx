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
 *
 * # Spatial-nav integration
 *
 * Every field control is also a spatial-nav focus leaf so the user can jump
 * to it and arrow-navigate the form. Each control is wrapped in an
 * {@link AiPanelFocusScope} or {@link AiPanelPressable} (the graceful-degradation
 * wrappers that fall back to a bare host when no `<FocusLayer>` is present, so
 * these pure-render unit tests keep working). The wrapper moniker is a relative
 * segment composed under the panel's `ui:ai-panel` zone:
 * `ui:ai-panel.elicitation.field:{key}` for a field, and
 * `ui:ai-panel.elicitation.field:{key}.option:{value}` for one multiselect
 * option. Text-like inputs register a per-scope Enter "drill-in" command that
 * hands DOM focus to the input, and an Escape "drill-out" keydown handler that
 * blurs the input and returns spatial focus to the field leaf so the input
 * stops trapping keys (mirroring the composer's `ComposerEditorDrillOutWiring`);
 * the select / checkbox controls are `AiPanelPressable` leaves whose activation
 * toggles or opens the control (Radix returns focus to the trigger on Escape,
 * so they release on their own and need no drill-out wiring).
 */

import type { ChangeEvent, KeyboardEvent, RefObject } from "react";
import { useId, useMemo, useRef } from "react";

import type {
  ElicitationField,
  ElicitationFieldValue,
  ElicitationOption,
  FormErrors,
  FormValues,
} from "@/ai/elicitation";
import {
  AiPanelFocusScope,
  AiPanelPressable,
} from "@/components/ai-panel-focus";
import { useOptionalFullyQualifiedMoniker } from "@/components/fully-qualified-moniker-context";
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
import { useDispatchCommand, type CommandDef } from "@/lib/command-scope";
import { asSegment } from "@/types/spatial";

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
        <TextareaControl
          field={field}
          value={asText(value)}
          onChange={onChange}
          controlId={controlId}
          invalid={invalid}
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
    case "text":
      return (
        <TextInputControl
          field={field}
          value={asText(value)}
          onChange={onChange}
          controlId={controlId}
          invalid={invalid}
        />
      );
  }
}

/**
 * The relative spatial-nav segment for one elicitation field.
 *
 * Composed under the panel's `ui:ai-panel` zone by the enclosing primitive;
 * the per-field key keeps each leaf a unique path segment (the path-monikers
 * rule that flat monikers cause duplicate-registration ambiguity).
 */
function fieldSegment(key: string): string {
  return `ui:ai-panel.elicitation.field:${key}`;
}

/**
 * Build the per-scope Enter "drill-in" command for a text-like field.
 *
 * Landing on a field's `<FocusScope>` only registers it as a nav target; the
 * returned `CommandDef` (keyed to Enter for every keymap) hands DOM focus to
 * the referenced input so the user can start typing — the same drill-in
 * contract the composer's CM6 scope uses. Shared by the text and textarea
 * controls, whose only difference is the focused element type.
 *
 * @param key - The field's key, naming both the command id and (via
 *   {@link fieldSegment}) the scope it shadows Enter for.
 * @param inputRef - The control to focus on drill-in.
 * @returns A single-entry, memoized `CommandDef` array for `commands`.
 */
function useFieldDrillIn(
  key: string,
  inputRef: RefObject<HTMLElement | null>,
): readonly CommandDef[] {
  return useMemo<readonly CommandDef[]>(
    () => [
      {
        id: `ui.ai-panel.elicitation.field.drillIn:${key}`,
        name: "Edit Field",
        keys: { cua: "Enter", vim: "Enter", emacs: "Enter" },
        execute: () => {
          inputRef.current?.focus();
        },
      },
    ],
    [key, inputRef],
  );
}

/**
 * Build the Escape "drill-out" keydown handler for a text-like field's input.
 *
 * The mirror of {@link useFieldDrillIn}: drill-in (Enter on the scope) hands
 * DOM focus to the input, and this hands it back. Without it the focused input
 * traps every keystroke — Escape would insert nothing and never reach the
 * global `nav.drillOut`/`nav.jump` bindings, so the user could not launch the
 * command palette or the `s` jump command after starting to type.
 *
 * This is the elicitation analogue of `ComposerEditorDrillOutWiring` in
 * `ai-prompt-composer.tsx`. It MUST be called from a component rendered INSIDE
 * the field's {@link AiPanelFocusScope} so {@link useOptionalFullyQualifiedMoniker}
 * resolves to that scope's composed FQM (the field leaf), not the enclosing
 * scrollback zone. On Escape the returned handler:
 *
 *   1. blurs the active element so the caret stops and DOM focus leaves the
 *      input — the kernel's spatial-focus update alone does not move DOM focus;
 *   2. dispatches `nav.focus` with the field scope's FQM to return kernel
 *      spatial focus to the field leaf.
 *
 * Outside the spatial-nav stack (no `<FocusLayer>`, so the FQM context is
 * `null`) the handler is an inert no-op — exactly like the composer's, so the
 * bare {@link ElicitationFields} unit tests, which mount one control at a time
 * without the provider stack, stay green.
 *
 * @returns An `onKeyDown` handler for the field's `<Input>` / `<Textarea>`.
 */
function useFieldDrillOut(): (event: KeyboardEvent<HTMLElement>) => void {
  const fieldFq = useOptionalFullyQualifiedMoniker();
  // Focus claims flow through the single auditable `nav.focus` command — the
  // same primitive the composer drill-out and the field drill-in use.
  const dispatchNavFocus = useDispatchCommand("nav.focus");

  return useMemo<(event: KeyboardEvent<HTMLElement>) => void>(() => {
    return (event: KeyboardEvent<HTMLElement>) => {
      // `Escape` is the canonical drill-out key — `nav.drillOut` in every
      // keymap (see `BINDING_TABLES` in `@/lib/keybindings`). Match it here so
      // the field releases focus the same way the rest of the app drills out.
      if (event.key !== "Escape") return;
      // Outside the spatial-nav stack there is no scope to return focus to;
      // leave the event alone so the bare unit-test render is unaffected.
      if (!fieldFq) return;
      event.preventDefault();
      // Drop DOM focus from the input so the caret stops blinking — the
      // kernel's spatial-focus update alone does not move DOM focus.
      if (
        typeof document !== "undefined" &&
        document.activeElement instanceof HTMLElement
      ) {
        document.activeElement.blur();
      }
      void dispatchNavFocus({ args: { fq: fieldFq } }).catch((err) =>
        console.error("[useFieldDrillOut] nav.focus dispatch failed", err),
      );
    };
  }, [fieldFq, dispatchNavFocus]);
}

/** Props for the text-like ({@link Input}) control. */
interface TextInputControlProps {
  field: ElicitationField;
  value: string;
  onChange: (key: string, value: ElicitationFieldValue) => void;
  controlId: string;
  invalid: boolean;
}

/**
 * A text / number / integer field rendered as a shadcn {@link Input} inside a
 * spatial-nav focus leaf.
 *
 * The {@link AiPanelFocusScope} registers the field as the
 * `ui:ai-panel.elicitation.field:{key}` leaf so jump-to and arrow-nav can land
 * on it; a per-scope Enter "drill-in" command hands DOM focus to the input so
 * the user can start typing — the same land-on-the-scope / drill-into-the-editor
 * pattern the composer's CM6 scope uses. `number` / `integer` keep the textual
 * edit value (coercion happens in `toAcceptResponse`).
 */
function TextInputControl({
  field,
  value,
  onChange,
  controlId,
  invalid,
}: TextInputControlProps) {
  const inputRef = useRef<HTMLInputElement>(null);
  const drillIn = useFieldDrillIn(field.key, inputRef);
  return (
    <AiPanelFocusScope
      moniker={asSegment(fieldSegment(field.key))}
      commands={drillIn}
    >
      <TextInputControlBody
        field={field}
        value={value}
        onChange={onChange}
        controlId={controlId}
        invalid={invalid}
        inputRef={inputRef}
      />
    </AiPanelFocusScope>
  );
}

/** Props for {@link TextInputControlBody}. */
interface TextInputControlBodyProps extends TextInputControlProps {
  /** Ref to the shared `<Input>` — drives both drill-in focus and drill-out. */
  inputRef: RefObject<HTMLInputElement | null>;
}

/**
 * The `<Input>` body of {@link TextInputControl}, rendered INSIDE the field's
 * {@link AiPanelFocusScope} so {@link useFieldDrillOut} reads the field leaf's
 * composed FQM (not the enclosing scrollback zone). The Escape drill-out
 * handler releases DOM focus and returns spatial focus to the field leaf so the
 * input stops trapping keys.
 */
function TextInputControlBody({
  field,
  value,
  onChange,
  controlId,
  invalid,
  inputRef,
}: TextInputControlBodyProps) {
  const onKeyDown = useFieldDrillOut();
  const isInteger = field.kind === "integer";
  const isNumeric = isInteger || field.kind === "number";
  return (
    <Input
      ref={inputRef}
      id={controlId}
      type={isNumeric ? "number" : "text"}
      step={isNumeric ? (isInteger ? 1 : "any") : undefined}
      value={value}
      aria-invalid={invalid}
      onKeyDown={onKeyDown}
      onChange={(event: ChangeEvent<HTMLInputElement>) =>
        onChange(field.key, event.target.value)
      }
    />
  );
}

/** Props for the {@link Textarea} control. */
interface TextareaControlProps {
  field: ElicitationField;
  value: string;
  onChange: (key: string, value: ElicitationFieldValue) => void;
  controlId: string;
  invalid: boolean;
}

/**
 * A textarea field rendered as a shadcn {@link Textarea} inside a spatial-nav
 * focus leaf, mirroring {@link TextInputControl}: the field is the
 * `ui:ai-panel.elicitation.field:{key}` leaf and Enter drills DOM focus into
 * the textarea.
 */
function TextareaControl({
  field,
  value,
  onChange,
  controlId,
  invalid,
}: TextareaControlProps) {
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const drillIn = useFieldDrillIn(field.key, inputRef);
  return (
    <AiPanelFocusScope
      moniker={asSegment(fieldSegment(field.key))}
      commands={drillIn}
    >
      <TextareaControlBody
        field={field}
        value={value}
        onChange={onChange}
        controlId={controlId}
        invalid={invalid}
        inputRef={inputRef}
      />
    </AiPanelFocusScope>
  );
}

/** Props for {@link TextareaControlBody}. */
interface TextareaControlBodyProps extends TextareaControlProps {
  /** Ref to the shared `<Textarea>` — drives drill-in focus and drill-out. */
  inputRef: RefObject<HTMLTextAreaElement | null>;
}

/**
 * The `<Textarea>` body of {@link TextareaControl}, rendered INSIDE the field's
 * {@link AiPanelFocusScope} (mirroring {@link TextInputControlBody}) so
 * {@link useFieldDrillOut} resolves the field leaf's FQM and Escape releases
 * DOM focus back to the field leaf rather than trapping keys in the textarea.
 */
function TextareaControlBody({
  field,
  value,
  onChange,
  controlId,
  invalid,
  inputRef,
}: TextareaControlBodyProps) {
  const onKeyDown = useFieldDrillOut();
  return (
    <Textarea
      ref={inputRef}
      id={controlId}
      value={value}
      aria-invalid={invalid}
      onKeyDown={onKeyDown}
      onChange={(event: ChangeEvent<HTMLTextAreaElement>) =>
        onChange(field.key, event.target.value)
      }
    />
  );
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
 * A single-select rendered as a shadcn {@link Select} inside a spatial-nav
 * focus leaf.
 *
 * The trigger is labeled by the field's {@link Label} via `controlId`; picking
 * an option reports the chosen option's `value` string through `onChange`.
 *
 * The trigger is the `ui:ai-panel.elicitation.field:{key}` leaf via
 * {@link AiPanelPressable} `asChild`: activating it (Enter / Space) hands DOM
 * focus to the Radix trigger `<button>` so its listbox keyboard interaction
 * takes over — the exact hand-off `ComposerModelSelect` performs for the
 * model picker.
 */
function SelectControl({
  field,
  value,
  onChange,
  controlId,
  invalid,
}: SelectControlProps) {
  const triggerRef = useRef<HTMLButtonElement>(null);
  return (
    <Select
      value={value === "" ? undefined : value}
      onValueChange={(next) => onChange(field.key, next)}
    >
      <AiPanelPressable
        asChild
        moniker={asSegment(fieldSegment(field.key))}
        ariaLabel={`Select ${field.label}`}
        onPress={() => triggerRef.current?.focus()}
      >
        <SelectTrigger
          ref={triggerRef}
          id={controlId}
          aria-invalid={invalid}
          className="w-full"
        >
          <SelectValue placeholder={`Select ${field.label}`} />
        </SelectTrigger>
      </AiPanelPressable>
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
 * A boolean rendered as a shadcn {@link Checkbox} with an inline label inside a
 * spatial-nav focus leaf.
 *
 * The checkbox carries its own label (so the whole row is one clickable target)
 * and shows the required marker after the text. Toggling reports the new boolean
 * through `onChange`; Radix may emit an `"indeterminate"` state, which is
 * coerced to `false`.
 *
 * The checkbox is the `ui:ai-panel.elicitation.field:{key}` leaf via
 * {@link AiPanelPressable} `asChild`: activating it (Enter / Space) flips the
 * boolean directly, so the control is fully keyboard-operable from spatial nav.
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
      <AiPanelPressable
        asChild
        moniker={asSegment(fieldSegment(field.key))}
        ariaLabel={field.label}
        onPress={() => onChange(field.key, value !== true)}
      >
        <Checkbox
          id={controlId}
          checked={value}
          aria-invalid={invalid}
          onCheckedChange={(next) => onChange(field.key, next === true)}
        />
      </AiPanelPressable>
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
          fieldKey={field.key}
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
  /** The owning field's key — composes this option's leaf moniker. */
  fieldKey: string;
  option: ElicitationOption;
  checked: boolean;
  onToggle: (checked: boolean) => void;
}

/**
 * A single labeled checkbox within a {@link MultiselectControl}, wrapped as its
 * own spatial-nav focus leaf.
 *
 * Each option is the `ui:ai-panel.elicitation.field:{key}.option:{value}` leaf
 * via {@link AiPanelPressable} `asChild` so the user can jump to and arrow-nav
 * between options; activating one (Enter / Space) toggles that option's
 * membership.
 */
function MultiselectOption({
  fieldKey,
  option,
  checked,
  onToggle,
}: MultiselectOptionProps) {
  const id = useId();
  return (
    <div className="flex items-center gap-2">
      <AiPanelPressable
        asChild
        moniker={asSegment(`${fieldSegment(fieldKey)}.option:${option.value}`)}
        ariaLabel={option.label}
        onPress={() => onToggle(!checked)}
      >
        <Checkbox
          id={id}
          checked={checked}
          onCheckedChange={(next) => onToggle(next === true)}
        />
      </AiPanelPressable>
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
