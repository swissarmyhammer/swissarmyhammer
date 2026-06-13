import { describe, it, expect, vi } from "vitest";
import { screen } from "@testing-library/react";
import { renderInAct } from "@/test/act-render";

// ---------------------------------------------------------------------------
// Mocks — must be declared before importing the component under test
// ---------------------------------------------------------------------------

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve("ok")),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock("@tauri-apps/plugin-log", () => ({
  error: vi.fn(),
  warn: vi.fn(),
  info: vi.fn(),
  debug: vi.fn(),
  trace: vi.fn(),
  attachConsole: vi.fn(() => Promise.resolve()),
}));

import { CommentLogDisplay } from "./comment-log-display";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { FocusLayer } from "@/components/focus-layer";
import { TooltipProvider } from "@/components/ui/tooltip";
import { asSegment } from "@/types/spatial";
import type { Entity, FieldDef } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/** Mirrors `builtin/definitions/comments.yaml`. */
const COMMENTS_FIELD: FieldDef = {
  id: "f-comments",
  name: "comments",
  description: "Conversation log",
  type: { kind: "comment-log" },
  icon: "message-square",
  editor: "comment-log",
  display: "comment-log",
  section: "log",
};

const COMMENT_A = {
  id: "01aaaaaaaaaaaaaaaaaaaaaaaaaa",
  actor: "alice",
  text: "First comment",
  timestamp: "2026-01-01T00:00:00+00:00",
};

const COMMENT_B = {
  id: "01bbbbbbbbbbbbbbbbbbbbbbbbbb",
  actor: "bob",
  text: "Second comment",
  timestamp: "2026-01-02T00:00:00+00:00",
};

function makeActor(id: string, name: string): Entity {
  return {
    entity_type: "actor",
    id,
    moniker: `actor:${id}`,
    fields: { name },
  };
}

const ACTORS = [makeActor("alice", "Alice Smith"), makeActor("bob", "Bob Jones")];

async function renderDisplay(
  value: unknown,
  options: { field?: FieldDef; mode?: "compact" | "full" } = {},
) {
  const field = options.field ?? COMMENTS_FIELD;
  const mode = options.mode ?? "full";
  return await renderInAct(
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <TooltipProvider>
          <SchemaProvider>
            <EntityStoreProvider entities={{ actor: ACTORS }}>
              <EntityFocusProvider>
                <CommentLogDisplay field={field} value={value} mode={mode} />
              </EntityFocusProvider>
            </EntityStoreProvider>
          </SchemaProvider>
        </TooltipProvider>
      </FocusLayer>
    </SpatialFocusProvider>,
  );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("CommentLogDisplay — full mode", () => {
  it("renders both member texts", async () => {
    await renderDisplay([COMMENT_A, COMMENT_B]);
    expect(screen.getByText("First comment")).toBeTruthy();
    expect(screen.getByText("Second comment")).toBeTruthy();
  });

  it("renders both resolved actor names from the entity store", async () => {
    await renderDisplay([COMMENT_A, COMMENT_B]);
    expect(screen.getByText("Alice Smith")).toBeTruthy();
    expect(screen.getByText("Bob Jones")).toBeTruthy();
  });

  it("falls back to the actor id when the actor entity is unknown", async () => {
    await renderDisplay([{ ...COMMENT_A, actor: "ghost" }]);
    expect(screen.getByText("ghost")).toBeTruthy();
  });

  it("renders members in id order even when the value array is unordered", async () => {
    const { container } = await renderDisplay([COMMENT_B, COMMENT_A]);
    const items = container.querySelectorAll("[data-comment-id]");
    expect(items.length).toBe(2);
    expect(items[0].getAttribute("data-comment-id")).toBe(COMMENT_A.id);
    expect(items[1].getAttribute("data-comment-id")).toBe(COMMENT_B.id);
  });

  it("exposes the raw timestamp as a title tooltip on each member", async () => {
    const { container } = await renderDisplay([COMMENT_A]);
    expect(
      container.querySelector(`[title="${COMMENT_A.timestamp}"]`),
    ).toBeTruthy();
  });

  it("renders the italic 'None' empty state when there are no comments", async () => {
    const { container } = await renderDisplay([]);
    const hint = container.querySelector("span.italic");
    expect(hint).toBeTruthy();
    expect(hint?.textContent).toBe("None");
  });

  it("honors the field placeholder in the empty state", async () => {
    const fieldWithPlaceholder: FieldDef = {
      ...COMMENTS_FIELD,
      placeholder: "No comments yet",
    };
    const { container } = await renderDisplay([], {
      field: fieldWithPlaceholder,
    });
    const hint = container.querySelector("span.italic");
    expect(hint?.textContent).toBe("No comments yet");
  });

  it("filters out tombstones and invalid entries", async () => {
    const { container } = await renderDisplay([
      COMMENT_A,
      { id: COMMENT_B.id, deleted: true },
      42,
      null,
      { notAnId: "oops" },
    ]);
    const items = container.querySelectorAll("[data-comment-id]");
    expect(items.length).toBe(1);
    expect(screen.getByText("First comment")).toBeTruthy();
  });
});

describe("CommentLogDisplay — compact mode", () => {
  it("renders the member count in a compact cell wrapper", async () => {
    const { container } = await renderDisplay([COMMENT_A, COMMENT_B], {
      mode: "compact",
    });
    const wrapper = container.querySelector("[data-compact-cell='true']");
    expect(wrapper).toBeTruthy();
    expect(wrapper?.textContent).toContain("2");
  });

  it("renders the dash fallback when empty and no placeholder is set", async () => {
    const { container } = await renderDisplay([], { mode: "compact" });
    const wrapper = container.querySelector("[data-compact-cell='true']");
    expect(wrapper).toBeTruthy();
    expect(wrapper?.textContent).toBe("-");
  });

  it("compact populated and empty variants share the CompactCellWrapper contract", async () => {
    const populated = await renderDisplay([COMMENT_A], { mode: "compact" });
    const populatedWrapper = populated.container.querySelector(
      "[data-compact-cell='true']",
    );
    expect(populatedWrapper).toBeTruthy();
    const populatedClassName = populatedWrapper!.className;
    populated.unmount();

    const empty = await renderDisplay([], { mode: "compact" });
    const emptyWrapper = empty.container.querySelector(
      "[data-compact-cell='true']",
    );
    expect(emptyWrapper).toBeTruthy();
    expect(emptyWrapper!.className).toBe(populatedClassName);
    empty.unmount();
  });
});
