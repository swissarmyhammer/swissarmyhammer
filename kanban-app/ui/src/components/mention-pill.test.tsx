import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, fireEvent, waitFor } from "@testing-library/react";
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
  "../../../../swissarmyhammer-kanban/builtin/entities",
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
// Backend context-menu types
// ---------------------------------------------------------------------------

/**
 * Shape returned by the backend `list_commands_for_scope`.
 * Used to build mock responses for context menu tests.
 */
interface ResolvedCommand {
  id: string;
  name: string;
  target?: string;
  group: string;
  context_menu: boolean;
  keys?: { vim?: string; cua?: string; emacs?: string };
  available: boolean;
}

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const mockInvoke = vi.fn((..._args: any[]) => Promise.resolve(undefined));

vi.mock("@tauri-apps/api/core", () => ({
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  invoke: (...args: any[]) => mockInvoke(...args),
}));

/**
 * Helper: configure invoke mock to return the given commands when
 * `list_commands_for_scope` is called, and resolve for everything else.
 */
function mockListCommands(commands: ResolvedCommand[]) {
  mockInvoke.mockImplementation(
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (cmd: string, _args?: any): Promise<any> => {
      if (cmd === "list_commands_for_scope") return Promise.resolve(commands);
      return Promise.resolve(undefined);
    },
  );
}
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main" }),
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
  return render(
    <TooltipProvider>
      <EntityFocusProvider>
        <MentionPill {...props} />
      </EntityFocusProvider>
    </TooltipProvider>,
  );
}

describe("MentionPill", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockInvoke.mockResolvedValue(undefined);
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

  it("right-click shows context menu with ui.inspect and task.untag for tags", async () => {
    mockListCommands([
      {
        id: "ui.inspect",
        name: "Inspect Tag",
        target: "tag:tag-1",
        group: "entity",
        context_menu: true,
        available: true,
      },
      {
        id: "task.untag",
        name: "Remove Tag",
        group: "entity",
        context_menu: true,
        available: true,
      },
    ]);
    const { container } = renderPill({
      entityType: "tag",
      slug: "bugfix",
      prefix: "#",
      taskId: "task-1",
    });
    const pill = container.querySelector("[data-moniker]")!;
    fireEvent.contextMenu(pill);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("show_context_menu", {
        items: expect.arrayContaining([
          expect.objectContaining({
            cmd: "ui.inspect",
            target: "tag:tag-1",
            name: "Inspect Tag",
          }),
          expect.objectContaining({ cmd: "task.untag", name: "Remove Tag" }),
        ]),
      });
    });
  });

  it("task.untag not available when taskId is undefined", async () => {
    // Backend only returns inspect — no task.untag since no taskId context
    mockListCommands([
      {
        id: "ui.inspect",
        name: "Inspect Tag",
        target: "tag:tag-1",
        group: "entity",
        context_menu: true,
        available: true,
      },
    ]);
    const { container } = renderPill({
      entityType: "tag",
      slug: "bugfix",
      prefix: "#",
    });
    const pill = container.querySelector("[data-moniker]")!;
    fireEvent.contextMenu(pill);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("show_context_menu", {
        items: expect.arrayContaining([
          expect.objectContaining({
            cmd: "ui.inspect",
            target: "tag:tag-1",
            name: "Inspect Tag",
          }),
        ]),
      });
    });
    // Verify task.untag was NOT included
    const showCall = mockInvoke.mock.calls.find(
      (c: unknown[]) => c[0] === "show_context_menu",
    );
    const items = (showCall![1] as { items: { cmd: string }[] }).items;
    expect(items.find((i) => i.cmd === "task.untag")).toBeUndefined();
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

  it("unresolved entity + parent: both inspect commands accumulate", async () => {
    mockGetEntities.mockReturnValue([]);
    // Backend returns both inspect commands — one for the tag pill, one for the parent task
    mockListCommands([
      {
        id: "ui.inspect",
        name: "Inspect Tag",
        target: "tag:unknown-tag",
        group: "entity",
        context_menu: true,
        available: true,
      },
      {
        id: "ui.inspect",
        name: "Inspect task",
        target: "task:parent",
        group: "entity",
        context_menu: true,
        available: true,
      },
    ]);
    const { container } = render(
      <TooltipProvider>
        <EntityFocusProvider>
          <FocusScope
            moniker="task:parent"
            commands={[
              {
                id: "ui.inspect",
                name: "Inspect task",
                target: "task:parent",
                contextMenu: true,
              },
            ]}
          >
            <MentionPill entityType="tag" slug="unknown-tag" prefix="#" />
          </FocusScope>
        </EntityFocusProvider>
      </TooltipProvider>,
    );
    const pill = container.querySelector("[data-moniker='tag:unknown-tag']")!;
    fireEvent.contextMenu(pill);

    await waitFor(() => {
      const ctxCall = mockInvoke.mock.calls.find(
        (c: unknown[]) => c[0] === "show_context_menu",
      );
      expect(ctxCall).toBeTruthy();
      const items = (ctxCall![1] as { items: { id: string; name: string }[] })
        .items;
      expect(items.find((i) => i.name === "Inspect Tag")).toBeTruthy();
      expect(items.find((i) => i.name === "Inspect task")).toBeTruthy();
    });
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
