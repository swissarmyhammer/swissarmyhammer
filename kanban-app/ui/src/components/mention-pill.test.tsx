import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, fireEvent } from "@testing-library/react";
import { readdirSync, readFileSync } from "fs";
import { join } from "path";
import yaml from "js-yaml";
import type { MentionableType } from "@/lib/schema-context";
import type { EntityCommand } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Load entity definitions from the actual builtin YAML files.
// Adding a new entity YAML with mention_prefix automatically covers it here.
// ---------------------------------------------------------------------------

const ENTITIES_DIR = join(
  __dirname,
  "../../../../swissarmyhammer-kanban/builtin/fields/entities",
);

interface EntityYaml {
  name: string;
  mention_prefix?: string;
  mention_display_field?: string;
  commands?: EntityCommand[];
  fields?: string[];
}

const entityDefs: EntityYaml[] = readdirSync(ENTITIES_DIR)
  .filter((f) => f.endsWith(".yaml"))
  .map(
    (f) => yaml.load(readFileSync(join(ENTITIES_DIR, f), "utf8")) as EntityYaml,
  );

/** Mentionable types derived from YAML — same logic as SchemaProvider. */
const MENTIONABLE_TYPES: MentionableType[] = entityDefs
  .filter((e) => e.mention_prefix && e.mention_display_field)
  .map((e) => ({
    entityType: e.name,
    prefix: e.mention_prefix!,
    displayField: e.mention_display_field!,
  }));

/** Commands by entity type, from YAML. */
const commandsByType = new Map<string, EntityCommand[]>(
  entityDefs.map((e) => [e.name, (e.commands ?? []) as EntityCommand[]]),
);

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const mockInvoke = vi.fn((..._args: any[]) => Promise.resolve("ok"));

vi.mock("@tauri-apps/api/core", () => ({
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  invoke: (...args: any[]) => mockInvoke(...args),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

const mockGetEntities = vi.fn(() => mockTags);
vi.mock("@/lib/entity-store-context", () => ({
  useEntityStore: () => ({ getEntities: mockGetEntities, getEntity: vi.fn() }),
}));

vi.mock("@/lib/schema-context", () => ({
  useSchema: () => ({
    getSchema: () => undefined,
    getFieldDef: () => undefined,
    getEntityCommands: (entityType: string) =>
      commandsByType.get(entityType) ?? [],
    mentionableTypes: MENTIONABLE_TYPES,
    loading: false,
  }),
  useSchemaOptional: () => ({
    getSchema: () => undefined,
    getFieldDef: () => undefined,
    getEntityCommands: (entityType: string) =>
      commandsByType.get(entityType) ?? [],
  }),
}));

// ---------------------------------------------------------------------------

import { MentionPill } from "./mention-pill";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { InspectProvider } from "@/lib/inspect-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import { FocusScope } from "@/components/focus-scope";
import type { Entity } from "@/types/kanban";

const mockTag: Entity = {
  id: "tag-1",
  entity_type: "tag",
  fields: {
    tag_name: "bugfix",
    color: "ff0000",
    description: "Bug fix tag",
  },
};

const mockTags: Entity[] = [mockTag];

function renderPill(props: {
  entityType: string;
  slug: string;
  prefix: string;
  taskId?: string;
}) {
  const onInspect = vi.fn();
  return {
    onInspect,
    ...render(
      <TooltipProvider>
        <EntityFocusProvider>
          <InspectProvider onInspect={onInspect} onDismiss={() => false}>
            <MentionPill {...props} />
          </InspectProvider>
        </EntityFocusProvider>
      </TooltipProvider>,
    ),
  };
}

describe("MentionPill", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockGetEntities.mockReturnValue(mockTags);
  });

  // --- Data-driven: every mentionable entity type resolves by its display field ---

  it("loads mentionable types from YAML", () => {
    expect(MENTIONABLE_TYPES.length).toBeGreaterThanOrEqual(3);
    for (const mt of MENTIONABLE_TYPES) {
      expect(mt.entityType).toBeTruthy();
      expect(mt.prefix).toBeTruthy();
      expect(mt.displayField).toBeTruthy();
    }
  });

  for (const mt of MENTIONABLE_TYPES) {
    it(`resolves ${mt.entityType} entity by ${mt.displayField} field`, () => {
      const entity: Entity = {
        id: `${mt.entityType}-99`,
        entity_type: mt.entityType,
        fields: { [mt.displayField]: "test-value", color: "aabbcc" },
      };
      mockGetEntities.mockReturnValue([entity]);
      const { container } = renderPill({
        entityType: mt.entityType,
        slug: "test-value",
        prefix: mt.prefix,
      });
      const pill = container.querySelector(
        `[data-moniker='${mt.entityType}:${mt.entityType}-99']`,
      );
      expect(pill).not.toBeNull();
    });
  }

  // --- Specific behavior tests ---

  it("right-click shows context menu with entity.inspect and task.untag for tags", () => {
    const { container } = renderPill({
      entityType: "tag",
      slug: "bugfix",
      prefix: "#",
      taskId: "task-1",
    });
    const pill = container.querySelector("[data-moniker]")!;
    fireEvent.contextMenu(pill);

    expect(mockInvoke).toHaveBeenCalledWith("show_context_menu", {
      items: expect.arrayContaining([
        expect.objectContaining({
          id: "entity.inspect:tag:tag-1",
          name: "Inspect Tag",
        }),
        expect.objectContaining({ id: "task.untag", name: "Remove Tag" }),
      ]),
    });
  });

  it("task.untag not available when taskId is undefined", () => {
    const { container } = renderPill({
      entityType: "tag",
      slug: "bugfix",
      prefix: "#",
    });
    const pill = container.querySelector("[data-moniker]")!;
    fireEvent.contextMenu(pill);

    expect(mockInvoke).toHaveBeenCalledWith("show_context_menu", {
      items: expect.arrayContaining([
        expect.objectContaining({
          id: "entity.inspect:tag:tag-1",
          name: "Inspect Tag",
        }),
      ]),
    });
  });

  it("falls back to slug moniker when entity not found", () => {
    mockGetEntities.mockReturnValue([]);
    const { container } = renderPill({
      entityType: "tag",
      slug: "unknown-tag",
      prefix: "#",
    });
    const pill = container.querySelector("[data-moniker='tag:unknown-tag']");
    expect(pill).not.toBeNull();
  });

  it("resolves entities by slugified display field match", () => {
    const taskEntities: Entity[] = [
      {
        id: "task-42",
        entity_type: "task",
        fields: { title: "Fix Login Bug", color: "3366ff" },
      },
    ];
    mockGetEntities.mockReturnValue(taskEntities);
    const { container } = renderPill({
      entityType: "task",
      slug: "fix-login-bug",
      prefix: "^",
    });
    const pill = container.querySelector("[data-moniker='task:task-42']");
    expect(pill).not.toBeNull();
  });

  it("unresolved entity + parent: both inspect commands accumulate", () => {
    mockGetEntities.mockReturnValue([]);
    const onInspect = vi.fn();
    const { container } = render(
      <TooltipProvider>
        <EntityFocusProvider>
          <InspectProvider onInspect={onInspect} onDismiss={() => false}>
            <FocusScope
              moniker="task:parent"
              commands={[
                {
                  id: "entity.inspect",
                  name: "Inspect task",
                  target: "task:parent",
                  contextMenu: true,
                  execute: vi.fn(),
                },
              ]}
            >
              <MentionPill entityType="tag" slug="unknown-tag" prefix="#" />
            </FocusScope>
          </InspectProvider>
        </EntityFocusProvider>
      </TooltipProvider>,
    );
    const pill = container.querySelector("[data-moniker='tag:unknown-tag']")!;
    fireEvent.contextMenu(pill);

    const ctxCall = mockInvoke.mock.calls.find(
      (c: unknown[]) => c[0] === "show_context_menu",
    );
    expect(ctxCall).toBeTruthy();
    const items = (ctxCall![1] as { items: { id: string; name: string }[] })
      .items;
    expect(items.find((i) => i.name === "Inspect Tag")).toBeTruthy();
    expect(items.find((i) => i.name === "Inspect task")).toBeTruthy();
  });

  it("FocusScope wrapping does not break inline layout", () => {
    const { container } = renderPill({
      entityType: "tag",
      slug: "bugfix",
      prefix: "#",
    });
    const scopeDiv = container.querySelector("[data-moniker]") as HTMLElement;
    expect(scopeDiv).not.toBeNull();
    expect(scopeDiv.classList.contains("inline")).toBe(true);
  });
});
