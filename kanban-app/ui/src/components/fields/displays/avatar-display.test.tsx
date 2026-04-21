import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve("ok")),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock("@tauri-apps/api/webviewWindow", () => ({
  getCurrentWebviewWindow: () => ({
    label: "main",
    listen: vi.fn(() => Promise.resolve(() => {})),
  }),
}));
vi.mock("@tauri-apps/plugin-log", () => ({
  error: vi.fn(),
  warn: vi.fn(),
  info: vi.fn(),
  debug: vi.fn(),
  trace: vi.fn(),
  attachConsole: vi.fn(() => Promise.resolve()),
}));

import { AvatarDisplay } from "./avatar-display";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";

import { TooltipProvider } from "@/components/ui/tooltip";
import type { Entity } from "@/types/kanban";

/** Wrap AvatarDisplay in required providers. */
function renderDisplay(value: unknown, actors: Entity[] = []) {
  return render(
    <TooltipProvider>
      <SchemaProvider>
        <EntityStoreProvider entities={{ actor: actors }}>
          <EntityFocusProvider>
            <AvatarDisplay value={value} />
          </EntityFocusProvider>
        </EntityStoreProvider>
      </SchemaProvider>
    </TooltipProvider>,
  );
}

function makeActor(
  id: string,
  name: string,
  overrides: Record<string, unknown> = {},
): Entity {
  return {
    entity_type: "actor",
    id,
    moniker: `actor:${id}`,
    fields: { name, ...overrides },
  };
}

const DATA_URI = "data:image/svg+xml;base64,PHN2Zz48L3N2Zz4=";

describe("AvatarDisplay", () => {
  it("renders dash for empty array", () => {
    renderDisplay([]);
    expect(screen.getByText("-")).toBeTruthy();
  });

  it("renders dash for null/undefined", () => {
    renderDisplay(null);
    expect(screen.getByText("-")).toBeTruthy();
  });

  it("renders Avatar components for array of actor IDs", () => {
    const actors = [makeActor("alice", "Alice Smith")];
    renderDisplay(["alice"], actors);
    // Should render initials for Alice
    expect(screen.getByText("AS")).toBeTruthy();
  });

  it("renders an image directly for a string URL value", () => {
    const { container } = renderDisplay(DATA_URI);
    const img = container.querySelector("img");
    expect(img).not.toBeNull();
    expect(img!.src).toBe(DATA_URI);
  });

  it("renders an image for an https URL string", () => {
    const url = "https://example.com/avatar.png";
    const { container } = renderDisplay(url);
    const img = container.querySelector("img");
    expect(img).not.toBeNull();
    expect(img!.src).toBe(url);
  });

  it("renders dash for empty string", () => {
    renderDisplay("");
    expect(screen.getByText("-")).toBeTruthy();
  });

  it("image is rendered as a circle (rounded-full)", () => {
    const { container } = renderDisplay(DATA_URI);
    const img = container.querySelector("img")!;
    expect(img.className).toContain("rounded-full");
  });
});
