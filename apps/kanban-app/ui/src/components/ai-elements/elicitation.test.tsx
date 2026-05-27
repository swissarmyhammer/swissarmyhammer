/**
 * Behavioral tests for the {@link ElicitationFields} AI element.
 *
 * `ElicitationFields` is a *controlled* presentational component: it turns the
 * pure {@link ElicitationField} descriptors (from `@/ai/elicitation`) into
 * shadcn controls, holds no internal form state, and reports every edit back
 * through `onChange(key, value)` in the field's natural editing type. These
 * tests pin three contracts for each field kind:
 *
 * 1. the descriptor renders the *correct control* (queried by role/slot),
 * 2. editing it fires `onChange(key, value)` with the *natural-type payload*
 *    documented on {@link ElicitationFieldValue}, and
 * 3. a provided `errors` entry renders next to its field.
 *
 * Tests run in real Chromium (the `browser` vitest project), so Radix's
 * portal-rendered Select content is reachable via `screen` role queries and
 * `userEvent`, mirroring the model-select tests in `ai-prompt-composer.test`.
 */

import { describe, it, expect, vi } from "vitest";
import { screen, within } from "@testing-library/react";
import { userEvent } from "vitest/browser";
import { renderInAct, clickInAct } from "@/test/act-render";
import type { ElicitationField, FormValues } from "@/ai/elicitation";
import { ElicitationFields } from "./elicitation";

/**
 * Render `ElicitationFields` for a single field with the given current value.
 *
 * Returns the render result plus the `onChange` spy so each test can assert
 * the exact `(key, value)` payload the control emits.
 */
async function renderField(
  field: ElicitationField,
  value: FormValues[string],
  errors?: Record<string, string>,
) {
  const onChange = vi.fn();
  const values: FormValues = { [field.key]: value };
  const result = await renderInAct(
    <ElicitationFields
      fields={[field]}
      values={values}
      onChange={onChange}
      errors={errors}
    />,
  );
  return { ...result, onChange };
}

/**
 * The `value` argument of the most recent `onChange(key, value)` call.
 *
 * Uses index access rather than `Array.prototype.at` so the type-check stays
 * within the project's ES2021 target lib.
 */
function lastValue(onChange: ReturnType<typeof vi.fn>): unknown {
  const { calls } = onChange.mock;
  return calls[calls.length - 1]?.[1];
}

describe("ElicitationFields: text", () => {
  const field: ElicitationField = {
    key: "name",
    label: "Name",
    kind: "text",
    required: true,
  };

  it("renders a labeled text input with a required marker", async () => {
    const { container } = await renderField(field, "");

    const input = container.querySelector(
      "input[data-slot='input']",
    ) as HTMLInputElement | null;
    expect(input).not.toBeNull();
    expect(input?.type).toBe("text");
    expect(container.textContent).toContain("Name");
    // Required fields carry a visible marker.
    expect(container.textContent).toContain("*");
  });

  it("typing emits onChange(key, string)", async () => {
    const { container, onChange } = await renderField(field, "");
    const input = container.querySelector(
      "input[data-slot='input']",
    ) as HTMLInputElement;

    await userEvent.type(input, "A");

    expect(onChange).toHaveBeenCalledWith("name", "A");
    expect(typeof lastValue(onChange)).toBe("string");
  });
});

describe("ElicitationFields: textarea", () => {
  const field: ElicitationField = {
    key: "bio",
    label: "Bio",
    kind: "textarea",
    required: false,
  };

  it("renders a textarea control", async () => {
    const { container } = await renderField(field, "");
    expect(
      container.querySelector("textarea[data-slot='textarea']"),
    ).not.toBeNull();
  });

  it("typing emits onChange(key, string)", async () => {
    const { container, onChange } = await renderField(field, "");
    const textarea = container.querySelector(
      "textarea[data-slot='textarea']",
    ) as HTMLTextAreaElement;

    await userEvent.type(textarea, "x");

    expect(onChange).toHaveBeenCalledWith("bio", "x");
    expect(typeof lastValue(onChange)).toBe("string");
  });
});

describe("ElicitationFields: select", () => {
  const field: ElicitationField = {
    key: "color",
    label: "Color",
    kind: "select",
    required: false,
    options: [
      { value: "red", label: "Red" },
      { value: "green", label: "Green" },
    ],
  };

  it("renders a shadcn Select trigger", async () => {
    await renderField(field, "");
    expect(screen.getByRole("combobox", { name: /color/i })).toBeTruthy();
  });

  it("choosing an option emits onChange(key, value)", async () => {
    const { onChange } = await renderField(field, "");

    await clickInAct(screen.getByRole("combobox", { name: /color/i }));
    const listbox = await screen.findByRole("listbox");
    await clickInAct(within(listbox).getByRole("option", { name: /green/i }));

    expect(onChange).toHaveBeenCalledWith("color", "green");
    expect(typeof lastValue(onChange)).toBe("string");
  });
});

describe("ElicitationFields: boolean", () => {
  const field: ElicitationField = {
    key: "agree",
    label: "Agree",
    kind: "boolean",
    required: false,
  };

  it("renders a checkbox control", async () => {
    await renderField(field, false);
    expect(screen.getByRole("checkbox", { name: /agree/i })).toBeTruthy();
  });

  it("toggling emits onChange(key, boolean)", async () => {
    const { onChange } = await renderField(field, false);

    await clickInAct(screen.getByRole("checkbox", { name: /agree/i }));

    expect(onChange).toHaveBeenCalledWith("agree", true);
    expect(typeof lastValue(onChange)).toBe("boolean");
  });
});

describe("ElicitationFields: number", () => {
  const field: ElicitationField = {
    key: "amount",
    label: "Amount",
    kind: "number",
    required: false,
  };

  it("renders a numeric input", async () => {
    const { container } = await renderField(field, "");
    const input = container.querySelector(
      "input[data-slot='input']",
    ) as HTMLInputElement | null;
    expect(input?.type).toBe("number");
  });

  it("typing emits onChange(key, string) preserving the textual edit value", async () => {
    const { container, onChange } = await renderField(field, "");
    const input = container.querySelector(
      "input[data-slot='input']",
    ) as HTMLInputElement;

    await userEvent.type(input, "4");

    // Numbers edit as text (see ElicitationFieldValue) — coercion happens in
    // toAcceptResponse, not here.
    expect(onChange).toHaveBeenCalledWith("amount", "4");
    expect(typeof lastValue(onChange)).toBe("string");
  });
});

describe("ElicitationFields: integer", () => {
  const field: ElicitationField = {
    key: "count",
    label: "Count",
    kind: "integer",
    required: false,
  };

  it("renders a numeric input with an integer step", async () => {
    const { container } = await renderField(field, "");
    const input = container.querySelector(
      "input[data-slot='input']",
    ) as HTMLInputElement | null;
    expect(input?.type).toBe("number");
    expect(input?.step).toBe("1");
  });

  it("typing emits onChange(key, string)", async () => {
    const { container, onChange } = await renderField(field, "");
    const input = container.querySelector(
      "input[data-slot='input']",
    ) as HTMLInputElement;

    await userEvent.type(input, "7");

    expect(onChange).toHaveBeenCalledWith("count", "7");
    expect(typeof lastValue(onChange)).toBe("string");
  });
});

describe("ElicitationFields: multiselect", () => {
  const field: ElicitationField = {
    key: "tags",
    label: "Tags",
    kind: "multiselect",
    required: false,
    options: [
      { value: "a", label: "Alpha" },
      { value: "b", label: "Beta" },
    ],
  };

  it("renders a checkbox per option", async () => {
    await renderField(field, []);
    expect(screen.getByRole("checkbox", { name: /alpha/i })).toBeTruthy();
    expect(screen.getByRole("checkbox", { name: /beta/i })).toBeTruthy();
  });

  it("checking an option adds it and emits onChange(key, string[])", async () => {
    const { onChange } = await renderField(field, []);

    await clickInAct(screen.getByRole("checkbox", { name: /beta/i }));

    expect(onChange).toHaveBeenCalledWith("tags", ["b"]);
    expect(Array.isArray(lastValue(onChange))).toBe(true);
  });

  it("unchecking an already-selected option removes it", async () => {
    const { onChange } = await renderField(field, ["a", "b"]);

    await clickInAct(screen.getByRole("checkbox", { name: /alpha/i }));

    expect(onChange).toHaveBeenCalledWith("tags", ["b"]);
  });
});

describe("ElicitationFields: errors", () => {
  it("renders a provided error next to its field", async () => {
    const field: ElicitationField = {
      key: "email",
      label: "Email",
      kind: "text",
      required: true,
    };

    const { container } = await renderField(field, "", {
      email: "email is required",
    });

    expect(container.textContent).toContain("email is required");
  });

  it("renders helper description text when present", async () => {
    const field: ElicitationField = {
      key: "email",
      label: "Email",
      kind: "text",
      required: false,
      description: "Your work address",
    };

    const { container } = await renderField(field, "");
    expect(container.textContent).toContain("Your work address");
  });
});
