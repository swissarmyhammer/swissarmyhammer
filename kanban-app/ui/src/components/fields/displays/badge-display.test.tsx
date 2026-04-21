/**
 * Tests for BadgeDisplay — the scalar-reference badge display for fields
 * like `position_column`. BadgeDisplay is a thin wrapper around MentionView
 * in single mode; the CM6 widget pipeline owns the rendered pill text and
 * color. This test file verifies that wrapping, plus the empty-state dash.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { act, render } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Tauri and plugin mocks — must be declared before importing the component
// ---------------------------------------------------------------------------

const mockInvoke = vi.fn(
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (..._args: any[]): Promise<unknown> => Promise.resolve(undefined),
);

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

// ---------------------------------------------------------------------------
// Mock entity store and schema — mutable for per-test customization
// ---------------------------------------------------------------------------

import type { Entity, FieldDef } from "@/types/kanban";
import type { MentionableType } from "@/lib/schema-context";

let mockEntities: Record<string, Entity[]> = {};
let mockMentionableTypes: MentionableType[] = [];

vi.mock("@/lib/entity-store-context", () => ({
  useEntityStore: () => ({
    getEntities: (type: string) => mockEntities[type] ?? [],
    getEntity: (type: string, id: string) =>
      mockEntities[type]?.find((e) => e.id === id),
  }),
}));

vi.mock("@/lib/schema-context", () => ({
  useSchema: () => ({
    getSchema: () => undefined,
    getFieldDef: () => undefined,
    mentionableTypes: mockMentionableTypes,
    loading: false,
  }),
  useSchemaOptional: () => ({
    getSchema: () => undefined,
    getFieldDef: () => undefined,
  }),
}));

vi.mock("@/components/window-container", () => ({
  useBoardData: () => null,
}));

// ---------------------------------------------------------------------------

import { BadgeDisplay } from "./badge-display";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { TooltipProvider } from "@/components/ui/tooltip";

/** Task entity used as the host of the `position_column` field in tests. */
const taskEntity: Entity = {
  id: "task-1",
  entity_type: "task",
  moniker: "task:task-1",
  fields: {},
};

/** Minimal provider tree MentionView needs. */
function Providers({ children }: { children: React.ReactNode }) {
  return (
    <EntityFocusProvider>
      <TooltipProvider>{children}</TooltipProvider>
    </EntityFocusProvider>
  );
}

/** Flush microtasks and pending effects (CM6 mounts asynchronously). */
async function flush() {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 0));
  });
}

/** Configure the mocked schema + entity store with column fixtures. */
function setupColumnFixtures() {
  mockEntities = {
    column: [
      {
        id: "col-doing",
        entity_type: "column",
        moniker: "column:col-doing",
        fields: { name: "Doing", color: "6366f1" },
      },
    ],
  };
  mockMentionableTypes = [
    { entityType: "column", prefix: "%", displayField: "name" },
  ];
}

/** The canonical `position_column` field definition (reference to column). */
const positionColumnField: FieldDef = {
  id: "0000000000000000000000000H",
  name: "position_column",
  type: { kind: "reference", entity: "column", multiple: false },
} as unknown as FieldDef;

describe("BadgeDisplay", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockInvoke.mockResolvedValue(undefined);
    mockEntities = {};
    mockMentionableTypes = [];
  });

  it("renders the column's display name via the CM6 mention widget", async () => {
    setupColumnFixtures();

    const { container } = render(
      <Providers>
        <BadgeDisplay
          field={positionColumnField}
          value="col-doing"
          entity={taskEntity}
          mode="full"
        />
      </Providers>,
    );
    await flush();

    // The CM6 mention widget renders the resolved display name with
    // the column prefix.
    const widget = container.querySelector(".cm-mention-pill");
    expect(widget).toBeTruthy();
    expect(widget?.textContent).toBe("%Doing");
  });

  it("wraps the TextViewer in a FocusScope bearing the column moniker", async () => {
    setupColumnFixtures();

    const { container } = render(
      <Providers>
        <BadgeDisplay
          field={positionColumnField}
          value="col-doing"
          entity={taskEntity}
          mode="full"
        />
      </Providers>,
    );
    await flush();

    const scope = container.querySelector("[data-moniker='column:col-doing']");
    expect(scope).toBeTruthy();
  });

  it("falls back to raw id with muted mark styling when the column is missing", async () => {
    mockEntities = { column: [] };
    mockMentionableTypes = [
      { entityType: "column", prefix: "%", displayField: "name" },
    ];

    const { container } = render(
      <Providers>
        <BadgeDisplay
          field={positionColumnField}
          value="missing-col"
          entity={taskEntity}
          mode="full"
        />
      </Providers>,
    );
    await flush();

    // No CM6 widget — widget needs a metaMap entry with a valid color.
    const widget = container.querySelector(".cm-mention-pill");
    expect(widget).toBeFalsy();

    // Muted-mark fallback carries the raw slug prefixed with the mention char.
    const mark = container.querySelector(".cm-column-pill");
    expect(mark).toBeTruthy();
    expect(mark?.textContent).toBe("%missing-col");
  });

  it("renders a dash when the value is empty", () => {
    setupColumnFixtures();

    const { container } = render(
      <Providers>
        <BadgeDisplay
          field={positionColumnField}
          value=""
          entity={taskEntity}
          mode="full"
        />
      </Providers>,
    );

    // Plain muted dash — same empty-state visual as the previous implementation.
    const dash = container.querySelector("span.text-muted-foreground\\/50");
    expect(dash).toBeTruthy();
    expect(dash?.textContent).toBe("-");

    // No CM6 pill rendered in the empty state.
    expect(container.querySelector(".cm-mention-pill")).toBeFalsy();
  });

  it("renders the configured placeholder when value is missing or empty string", () => {
    setupColumnFixtures();

    const fieldWithPlaceholder: FieldDef = {
      ...positionColumnField,
      placeholder: "Assign a project",
    } as unknown as FieldDef;

    // Empty string → placeholder renders in the muted span.
    const { container: emptyContainer } = render(
      <Providers>
        <BadgeDisplay
          field={fieldWithPlaceholder}
          value=""
          entity={taskEntity}
          mode="full"
        />
      </Providers>,
    );
    const emptyHint = emptyContainer.querySelector(
      "span.text-muted-foreground\\/50",
    );
    expect(emptyHint).toBeTruthy();
    expect(emptyHint?.textContent).toBe("Assign a project");
    expect(emptyContainer.querySelector(".cm-mention-pill")).toBeFalsy();

    // Non-string value (defensive — null) → same placeholder path.
    const { container: nullContainer } = render(
      <Providers>
        <BadgeDisplay
          field={fieldWithPlaceholder}
          value={null}
          entity={taskEntity}
          mode="full"
        />
      </Providers>,
    );
    const nullHint = nullContainer.querySelector(
      "span.text-muted-foreground\\/50",
    );
    expect(nullHint).toBeTruthy();
    expect(nullHint?.textContent).toBe("Assign a project");
  });

  it("renders a dash when the value is a non-string (defensive)", () => {
    setupColumnFixtures();

    const { container } = render(
      <Providers>
        <BadgeDisplay
          field={positionColumnField}
          value={null}
          entity={taskEntity}
          mode="full"
        />
      </Providers>,
    );

    const dash = container.querySelector("span.text-muted-foreground\\/50");
    expect(dash).toBeTruthy();
    expect(dash?.textContent).toBe("-");
  });

  it("falls back to a plain span when field.type.entity is unset", () => {
    // Defensive guard path — no shipping field has this shape, but the
    // component must degrade gracefully instead of crashing.
    mockEntities = {};
    mockMentionableTypes = [];

    const fieldWithoutEntity: FieldDef = {
      id: "00000000000000000000000099",
      name: "legacy_badge",
      type: { kind: "select" },
    } as unknown as FieldDef;

    const { container } = render(
      <Providers>
        <BadgeDisplay
          field={fieldWithoutEntity}
          value="raw-value"
          entity={taskEntity}
          mode="full"
        />
      </Providers>,
    );

    // No CM6 widget — just a plain text span with the raw value.
    expect(container.querySelector(".cm-mention-pill")).toBeFalsy();
    expect(container.textContent).toContain("raw-value");
  });
});
