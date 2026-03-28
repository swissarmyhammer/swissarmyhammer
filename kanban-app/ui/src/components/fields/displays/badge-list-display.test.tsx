import { describe, it, expect, vi } from "vitest";
import { render } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Mocks — must be declared before importing the component under test
// ---------------------------------------------------------------------------

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve("ok")),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

const mockTags = [
  {
    id: "tag-1",
    entity_type: "tag",
    fields: { tag_name: "bugfix", color: "ff0000" },
  },
  {
    id: "tag-2",
    entity_type: "tag",
    fields: { tag_name: "feature", color: "00ff00" },
  },
];

vi.mock("@/lib/entity-store-context", () => ({
  useEntityStore: () => ({ getEntities: () => mockTags, getEntity: vi.fn() }),
}));

vi.mock("@/lib/schema-context", () => ({
  useSchema: () => ({
    getSchema: () => undefined,
    getFieldDef: () => undefined,
    getEntityCommands: () => [],
    mentionableTypes: [
      { entityType: "tag", prefix: "#", displayField: "tag_name" },
    ],
    loading: false,
  }),
  useSchemaOptional: () => ({
    getSchema: () => undefined,
    getFieldDef: () => undefined,
    getEntityCommands: () => [],
  }),
}));

vi.mock("@/lib/entity-focus-context", () => ({
  useEntityFocus: () => ({
    focusedMoniker: null,
    setFocus: vi.fn(),
    registerScope: vi.fn(),
    unregisterScope: vi.fn(),
    registerClaimPredicates: vi.fn(),
    unregisterClaimPredicates: vi.fn(),
    getScope: () => null,
  }),
  useIsFocused: () => false,
}));

// ---------------------------------------------------------------------------

import { BadgeListDisplay } from "./badge-list-display";
import { TooltipProvider } from "@/components/ui/tooltip";
import { InspectProvider } from "@/lib/inspect-context";
import type { Entity, FieldDef } from "@/types/kanban";

const tagField: FieldDef = {
  name: "tags",
  display: "badge-list",
  type: { entity: "tag", commit_display_names: true },
} as unknown as FieldDef;

const taskEntity: Entity = {
  id: "task-1",
  entity_type: "task",
  fields: { tags: ["bugfix", "feature"] },
};

function renderDisplay(overrides: {
  value?: unknown;
} = {}) {
  return render(
    <TooltipProvider>
      <InspectProvider onInspect={() => {}} onDismiss={() => false}>
        <BadgeListDisplay
          field={tagField}
          value={overrides.value ?? ["bugfix", "feature"]}
          entity={taskEntity}
          mode="full"
        />
      </InspectProvider>
    </TooltipProvider>,
  );
}

/** Get all pill spans (the ones with data-moniker from FocusScope wrappers). */
function getPills(container: HTMLElement) {
  return Array.from(container.querySelectorAll("[data-moniker]"));
}

/** Get the inner pill span elements (the rounded-full badges inside data-moniker wrappers). */
function getPillSpans(container: HTMLElement) {
  return Array.from(
    container.querySelectorAll("[data-moniker] span"),
  ).filter(
    (el) => el.classList.contains("rounded-full"),
  );
}

describe("BadgeListDisplay", () => {
  it("renders all pills for tag values", () => {
    const { container } = renderDisplay();
    const pills = getPillSpans(container);
    expect(pills.length).toBe(2);
    expect(pills[0].textContent).toContain("bugfix");
    expect(pills[1].textContent).toContain("feature");
  });

  it("renders empty state when values are empty", () => {
    const { container } = renderDisplay({ value: [] });
    const pills = getPillSpans(container);
    expect(pills.length).toBe(0);
  });

  it("renders pills inside FocusScope wrappers (data-moniker)", () => {
    const { container } = renderDisplay();
    const scopes = getPills(container);
    expect(scopes.length).toBe(2);
  });
});
