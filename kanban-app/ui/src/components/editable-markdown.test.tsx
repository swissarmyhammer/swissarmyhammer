import { describe, it, expect, vi, beforeEach } from "vitest";
import { render } from "@testing-library/react";
import type { MentionableType } from "@/lib/schema-context";
import type { Entity } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Mock data — tag entity with name "bug"
// ---------------------------------------------------------------------------

const mockTag: Entity = {
  id: "tag-bug",
  entity_type: "tag",
  moniker: "tag:tag-bug",
  fields: { tag_name: "bug", color: "ff0000" },
};

const MENTIONABLE_TYPES: MentionableType[] = [
  { entityType: "tag", prefix: "#", displayField: "tag_name" },
];

// ---------------------------------------------------------------------------
// Mocks — Tauri, schema, entity store, entity-commands
// ---------------------------------------------------------------------------

const mockGetEntities = vi.fn((_type: string) => [mockTag]);

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve("ok")),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

vi.mock("@/lib/schema-context", () => ({
  useSchema: () => ({
    getSchema: () => undefined,
    getFieldDef: () => undefined,
    getEntityCommands: () => [],
    mentionableTypes: MENTIONABLE_TYPES,
    loading: false,
  }),
  useSchemaOptional: () => ({
    getSchema: () => undefined,
    getFieldDef: () => undefined,
    getEntityCommands: () => [],
  }),
}));

vi.mock("@/lib/entity-store-context", () => ({
  useEntityStore: () => ({ getEntities: mockGetEntities, getEntity: vi.fn() }),
}));

// ---------------------------------------------------------------------------

import { MarkdownDisplay } from "@/components/fields/displays/markdown-display";
import { EntityFocusProvider } from "@/lib/entity-focus-context";

import { TooltipProvider } from "@/components/ui/tooltip";
import type { DisplayProps } from "@/components/fields/displays/text-display";

function makeProps(
  value: unknown,
  mode: "compact" | "full" = "full",
  onCommit?: (value: unknown) => void,
) {
  return {
    field: {
      id: "f1",
      name: "body",
      type: { kind: "text" },
    } as DisplayProps["field"],
    value,
    entity: { entity_type: "task", id: "t1", moniker: "task:t1", fields: {} },
    mode,
    onCommit,
  };
}

/** Render with all required context providers. */
function renderMarkdown(value: string) {
  return render(
    <TooltipProvider>
      <EntityFocusProvider>
        <MarkdownDisplay {...makeProps(value, "full")} />
      </EntityFocusProvider>
    </TooltipProvider>,
  );
}

describe("multiline editing with mention types", () => {
  beforeEach(() => {
    mockGetEntities.mockReturnValue([mockTag]);
  });

  it("renders tag pills in display mode with mentions loaded", () => {
    const { container } = renderMarkdown("Fix the #bug in login");

    // The remark-mentions plugin should transform #bug into a custom element
    // rendered by MentionPill. MentionPill renders inside a FocusScope with
    // a data-moniker attribute.
    const pill = container.querySelector("[data-moniker='tag:tag-bug']");
    expect(pill).not.toBeNull();

    // The pill text should contain the prefix and slug
    expect(pill?.textContent).toContain("#");
    expect(pill?.textContent).toContain("bug");
  });
});
