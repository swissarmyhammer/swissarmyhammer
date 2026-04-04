import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";

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

import { Avatar } from "./avatar";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";

import { TooltipProvider } from "@/components/ui/tooltip";
import type { Entity } from "@/types/kanban";

/** Render Avatar inside required providers with a configurable entity store. */
function renderAvatar(
  actorId: string,
  actors: Entity[],
  size?: "sm" | "md" | "lg",
) {
  return render(
    <TooltipProvider>
      <SchemaProvider>
        <EntityStoreProvider entities={{ actor: actors }}>
          <EntityFocusProvider>
            <Avatar actorId={actorId} size={size} />
          </EntityFocusProvider>
        </EntityStoreProvider>
      </SchemaProvider>
    </TooltipProvider>,
  );
}

/** Create a minimal actor entity. */
function makeActor(
  id: string,
  name: string,
  overrides: Record<string, unknown> = {},
): Entity {
  return {
    entity_type: "actor",
    id,
    fields: {
      name,
      ...overrides,
    },
  };
}

const DATA_URI = "data:image/svg+xml;base64,PHN2Zz48L3N2Zz4=";

describe("Avatar", () => {
  it("renders an <img> when actor has a data:image avatar", () => {
    const actor = makeActor("alice", "Alice Smith", { avatar: DATA_URI });
    const { container } = renderAvatar("alice", [actor]);

    const img = container.querySelector("img");
    expect(img).not.toBeNull();
    expect(img!.src).toBe(DATA_URI);
    expect(img!.alt).toBe("Alice Smith");
  });

  it("image avatar uses rounded-full (circle, not rounded rectangle)", () => {
    const actor = makeActor("alice", "Alice Smith", { avatar: DATA_URI });
    const { container } = renderAvatar("alice", [actor]);

    const img = container.querySelector("img")!;
    expect(img.className).toContain("rounded-full");
    expect(img.className).not.toMatch(/rounded-(?:lg|md|sm|xl|2xl)\b/);
  });

  it("image avatar uses object-cover for proper circle fit", () => {
    const actor = makeActor("alice", "Alice Smith", { avatar: DATA_URI });
    const { container } = renderAvatar("alice", [actor]);

    const img = container.querySelector("img")!;
    expect(img.className).toContain("object-cover");
  });

  it("falls back to initials when actor has no avatar field", () => {
    const actor = makeActor("alice", "Alice Smith");
    const { container } = renderAvatar("alice", [actor]);

    expect(container.querySelector("img")).toBeNull();
    expect(screen.getByText("AS")).toBeTruthy();
  });

  it("initials fallback uses rounded-full (circle)", () => {
    const actor = makeActor("alice", "Alice Smith");
    renderAvatar("alice", [actor]);

    const span = screen.getByText("AS");
    expect(span.className).toContain("rounded-full");
    expect(span.className).not.toMatch(/rounded-(?:lg|md|sm|xl|2xl)\b/);
  });

  it("falls back to initials when avatar is empty string", () => {
    const actor = makeActor("alice", "Alice Smith", { avatar: "" });
    const { container } = renderAvatar("alice", [actor]);

    expect(container.querySelector("img")).toBeNull();
    expect(screen.getByText("AS")).toBeTruthy();
  });

  it("shows actorId-based initials when actor is not in store", () => {
    const { container } = renderAvatar("unknown-user", []);

    expect(container.querySelector("img")).toBeNull();
    // Falls back to first char of actorId
    expect(screen.getByText("U")).toBeTruthy();
  });

  it("applies size classes correctly for all sizes", () => {
    const actor = makeActor("alice", "Alice Smith", { avatar: DATA_URI });

    for (const size of ["sm", "md", "lg"] as const) {
      const { container: c, unmount } = renderAvatar("alice", [actor], size);
      const img = c.querySelector("img")!;
      expect(img).not.toBeNull();
      // All sizes should be round
      expect(img.className).toContain("rounded-full");
      unmount();
    }
  });

  it("single-name actor gets single initial", () => {
    const actor = makeActor("bob", "Bob");
    renderAvatar("bob", [actor]);

    expect(screen.getByText("B")).toBeTruthy();
  });
});
