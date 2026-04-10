/**
 * Tests for SchemaContext's `mentionableTypes` derivation.
 *
 * The schema loader has a hard invariant: an entity is only included in
 * `mentionableTypes` when BOTH `mention_prefix` AND `mention_display_field`
 * are set on its EntityDef. Missing either one silently drops the entity
 * from autocomplete. These tests lock in that invariant so a regression
 * in the loader cannot silently break `$project` / `#tag` / `@actor`
 * mention autocomplete.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";

// Mock Tauri APIs before importing any modules that use them.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const mockInvoke = vi.fn((..._args: any[]): Promise<any> => Promise.resolve(null));
vi.mock("@tauri-apps/api/core", () => ({
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  invoke: (...args: any[]) => mockInvoke(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main" }),
}));

vi.mock("@tauri-apps/plugin-log", () => ({
  error: vi.fn(),
  warn: vi.fn(),
  info: vi.fn(),
  debug: vi.fn(),
  trace: vi.fn(),
  attachConsole: vi.fn(() => Promise.resolve()),
}));

import { SchemaProvider, useSchema } from "./schema-context";
import type { EntityDef, EntitySchema } from "@/types/kanban";

/** Build a minimal EntitySchema with an EntityDef override for tests. */
function makeSchema(entity: Partial<EntityDef> & { name: string }): EntitySchema {
  return {
    entity: {
      fields: [],
      ...entity,
    },
    fields: [],
  };
}

/**
 * Configure the mocked `invoke` so `list_entity_types` returns the keys of
 * `schemas` and `get_entity_schema` returns the matching entry.
 */
function mockSchemaLoader(schemas: Record<string, EntitySchema>) {
  mockInvoke.mockImplementation((command: string, args?: unknown) => {
    if (command === "list_entity_types") {
      return Promise.resolve(Object.keys(schemas));
    }
    if (command === "get_entity_schema") {
      const entityType = (args as { entityType: string }).entityType;
      const schema = schemas[entityType];
      if (!schema) return Promise.reject(new Error("unknown"));
      return Promise.resolve(schema);
    }
    return Promise.resolve(null);
  });
}

function wrapper({ children }: { children: ReactNode }) {
  return <SchemaProvider>{children}</SchemaProvider>;
}

describe("SchemaContext.mentionableTypes", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("includes an entity with both mention_prefix and mention_display_field", async () => {
    mockSchemaLoader({
      project: makeSchema({
        name: "project",
        mention_prefix: "$",
        mention_display_field: "name",
      }),
    });

    const { result } = renderHook(() => useSchema(), { wrapper });

    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.mentionableTypes).toContainEqual({
      entityType: "project",
      prefix: "$",
      displayField: "name",
    });
  });

  it("includes multiple mentionable entities with different prefixes", async () => {
    mockSchemaLoader({
      tag: makeSchema({
        name: "tag",
        mention_prefix: "#",
        mention_display_field: "tag_name",
      }),
      actor: makeSchema({
        name: "actor",
        mention_prefix: "@",
        mention_display_field: "name",
      }),
      project: makeSchema({
        name: "project",
        mention_prefix: "$",
        mention_display_field: "name",
      }),
    });

    const { result } = renderHook(() => useSchema(), { wrapper });

    await waitFor(() => expect(result.current.loading).toBe(false));

    const prefixes = result.current.mentionableTypes
      .map((mt) => mt.prefix)
      .sort();
    expect(prefixes).toEqual(["#", "$", "@"]);
  });

  it("excludes an entity missing mention_prefix", async () => {
    mockSchemaLoader({
      project: makeSchema({
        name: "project",
        // mention_prefix intentionally omitted
        mention_display_field: "name",
      }),
    });

    const { result } = renderHook(() => useSchema(), { wrapper });

    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(
      result.current.mentionableTypes.find((mt) => mt.entityType === "project"),
    ).toBeUndefined();
  });

  it("excludes an entity missing mention_display_field", async () => {
    mockSchemaLoader({
      project: makeSchema({
        name: "project",
        mention_prefix: "$",
        // mention_display_field intentionally omitted
      }),
    });

    const { result } = renderHook(() => useSchema(), { wrapper });

    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(
      result.current.mentionableTypes.find((mt) => mt.entityType === "project"),
    ).toBeUndefined();
  });

  it("excludes an entity missing BOTH mention fields", async () => {
    mockSchemaLoader({
      board: makeSchema({
        name: "board",
        // neither mention_prefix nor mention_display_field set
      }),
    });

    const { result } = renderHook(() => useSchema(), { wrapper });

    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.mentionableTypes).toEqual([]);
  });
});
