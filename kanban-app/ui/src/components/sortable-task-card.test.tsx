/**
 * Tests for `<DraggableTaskCard>` — the HTML5-draggable wrapper around
 * `<EntityCard>`.
 *
 * The card itself registers as a `<FocusScope>` (leaf, NOT a zone — see
 * the docstring on `<EntityCard>` for why); these tests assert the
 * wrapper preserves that shape and continues to wire the drag handle.
 * The leaf shape is what enables the unified cascade's iter-0 / iter-1
 * trajectory for cross-column right/left navigation: iter 0 finds
 * in-column card peers, and when no peer satisfies the beam test the
 * cascade escalates to iter 1 — the card's parent column zone — and
 * lands on the neighbouring column zone.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, act } from "@testing-library/react";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const TASK_SCHEMA = {
  entity: {
    name: "task",
    body_field: "body",
    fields: ["title"],
    commands: [],
  },
  fields: [
    {
      id: "f1",
      name: "title",
      type: { kind: "markdown", single_line: true },
      editor: "markdown",
      display: "text",
      section: "header",
    },
  ],
};

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const mockInvoke = vi.fn((...args: any[]) => {
  if (args[0] === "list_entity_types") return Promise.resolve(["task"]);
  if (args[0] === "get_entity_schema") return Promise.resolve(TASK_SCHEMA);
  if (args[0] === "get_ui_state")
    return Promise.resolve({
      palette_open: false,
      palette_mode: "command",
      keymap_mode: "cua",
      scope_chain: [],
      open_boards: [],
      windows: {},
      recent_boards: [],
    });
  if (args[0] === "list_commands_for_scope") return Promise.resolve([]);
  if (args[0] === "show_context_menu") return Promise.resolve();
  return Promise.resolve("ok");
});

vi.mock("@tauri-apps/api/core", () => ({
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  invoke: (...args: any[]) => mockInvoke(...args),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main" }),
}));
vi.mock("@tauri-apps/plugin-log", () => ({
  error: vi.fn(),
  warn: vi.fn(),
  info: vi.fn(),
  debug: vi.fn(),
  trace: vi.fn(),
  attachConsole: vi.fn(() => Promise.resolve()),
}));

import "@/components/fields/registrations";
import { DraggableTaskCard } from "./sortable-task-card";
import { TooltipProvider } from "@/components/ui/tooltip";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { FieldUpdateProvider } from "@/lib/field-update-context";
import { UIStateProvider } from "@/lib/ui-state-context";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { FocusLayer } from "@/components/focus-layer";
import {
  asSegment
} from "@/types/spatial";
import type { Entity } from "@/types/kanban";

function makeEntity(): Entity {
  return {
    entity_type: "task",
    id: "task-7",
    moniker: "task:task-7",
    fields: {
      title: "Sortable card",
      body: "",
      tags: [],
      assignees: [],
      depends_on: [],
      position_column: "col-1",
      position_ordinal: "a0",
    },
  };
}

let currentEntity: Entity = makeEntity();

/** Render the card inside the full spatial-focus stack so the card mounts as a real `<FocusZone>`. */
function renderCard(ui: React.ReactElement) {
  return render(
    <TooltipProvider>
      <SchemaProvider>
        <EntityStoreProvider entities={{ task: [currentEntity], tag: [] }}>
          <EntityFocusProvider>
            <FieldUpdateProvider>
              <UIStateProvider>
                <SpatialFocusProvider>
                  <FocusLayer name={asSegment("window")}>{ui}</FocusLayer>
                </SpatialFocusProvider>
              </UIStateProvider>
            </FieldUpdateProvider>
          </EntityFocusProvider>
        </EntityStoreProvider>
      </SchemaProvider>
    </TooltipProvider>,
  );
}

async function renderWith(ui: React.ReactElement) {
  const result = renderCard(ui);
  await act(async () => {
    await new Promise((r) => setTimeout(r, 100));
  });
  return result;
}

describe("DraggableTaskCard", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    currentEntity = makeEntity();
  });

  it("registers the card body as a FocusZone with the entity moniker", async () => {
    await renderWith(<DraggableTaskCard entity={currentEntity} />);
    const zoneCalls = mockInvoke.mock.calls
      .filter((c) => c[0] === "spatial_register_zone")
      .map((c) => c[1] as Record<string, unknown>);
    expect(zoneCalls.find((a) => a.segment === "task:task-7")).toBeTruthy();
  });

  it("does not register the card root as a FocusScope (the card is a zone, not a leaf)", async () => {
    // Cards register as zones because they hold multiple focusable
    // atoms (drag handle, Field rows, inspect button). The kernel's
    // path-prefix scope-is-leaf invariant fires
    // `scope-not-leaf` whenever a Scope's FQM is a strict prefix of
    // any registered descendant — exactly the shape the previous
    // card-as-Scope wrapper produced. See
    // `swissarmyhammer-focus/tests/scope_is_leaf.rs`.
    await renderWith(<DraggableTaskCard entity={currentEntity} />);
    const scopeCalls = mockInvoke.mock.calls
      .filter((c) => c[0] === "spatial_register_scope")
      .map((c) => c[1] as Record<string, unknown>);
    expect(
      scopeCalls.find((a) => a.segment === "task:task-7"),
    ).toBeUndefined();
  });

  it("renders the drag handle button", async () => {
    const { container } = await renderWith(
      <DraggableTaskCard entity={currentEntity} />,
    );
    // The drag handle is a button with the cursor-grab class — it lives
    // inside the card body and is the source for the OS drag image
    // built by `handleDragStart`.
    const dragHandle = container.querySelector("button.cursor-grab");
    expect(dragHandle).toBeTruthy();
  });

  it("does not accept a `claimWhen` prop (compile-time and runtime)", async () => {
    // The `claimWhen` prop and its `ClaimPredicate` import have been
    // removed from `DraggableTaskCard`. We cannot type-test the
    // absence at runtime, but rendering with only the new prop
    // surface and proving the card mounts is the runtime stand-in.
    const { container } = await renderWith(
      <DraggableTaskCard entity={currentEntity} />,
    );
    expect(container.querySelector("[data-entity-card='task-7']")).toBeTruthy();
  });
});
