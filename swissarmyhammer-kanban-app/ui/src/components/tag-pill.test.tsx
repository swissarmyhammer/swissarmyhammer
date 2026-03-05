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

import { TagPill } from "./tag-pill";
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

function renderTagPill(props: { slug: string; tags?: Entity[]; taskId?: string }) {
  const onInspect = vi.fn();
  return {
    onInspect,
    ...render(
      <TooltipProvider>
        <EntityFocusProvider>
          <InspectProvider onInspect={onInspect}>
            <TagPill slug={props.slug} tags={props.tags ?? mockTags} taskId={props.taskId} />
          </InspectProvider>
        </EntityFocusProvider>
      </TooltipProvider>
    ),
  };
}

describe("TagPill", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
  });

  it("right-click shows context menu with entity.inspect and entity.remove", () => {
    const { container } = renderTagPill({ slug: "bugfix", taskId: "task-1" });
    const pill = container.querySelector("[data-moniker]")!;
    fireEvent.contextMenu(pill);

    expect(mockInvoke).toHaveBeenCalledWith("show_context_menu", {
      items: expect.arrayContaining([
        // target-aware: handler key is "id:target"
        expect.objectContaining({ id: "entity.inspect:tag:tag-1", name: "Inspect Tag" }),
        expect.objectContaining({ id: "entity.remove", name: "Remove Tag" }),
      ]),
    });
  });

  it("entity.remove not available when taskId is undefined", () => {
    const { container } = renderTagPill({ slug: "bugfix" });
    const pill = container.querySelector("[data-moniker]")!;
    fireEvent.contextMenu(pill);

    expect(mockInvoke).toHaveBeenCalledWith("show_context_menu", {
      items: [{ id: "entity.inspect:tag:tag-1", name: "Inspect Tag" }],
    });
  });

  it("entity.inspect command includes target moniker", () => {
    const { container } = renderTagPill({ slug: "bugfix", taskId: "task-1" });
    const pill = container.querySelector("[data-moniker]")!;
    fireEvent.contextMenu(pill);
    const ctxCall = mockInvoke.mock.calls.find((c: unknown[]) => c[0] === "show_context_menu");
    expect(ctxCall).toBeTruthy();
    const items = (ctxCall![1] as { items: { id: string }[] }).items;
    // target is tag:tag-1 (resolved tag moniker), so handler key is "entity.inspect:tag:tag-1"
    expect(items.find((i) => i.id === "entity.inspect:tag:tag-1")).toBeTruthy();
  });

  it("entity.inspect uses tag entity id, not slug", () => {
    const { container } = renderTagPill({ slug: "bugfix", taskId: "task-1" });
    // The moniker should use the resolved tag entity ID
    const pill = container.querySelector("[data-moniker='tag:tag-1']");
    expect(pill).not.toBeNull();
  });

  it("falls back to slug moniker when tag entity not found", () => {
    const { container } = renderTagPill({ slug: "unknown-tag", tags: [] });
    const pill = container.querySelector("[data-moniker='tag:unknown-tag']");
    expect(pill).not.toBeNull();
  });

  it("inspect tag always available even when tag entity not resolved", () => {
    const { container } = renderTagPill({ slug: "unknown-tag", tags: [] });
    const pill = container.querySelector("[data-moniker]")!;
    fireEvent.contextMenu(pill);

    // entity.inspect is always available now — uses slug-based moniker as fallback
    const ctxCall = mockInvoke.mock.calls.find((c: unknown[]) => c[0] === "show_context_menu");
    expect(ctxCall).toBeTruthy();
    const items = (ctxCall![1] as { items: { id: string; name: string }[] }).items;
    expect(items.find((i) => i.name === "Inspect Tag")).toBeTruthy();
  });

  it("unresolved tag + parent: both inspect commands accumulate", () => {
    const onInspect = vi.fn();
    const { container } = render(
      <TooltipProvider>
        <EntityFocusProvider>
          <InspectProvider onInspect={onInspect}>
            <FocusScope
              moniker="task:parent"
              commands={[{ id: "entity.inspect", name: "Inspect task", target: "task:parent", contextMenu: true, execute: vi.fn() }]}
            >
              <TagPill slug="unknown-tag" tags={[]} />
            </FocusScope>
          </InspectProvider>
        </EntityFocusProvider>
      </TooltipProvider>
    );
    const pill = container.querySelector("[data-moniker='tag:unknown-tag']")!;
    fireEvent.contextMenu(pill);

    const ctxCall = mockInvoke.mock.calls.find((c: unknown[]) => c[0] === "show_context_menu");
    expect(ctxCall).toBeTruthy();
    const items = (ctxCall![1] as { items: { id: string; name: string }[] }).items;
    expect(items.find((i) => i.name === "Inspect Tag")).toBeTruthy();
    expect(items.find((i) => i.name === "Inspect task")).toBeTruthy();
  });

  it("old show_tag_context_menu invoke is gone", () => {
    const { container } = renderTagPill({ slug: "bugfix", taskId: "task-1" });
    const pill = container.querySelector("[data-moniker]")!;
    fireEvent.contextMenu(pill);

    const calls = mockInvoke.mock.calls;
    const oldCalls = calls.filter((c: unknown[]) => c[0] === "show_tag_context_menu");
    expect(oldCalls).toHaveLength(0);
  });

  it("FocusScope wrapping does not break inline layout", () => {
    const { container } = renderTagPill({ slug: "bugfix" });
    const scopeDiv = container.querySelector("[data-moniker]") as HTMLElement;
    expect(scopeDiv).not.toBeNull();
    expect(scopeDiv.classList.contains("inline")).toBe(true);
  });
});
