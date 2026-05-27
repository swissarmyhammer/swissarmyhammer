/**
 * Unit tests for the pure elicitation form model (`ai/elicitation.ts`).
 *
 * The elicitation module is the React-free seam between an ACP
 * `CreateElicitationRequest` and the form the AI panel renders. These tests
 * pin every contract the panel and the ACP client depend on:
 *
 *   - every `ElicitationPropertySchema` variant maps to the right field kind;
 *   - URL-mode requests parse to the url branch without inventing fields;
 *   - `initialFormState` seeds each field with the schema's default;
 *   - `validateForm` flags empty required fields and passes when satisfied;
 *   - `toAcceptResponse` coerces values to the requested JSON types and omits
 *     optional empties;
 *   - the decline/cancel helpers return the documented responses;
 *   - the single-string `{answer: string}` schema the SAH `ask question` tool
 *     emits round-trips through the model.
 */
import { describe, it, expect } from "vitest";
import type {
  CreateElicitationRequest,
  ElicitationPropertySchema,
} from "@agentclientprotocol/sdk";
import {
  parseElicitation,
  initialFormState,
  validateForm,
  toAcceptResponse,
  declineResponse,
  cancelResponse,
  type ElicitationField,
  type ParsedElicitation,
} from "./elicitation";

/** Build a form-mode request from a single property and its required-ness. */
function formRequest(
  key: string,
  property: ElicitationPropertySchema,
  options: { required?: boolean; message?: string } = {},
): CreateElicitationRequest {
  return {
    mode: "form",
    sessionId: "sess-1",
    message: options.message ?? "Please fill this in",
    requestedSchema: {
      type: "object",
      properties: { [key]: property },
      required: options.required ? [key] : [],
    },
  };
}

/** Parse a form request and assert it came back as the form branch. */
function parseForm(request: CreateElicitationRequest): {
  message: string;
  fields: ElicitationField[];
} {
  const parsed = parseElicitation(request);
  if (parsed.mode !== "form") {
    throw new Error(`expected form mode, got ${parsed.mode}`);
  }
  return parsed;
}

describe("parseElicitation — field kind mapping", () => {
  it("maps a plain string to a text field", () => {
    const { fields } = parseForm(
      formRequest("name", { type: "string", title: "Your name" }),
    );
    expect(fields).toEqual([
      { key: "name", label: "Your name", kind: "text", required: false },
    ]);
  });

  it("falls back to the property key when no title is given", () => {
    const { fields } = parseForm(formRequest("city", { type: "string" }));
    expect(fields[0].label).toBe("city");
  });

  it("maps a long-form string (maxLength over the threshold) to a textarea", () => {
    const { fields } = parseForm(
      formRequest("bio", { type: "string", maxLength: 500 }),
    );
    expect(fields[0].kind).toBe("textarea");
  });

  it("maps a string with an enum to a single-select field", () => {
    const { fields } = parseForm(
      formRequest("color", {
        type: "string",
        enum: ["red", "green", "blue"],
      }),
    );
    expect(fields[0].kind).toBe("select");
    expect(fields[0].options).toEqual([
      { value: "red", label: "red" },
      { value: "green", label: "green" },
      { value: "blue", label: "blue" },
    ]);
  });

  it("maps a string with titled oneOf options to a single-select field", () => {
    const { fields } = parseForm(
      formRequest("size", {
        type: "string",
        oneOf: [
          { const: "s", title: "Small" },
          { const: "l", title: "Large" },
        ],
      }),
    );
    expect(fields[0].kind).toBe("select");
    expect(fields[0].options).toEqual([
      { value: "s", label: "Small" },
      { value: "l", label: "Large" },
    ]);
  });

  it("carries a string format through to the field", () => {
    const { fields } = parseForm(
      formRequest("email", { type: "string", format: "email" }),
    );
    expect(fields[0].format).toBe("email");
  });

  it("maps a number property to a number field", () => {
    const { fields } = parseForm(formRequest("ratio", { type: "number" }));
    expect(fields[0].kind).toBe("number");
  });

  it("maps an integer property to an integer field", () => {
    const { fields } = parseForm(formRequest("count", { type: "integer" }));
    expect(fields[0].kind).toBe("integer");
  });

  it("maps a boolean property to a boolean field", () => {
    const { fields } = parseForm(formRequest("agree", { type: "boolean" }));
    expect(fields[0].kind).toBe("boolean");
  });

  it("maps an array property with untitled items to a multiselect field", () => {
    const { fields } = parseForm(
      formRequest("tags", {
        type: "array",
        items: { type: "string", enum: ["a", "b", "c"] },
      }),
    );
    expect(fields[0].kind).toBe("multiselect");
    expect(fields[0].options).toEqual([
      { value: "a", label: "a" },
      { value: "b", label: "b" },
      { value: "c", label: "c" },
    ]);
  });

  it("maps an array property with titled anyOf items to a multiselect field", () => {
    const { fields } = parseForm(
      formRequest("perms", {
        type: "array",
        items: {
          anyOf: [
            { const: "read", title: "Read" },
            { const: "write", title: "Write" },
          ],
        },
      }),
    );
    expect(fields[0].kind).toBe("multiselect");
    expect(fields[0].options).toEqual([
      { value: "read", label: "Read" },
      { value: "write", label: "Write" },
    ]);
  });

  it("carries the property description and required flag onto the field", () => {
    const { fields } = parseForm(
      formRequest(
        "name",
        { type: "string", description: "Full legal name" },
        { required: true },
      ),
    );
    expect(fields[0].description).toBe("Full legal name");
    expect(fields[0].required).toBe(true);
  });

  it("carries the message through", () => {
    const { message } = parseForm(
      formRequest("name", { type: "string" }, { message: "Who are you?" }),
    );
    expect(message).toBe("Who are you?");
  });

  it("yields no fields for a schema with no properties", () => {
    const { fields } = parseForm({
      mode: "form",
      sessionId: "sess-1",
      message: "Nothing to fill",
      requestedSchema: { type: "object" },
    });
    expect(fields).toEqual([]);
  });
});

describe("parseElicitation — url mode", () => {
  it("parses a url request to the url branch without inventing fields", () => {
    const request: CreateElicitationRequest = {
      mode: "url",
      sessionId: "sess-1",
      message: "Authorize in your browser",
      url: "https://example.com/authorize",
      elicitationId: "elic-42",
    };
    const parsed: ParsedElicitation = parseElicitation(request);
    expect(parsed).toEqual({
      mode: "url",
      message: "Authorize in your browser",
      url: "https://example.com/authorize",
      elicitationId: "elic-42",
    });
  });
});

describe("initialFormState", () => {
  it("seeds each field with its schema default", () => {
    const { fields } = parseForm({
      mode: "form",
      sessionId: "s",
      message: "m",
      requestedSchema: {
        type: "object",
        properties: {
          name: { type: "string", default: "Ada" },
          count: { type: "integer", default: 3 },
          agree: { type: "boolean", default: true },
          tags: {
            type: "array",
            items: { type: "string", enum: ["a", "b"] },
            default: ["a"],
          },
        },
      },
    });
    // Number/integer fields edit as strings (so partial input is
    // representable), so a numeric default is seeded in that textual form;
    // `toAcceptResponse` coerces it back to a real number on submit.
    expect(initialFormState(fields)).toEqual({
      name: "Ada",
      count: "3",
      agree: true,
      tags: ["a"],
    });
  });

  it("uses empty/false defaults when the schema gives none", () => {
    const { fields } = parseForm({
      mode: "form",
      sessionId: "s",
      message: "m",
      requestedSchema: {
        type: "object",
        properties: {
          name: { type: "string" },
          count: { type: "integer" },
          ratio: { type: "number" },
          agree: { type: "boolean" },
          tags: { type: "array", items: { type: "string", enum: ["a"] } },
        },
      },
    });
    expect(initialFormState(fields)).toEqual({
      name: "",
      count: "",
      ratio: "",
      agree: false,
      tags: [],
    });
  });
});

describe("validateForm", () => {
  it("flags an empty required text field", () => {
    const { fields } = parseForm(
      formRequest("name", { type: "string" }, { required: true }),
    );
    const errors = validateForm(fields, { name: "" });
    expect(errors).toEqual({ name: "name is required" });
  });

  it("flags an empty required multiselect", () => {
    const { fields } = parseForm(
      formRequest(
        "tags",
        { type: "array", items: { type: "string", enum: ["a", "b"] } },
        { required: true },
      ),
    );
    const errors = validateForm(fields, { tags: [] });
    expect(errors).toEqual({ tags: "tags is required" });
  });

  it("passes when a required field is satisfied", () => {
    const { fields } = parseForm(
      formRequest("name", { type: "string" }, { required: true }),
    );
    expect(validateForm(fields, { name: "Ada" })).toEqual({});
  });

  it("does not flag an empty optional field", () => {
    const { fields } = parseForm(formRequest("name", { type: "string" }));
    expect(validateForm(fields, { name: "" })).toEqual({});
  });

  it("flags a number field whose value is not a finite number", () => {
    const { fields } = parseForm(
      formRequest("count", { type: "number" }, { required: true }),
    );
    const errors = validateForm(fields, { count: "not a number" });
    expect(errors).toEqual({ count: "count must be a number" });
  });

  it("flags an integer field whose value is not a whole number", () => {
    const { fields } = parseForm(formRequest("count", { type: "integer" }));
    const errors = validateForm(fields, { count: "3.5" });
    expect(errors).toEqual({ count: "count must be an integer" });
  });

  it("accepts a numeric string for a number field", () => {
    const { fields } = parseForm(formRequest("ratio", { type: "number" }));
    expect(validateForm(fields, { ratio: "1.5" })).toEqual({});
  });
});

describe("toAcceptResponse — type coercion", () => {
  it("returns a string for a text field", () => {
    const { fields } = parseForm(formRequest("name", { type: "string" }));
    const response = toAcceptResponse(fields, { name: "Ada" });
    expect(response).toEqual({ action: "accept", content: { name: "Ada" } });
  });

  it("coerces a numeric string to a number for a number field", () => {
    const { fields } = parseForm(formRequest("ratio", { type: "number" }));
    const response = toAcceptResponse(fields, { ratio: "1.5" });
    expect(response.action).toBe("accept");
    if (response.action !== "accept") return;
    expect(response.content).toEqual({ ratio: 1.5 });
    expect(typeof response.content?.ratio).toBe("number");
  });

  it("coerces a numeric string to an integer for an integer field", () => {
    const { fields } = parseForm(formRequest("count", { type: "integer" }));
    const response = toAcceptResponse(fields, { count: "7" });
    if (response.action !== "accept") throw new Error("expected accept");
    expect(response.content).toEqual({ count: 7 });
    expect(typeof response.content?.count).toBe("number");
  });

  it("emits a real boolean for a boolean field, not the string 'true'", () => {
    const { fields } = parseForm(formRequest("agree", { type: "boolean" }));
    const response = toAcceptResponse(fields, { agree: true });
    if (response.action !== "accept") throw new Error("expected accept");
    expect(response.content).toEqual({ agree: true });
    expect(response.content?.agree).toBe(true);
  });

  it("emits a string array for a multiselect field", () => {
    const { fields } = parseForm(
      formRequest("tags", {
        type: "array",
        items: { type: "string", enum: ["a", "b", "c"] },
      }),
    );
    const response = toAcceptResponse(fields, { tags: ["a", "c"] });
    if (response.action !== "accept") throw new Error("expected accept");
    expect(response.content).toEqual({ tags: ["a", "c"] });
    expect(Array.isArray(response.content?.tags)).toBe(true);
  });

  it("emits the selected option string for a select field", () => {
    const { fields } = parseForm(
      formRequest("color", { type: "string", enum: ["red", "blue"] }),
    );
    const response = toAcceptResponse(fields, { color: "blue" });
    if (response.action !== "accept") throw new Error("expected accept");
    expect(response.content).toEqual({ color: "blue" });
  });

  it("omits optional empty fields from the content map", () => {
    const { fields } = parseForm({
      mode: "form",
      sessionId: "s",
      message: "m",
      requestedSchema: {
        type: "object",
        properties: {
          name: { type: "string" },
          nickname: { type: "string" },
          tags: { type: "array", items: { type: "string", enum: ["a"] } },
        },
        required: ["name"],
      },
    });
    const response = toAcceptResponse(fields, {
      name: "Ada",
      nickname: "",
      tags: [],
    });
    if (response.action !== "accept") throw new Error("expected accept");
    expect(response.content).toEqual({ name: "Ada" });
  });

  it("omits an optional number left blank", () => {
    const { fields } = parseForm(formRequest("ratio", { type: "number" }));
    const response = toAcceptResponse(fields, { ratio: "" });
    if (response.action !== "accept") throw new Error("expected accept");
    expect(response.content).toEqual({});
  });

  it("keeps a boolean false in the content map (false is a real answer)", () => {
    const { fields } = parseForm(formRequest("agree", { type: "boolean" }));
    const response = toAcceptResponse(fields, { agree: false });
    if (response.action !== "accept") throw new Error("expected accept");
    expect(response.content).toEqual({ agree: false });
  });
});

describe("decline / cancel helpers", () => {
  it("declineResponse returns the decline action", () => {
    expect(declineResponse()).toEqual({ action: "decline" });
  });

  it("cancelResponse returns the cancel action", () => {
    expect(cancelResponse()).toEqual({ action: "cancel" });
  });
});

describe("SAH `ask question` single-string schema round-trip", () => {
  /** The schema the SAH `ask question` MCP tool emits: `{answer: string}`. */
  const askQuestionRequest: CreateElicitationRequest = {
    mode: "form",
    sessionId: "sess-1",
    message: "What is your preferred deployment target?",
    requestedSchema: {
      type: "object",
      properties: {
        answer: { type: "string", title: "Answer" },
      },
      required: ["answer"],
    },
  };

  it("parses to a single required text field", () => {
    const { fields } = parseForm(askQuestionRequest);
    expect(fields).toEqual([
      { key: "answer", label: "Answer", kind: "text", required: true },
    ]);
  });

  it("round-trips a typed answer back to {answer: '...'}", () => {
    const { fields } = parseForm(askQuestionRequest);
    const response = toAcceptResponse(fields, { answer: "Kubernetes" });
    expect(response).toEqual({
      action: "accept",
      content: { answer: "Kubernetes" },
    });
  });

  it("flags a blank answer as a missing required field", () => {
    const { fields } = parseForm(askQuestionRequest);
    expect(validateForm(fields, { answer: "" })).toEqual({
      answer: "answer is required",
    });
  });
});
