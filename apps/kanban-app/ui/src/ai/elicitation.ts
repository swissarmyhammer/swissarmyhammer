/**
 * Pure, React-free model for ACP/MCP elicitation forms.
 *
 * An ACP agent requests structured input from the user with a
 * {@link CreateElicitationRequest}. In *form* mode the request carries a
 * JSON-Schema-like {@link ElicitationSchema} describing the fields to render;
 * in *url* mode it directs the user to a URL. The panel renders those fields
 * "as documented in MCP" and coerces the user's input back into a typed
 * {@link CreateElicitationResponse}.
 *
 * This module owns that schema -> form -> response translation with no React
 * dependency, the same split as `conversation.ts`'s pure reducer. Keeping it
 * pure is what makes every field kind, validation rule, and coercion
 * exhaustively unit-testable.
 *
 * # The flow
 *
 * 1. {@link parseElicitation} turns a request into either a list of
 *    {@link ElicitationField} descriptors (form mode) or a url branch.
 * 2. {@link initialFormState} seeds the editable {@link FormValues} from each
 *    field's schema default.
 * 3. The panel edits the {@link FormValues} as the user types.
 * 4. {@link validateForm} reports missing-required and type errors.
 * 5. {@link toAcceptResponse} coerces the form values back to the JSON types
 *    the schema requested and wraps them in an `accept` response.
 *
 * {@link declineResponse} and {@link cancelResponse} are the other two
 * outcomes the user can choose.
 *
 * # Field-kind mapping
 *
 * | `ElicitationPropertySchema` `type` | Field kind                          |
 * |------------------------------------|-------------------------------------|
 * | `string` + `enum`/`oneOf`          | `select` (single-select)            |
 * | `string` (long-form `maxLength`)   | `textarea`                          |
 * | `string` (otherwise)               | `text`                              |
 * | `number`                           | `number`                            |
 * | `integer`                          | `integer`                           |
 * | `boolean`                          | `boolean`                           |
 * | `array`                            | `multiselect`                       |
 */
import type {
  CreateElicitationRequest,
  CreateElicitationResponse,
  ElicitationContentValue,
  ElicitationPropertySchema,
  ElicitationSchema,
  StringFormat,
} from "@agentclientprotocol/sdk";

/**
 * The renderable kind of a form field.
 *
 * Each kind corresponds to one input control the panel renders. `text` and
 * `textarea` are both backed by a `string` schema; the split is purely a
 * presentation hint for short vs long-form input.
 */
export type ElicitationFieldKind =
  | "text"
  | "textarea"
  | "select"
  | "boolean"
  | "number"
  | "integer"
  | "multiselect";

/**
 * A single selectable option for a `select` or `multiselect` field.
 *
 * `value` is the constant string sent back in the response; `label` is the
 * human-readable text shown to the user. For untitled enums the two are equal.
 */
export interface ElicitationOption {
  /** The constant value placed in the response when this option is chosen. */
  value: string;
  /** The human-readable label shown in the control. */
  label: string;
}

/**
 * A renderable descriptor for one elicitation form field.
 *
 * Derived from one {@link ElicitationPropertySchema} by {@link parseElicitation}.
 * It is a faithful, presentation-ready projection of the schema: it carries
 * everything the panel needs to render the control and everything
 * {@link toAcceptResponse} needs to coerce the value back, and nothing else.
 */
export interface ElicitationField {
  /** The property name — the key under which the value lands in the response. */
  key: string;
  /** The human-readable label: the schema `title`, falling back to {@link key}. */
  label: string;
  /** Which control to render and how to coerce the value. */
  kind: ElicitationFieldKind;
  /** Whether the schema lists this property in its `required` array. */
  required: boolean;
  /** The choices for a `select`/`multiselect` field; absent for other kinds. */
  options?: ElicitationOption[];
  /** The schema `description`, shown as helper text; absent when unset. */
  description?: string;
  /** The string `format` hint (`email`, `uri`, `date`, `date-time`), if any. */
  format?: StringFormat;
  /**
   * The schema `default`, in the field's editing type, when one was provided.
   *
   * Carried so {@link initialFormState} can seed the form from the schema
   * without re-reading the raw request. Absent when the schema gave no default.
   */
  default?: ElicitationFieldValue;
}

/**
 * The editable value of a single field while the form is being filled in.
 *
 * Text, number, and integer fields edit as `string` (numbers stay textual
 * until {@link toAcceptResponse} coerces them, so partial input like `"-"` is
 * representable). Booleans edit as `boolean`; multiselects as `string[]`.
 */
export type ElicitationFieldValue = string | boolean | string[];

/** The full set of editable form values, keyed by field {@link ElicitationField.key}. */
export type FormValues = Record<string, ElicitationFieldValue>;

/** Validation errors, keyed by field key. Empty when the form is valid. */
export type FormErrors = Record<string, string>;

/**
 * A parsed elicitation request: either a renderable form or a url redirect.
 *
 * The discriminant is `mode`, mirroring the ACP request's own discriminator,
 * so the panel switches on the same field whether reading the raw request or
 * this parsed view.
 */
export type ParsedElicitation =
  | {
      /** Form mode — render {@link fields} and collect input. */
      mode: "form";
      /** The agent's human-readable prompt. */
      message: string;
      /** The fields to render, in schema declaration order. */
      fields: ElicitationField[];
    }
  | {
      /** URL mode — direct the user to {@link url}. */
      mode: "url";
      /** The agent's human-readable prompt. */
      message: string;
      /** The URL to open. */
      url: string;
      /** The elicitation's id, echoed in the completion notification. */
      elicitationId: string;
    };

/**
 * `maxLength` at or above which a `string` field renders as a `textarea`.
 *
 * Single-line inputs comfortably hold short answers; an agent that allows a
 * long answer (a large `maxLength`) signals free-form prose, which a textarea
 * renders better. The threshold is a presentation heuristic only — both kinds
 * coerce to a plain `string` in the response.
 */
const TEXTAREA_MAX_LENGTH_THRESHOLD = 120;

/**
 * Parse an ACP {@link CreateElicitationRequest} into a renderable view.
 *
 * URL-mode requests parse straight to the url branch — no form fields are
 * invented. Form-mode requests map each property of the requested schema to an
 * {@link ElicitationField} via {@link toField}, preserving declaration order.
 *
 * @param request - The agent's elicitation request.
 * @returns The discriminated {@link ParsedElicitation} view.
 */
export function parseElicitation(
  request: CreateElicitationRequest,
): ParsedElicitation {
  if (request.mode === "url") {
    return {
      mode: "url",
      message: request.message,
      url: request.url,
      elicitationId: request.elicitationId,
    };
  }

  return {
    mode: "form",
    message: request.message,
    fields: schemaFields(request.requestedSchema),
  };
}

/**
 * Map every property of an {@link ElicitationSchema} to an {@link ElicitationField}.
 *
 * Iteration order follows the `properties` object's own key order, which is the
 * schema's declaration order. A property named in `required` is flagged on its
 * field. A schema with no `properties` yields an empty list.
 */
function schemaFields(schema: ElicitationSchema): ElicitationField[] {
  const properties = schema.properties ?? {};
  const required = new Set(schema.required ?? []);
  return Object.entries(properties).map(([key, property]) =>
    toField(key, property, required.has(key)),
  );
}

/**
 * Map one {@link ElicitationPropertySchema} to an {@link ElicitationField}.
 *
 * Dispatches on the schema's `type` discriminator. A `string` schema is
 * refined further by {@link stringField} (select vs textarea vs text); the
 * other types map one-to-one to their field kind.
 */
function toField(
  key: string,
  property: ElicitationPropertySchema,
  required: boolean,
): ElicitationField {
  const base = {
    key,
    label: property.title ?? key,
    required,
    ...(property.description ? { description: property.description } : {}),
  };

  switch (property.type) {
    case "string":
      return {
        ...base,
        ...stringField(property),
        ...defaultPatch(property.default, (value) => value),
      };
    case "number":
      return {
        ...base,
        kind: "number",
        ...defaultPatch(property.default, String),
      };
    case "integer":
      return {
        ...base,
        kind: "integer",
        ...defaultPatch(property.default, String),
      };
    case "boolean":
      return {
        ...base,
        kind: "boolean",
        ...defaultPatch(property.default, (value) => value),
      };
    case "array":
      return {
        ...base,
        kind: "multiselect",
        options: arrayOptions(property),
        ...defaultPatch(property.default, (value) => value),
      };
  }
}

/**
 * Build a `{ default }` patch from a schema default, or `{}` when none exists.
 *
 * Spread into the field literal, the empty patch contributes no `default` key,
 * so a field without a schema default carries no `default` property at all. The
 * `transform` maps the schema's default into the field's editing type — most
 * kinds pass it through, but numbers and integers stringify it (their controls
 * edit as text; see {@link ElicitationFieldValue}).
 *
 * @param value - The schema default for the property, if any.
 * @param transform - Maps the present default to its editing-type value.
 * @returns `{ default }` when a default was present, otherwise `{}`.
 */
function defaultPatch<T>(
  value: T | null | undefined,
  transform: (value: T) => ElicitationFieldValue,
): { default?: ElicitationFieldValue } {
  return value !== null && value !== undefined
    ? { default: transform(value) }
    : {};
}

/**
 * The kind-specific fields for a `string` property.
 *
 * A string with `enum` or `oneOf` is a single-select, so it becomes a `select`
 * carrying its options. Otherwise it is free text: a large `maxLength` (or any
 * value at or above {@link TEXTAREA_MAX_LENGTH_THRESHOLD}) renders as a
 * `textarea`, and anything else as a single-line `text`. The `format` hint
 * rides along on the free-text kinds for the panel to apply (e.g. an email
 * input).
 */
function stringField(
  property: Extract<ElicitationPropertySchema, { type: "string" }>,
): Pick<ElicitationField, "kind" | "options" | "format"> {
  const options = stringOptions(property);
  if (options !== undefined) {
    return { kind: "select", options };
  }

  const kind: ElicitationFieldKind =
    property.maxLength !== null &&
    property.maxLength !== undefined &&
    property.maxLength >= TEXTAREA_MAX_LENGTH_THRESHOLD
      ? "textarea"
      : "text";

  return property.format ? { kind, format: property.format } : { kind };
}

/**
 * Extract single-select options from a `string` property, or `undefined`.
 *
 * Titled `oneOf` options win when present (they carry human labels); otherwise
 * an `enum` becomes value-equals-label options. A string with neither is plain
 * free text, signalled by `undefined`.
 */
function stringOptions(
  property: Extract<ElicitationPropertySchema, { type: "string" }>,
): ElicitationOption[] | undefined {
  if (property.oneOf && property.oneOf.length > 0) {
    return property.oneOf.map((option) => ({
      value: option.const,
      label: option.title,
    }));
  }
  if (property.enum && property.enum.length > 0) {
    return property.enum.map((value) => ({ value, label: value }));
  }
  return undefined;
}

/**
 * Extract multi-select options from an `array` property's `items` definition.
 *
 * The items are either titled (`anyOf` of titled options) or untitled (an
 * `enum` of bare strings); the two map to options exactly as the single-select
 * `oneOf`/`enum` forms do.
 */
function arrayOptions(
  property: Extract<ElicitationPropertySchema, { type: "array" }>,
): ElicitationOption[] {
  const items = property.items;
  if ("anyOf" in items) {
    return items.anyOf.map((option) => ({
      value: option.const,
      label: option.title,
    }));
  }
  return items.enum.map((value) => ({ value, label: value }));
}

/**
 * Build the initial {@link FormValues} from a field list.
 *
 * Each field is seeded from its schema {@link ElicitationField.default} when one
 * was provided; absent a default, a field starts empty in the type its control
 * edits: `""` for text, number, and integer; `false` for boolean; `[]` for
 * multiselect.
 *
 * @param fields - The parsed fields to seed.
 * @returns A fresh {@link FormValues} map.
 */
export function initialFormState(fields: ElicitationField[]): FormValues {
  const values: FormValues = {};
  for (const field of fields) {
    values[field.key] =
      field.default !== undefined ? field.default : emptyValueFor(field.kind);
  }
  return values;
}

/** The editing-empty value for a field kind. */
function emptyValueFor(kind: ElicitationFieldKind): ElicitationFieldValue {
  switch (kind) {
    case "boolean":
      return false;
    case "multiselect":
      return [];
    default:
      return "";
  }
}

/**
 * Validate the current {@link FormValues} against the field list.
 *
 * Reports two error classes:
 *
 * - **Missing required** — a required field whose value is empty (`""`, `[]`).
 *   A boolean is never "empty": `false` is a real answer.
 * - **Type error** — a non-empty `number`/`integer` field whose value does not
 *   parse to a finite number (and, for `integer`, a whole number).
 *
 * Optional empty fields are silently valid. The returned map is keyed by field
 * key and is empty exactly when the form is valid.
 *
 * @param fields - The fields to validate.
 * @param values - The current form values.
 * @returns The {@link FormErrors} map, empty when valid.
 */
export function validateForm(
  fields: ElicitationField[],
  values: FormValues,
): FormErrors {
  const errors: FormErrors = {};
  for (const field of fields) {
    const value = values[field.key];
    if (field.required && isEmpty(value)) {
      errors[field.key] = `${field.key} is required`;
      continue;
    }
    const typeError = numericError(field, value);
    if (typeError !== undefined) {
      errors[field.key] = typeError;
    }
  }
  return errors;
}

/**
 * Whether a form value counts as empty for required-field validation.
 *
 * An empty string and an empty array are empty; a boolean is never empty
 * (`false` is a deliberate answer). `undefined` (an unset field) is empty too.
 */
function isEmpty(value: ElicitationFieldValue | undefined): boolean {
  if (value === undefined) {
    return true;
  }
  if (typeof value === "string") {
    return value.trim() === "";
  }
  if (Array.isArray(value)) {
    return value.length === 0;
  }
  return false;
}

/**
 * The type error for a `number`/`integer` field, or `undefined` when valid.
 *
 * A blank value is left to required-field validation, so it produces no type
 * error here. A non-blank value must parse to a finite number; an `integer`
 * field additionally requires a whole number.
 */
function numericError(
  field: ElicitationField,
  value: ElicitationFieldValue | undefined,
): string | undefined {
  if (field.kind !== "number" && field.kind !== "integer") {
    return undefined;
  }
  if (typeof value !== "string" || value.trim() === "") {
    return undefined;
  }
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) {
    return `${field.key} must be a number`;
  }
  if (field.kind === "integer" && !Number.isInteger(parsed)) {
    return `${field.key} must be an integer`;
  }
  return undefined;
}

/**
 * Coerce the form values into an `accept` {@link CreateElicitationResponse}.
 *
 * Each field's value is coerced to the JSON type its schema requested — a
 * number for `number`/`integer`, a real boolean for `boolean`, a `string[]`
 * for `multiselect`, a `string` for the rest — so the agent receives a typed
 * answer (a number, not `"42"`; `true`, not `"true"`).
 *
 * Optional fields left empty are omitted from the `content` map entirely,
 * matching the elicitation contract that an absent value means "not provided".
 * A boolean is always included: `false` is a real answer, not an omission.
 *
 * This does not validate; callers run {@link validateForm} first and only build
 * an accept response once it is clean.
 *
 * @param fields - The fields the form was built from.
 * @param values - The current form values to coerce.
 * @returns An `accept` response carrying the coerced `content` map.
 */
export function toAcceptResponse(
  fields: ElicitationField[],
  values: FormValues,
): CreateElicitationResponse {
  const content: Record<string, ElicitationContentValue> = {};
  for (const field of fields) {
    const coerced = coerceValue(field, values[field.key]);
    if (coerced !== undefined) {
      content[field.key] = coerced;
    }
  }
  return { action: "accept", content };
}

/**
 * Coerce one field's value to its {@link ElicitationContentValue}, or omit it.
 *
 * Returns `undefined` to signal omission: an empty optional value (empty
 * string, empty array) is dropped from the response. A boolean is never
 * dropped. Numbers parse from their textual edit value; a blank or
 * unparseable number is omitted rather than sent as `NaN`.
 */
function coerceValue(
  field: ElicitationField,
  value: ElicitationFieldValue | undefined,
): ElicitationContentValue | undefined {
  switch (field.kind) {
    case "boolean":
      return value === true;
    case "multiselect": {
      const items = Array.isArray(value) ? value : [];
      return items.length > 0 ? items : undefined;
    }
    case "number":
    case "integer": {
      if (typeof value !== "string" || value.trim() === "") {
        return undefined;
      }
      const parsed = Number(value);
      return Number.isFinite(parsed) ? parsed : undefined;
    }
    default: {
      const text = typeof value === "string" ? value : "";
      return text.trim() === "" ? undefined : text;
    }
  }
}

/**
 * Build the `decline` {@link CreateElicitationResponse}.
 *
 * Decline means the user actively refused to provide the requested input.
 *
 * @returns The decline response.
 */
export function declineResponse(): CreateElicitationResponse {
  return { action: "decline" };
}

/**
 * Build the `cancel` {@link CreateElicitationResponse}.
 *
 * Cancel means the user dismissed the request without deciding (e.g. closed
 * the dialog).
 *
 * @returns The cancel response.
 */
export function cancelResponse(): CreateElicitationResponse {
  return { action: "cancel" };
}
