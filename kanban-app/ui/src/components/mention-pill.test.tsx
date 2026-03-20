import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, fireEvent } from "@testing-library/react";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const mockInvoke = vi.fn((..._args: any[]) => Promise.resolve("ok"));

vi.mock("@tauri-apps/api/core", () => ({
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  invoke: (...args: any[]) => mockInvoke(...args),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

// Mock entity store to return test entities
const mockGetEntities = vi.fn(() => mockTags);
vi.mock("@/lib/entity-store-context", () => ({
  useEntityStore: () => ({ getEntities: mockGetEntities, getEntity: vi.fn() }),
}));

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
          name: "Inspect tag",
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
      items: [{ id: "entity.inspect:tag:tag-1", name: "Inspect tag" }],
    });
  });

  it("entity.inspect uses resolved entity id, not slug", () => {
    const { container } = renderPill({
      entityType: "tag",
      slug: "bugfix",
      prefix: "#",
    });
    const pill = container.querySelector("[data-moniker='tag:tag-1']");
    expect(pill).not.toBeNull();
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

  it("resolves entities by slugified title match", () => {
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

  it("works for non-tag entity types like actor", () => {
    const actors: Entity[] = [
      {
        id: "actor-1",
        entity_type: "actor",
        fields: { name: "alice", color: "00ff00" },
      },
    ];
    mockGetEntities.mockReturnValue(actors);
    const { container } = renderPill({
      entityType: "actor",
      slug: "alice",
      prefix: "@",
    });
    const pill = container.querySelector("[data-moniker='actor:actor-1']");
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
    expect(items.find((i) => i.name === "Inspect tag")).toBeTruthy();
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
