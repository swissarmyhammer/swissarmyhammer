/**
 * Characterization tests for `EntityIcon` — the metadata-driven entity glyph.
 *
 * `EntityIcon` reads an entity type's `icon` from its schema and renders the
 * matching lucide component, falling back to `LayoutGrid` when the icon is
 * missing or unresolvable. This file pins that contract so the unification of
 * the lookup onto `@/lib/icon-name::iconByName` (card 77zwtq2) preserves
 * behavior exactly:
 *
 *   - a declared kebab-case icon name resolves to its lucide component;
 *   - an unknown icon name renders `LayoutGrid`;
 *   - a schema with no `entity.icon` renders `LayoutGrid`.
 *
 * Assertions match against lucide's stable `lucide-<name>` class fingerprint
 * so a stylesheet rename without a real component swap can't pass falsely.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, waitFor } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Tauri API mock — must come before component imports.
// ---------------------------------------------------------------------------

const mockInvoke = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...a: unknown[]) => mockInvoke(...(a as [string, unknown?])),
}));

// ---------------------------------------------------------------------------
// Imports come after mocks.
// ---------------------------------------------------------------------------

import { EntityIcon } from "@/components/entity-icon";
import { SchemaProvider } from "@/lib/schema-context";
import type { EntitySchema } from "@/types/kanban";

/** Build a minimal EntitySchema carrying only the entity-level `icon`. */
function schema(name: string, icon?: string): EntitySchema {
  return { entity: { name, icon, fields: [] }, fields: [] };
}

const SCHEMAS: Record<string, EntitySchema> = {
  task: schema("task", "table"),
  bogus: schema("bogus", "definitely-not-a-real-lucide-icon-name"),
  iconless: schema("iconless"),
};

/** Default invoke responses for the schema-load IPCs SchemaProvider fires. */
async function defaultInvokeImpl(
  cmd: string,
  args?: unknown,
): Promise<unknown> {
  if (cmd === "list_entity_types") return Object.keys(SCHEMAS);
  if (cmd === "get_entity_schema") {
    const entityType = (args as { entityType?: string })?.entityType ?? "";
    return SCHEMAS[entityType];
  }
  return undefined;
}

/** Render an EntityIcon inside a real SchemaProvider and return the container. */
function renderEntityIcon(entityType: string) {
  return render(
    <SchemaProvider>
      <EntityIcon entityType={entityType} className="h-4 w-4" />
    </SchemaProvider>,
  );
}

describe("EntityIcon — metadata-driven lucide lookup", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("resolves a declared icon name to its lucide component", async () => {
    const { container, unmount } = renderEntityIcon("task");

    await waitFor(() => {
      const svg = container.querySelector("svg");
      expect(svg, "entity with icon=table must render an svg").not.toBeNull();
      expect(
        svg!.getAttribute("class"),
        "the rendered svg must carry the lucide-table class fingerprint",
      ).toContain("lucide-table");
    });

    unmount();
  });

  it("falls back to LayoutGrid for an unresolvable icon name", async () => {
    const { container, unmount } = renderEntityIcon("bogus");

    await waitFor(() => {
      const svg = container.querySelector("svg");
      expect(svg).not.toBeNull();
      expect(
        svg!.getAttribute("class"),
        "unknown icon name must render the LayoutGrid fallback",
      ).toContain("lucide-layout-grid");
    });

    unmount();
  });

  it("falls back to LayoutGrid when the schema declares no icon", async () => {
    const { container, unmount } = renderEntityIcon("iconless");

    // The fallback renders immediately (no schema icon to resolve), but assert
    // after a settle so a future schema-driven icon couldn't slip in unnoticed.
    await waitFor(() => {
      const svg = container.querySelector("svg");
      expect(svg).not.toBeNull();
      expect(svg!.getAttribute("class")).toContain("lucide-layout-grid");
    });

    unmount();
  });
});
