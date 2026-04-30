/**
 * Spatial-nav integration tests for `<Field>`.
 *
 * Pins the contract from card `01KQ5QB6F4MTD35GBTARJH4JEW`: `<Field>` is
 * a `<FocusZone>` keyed by `field:{type}:{id}.{name}` whose internal
 * children are leaves of mode-appropriate shape. Edit mode replaces the
 * zone with the bare editor (which takes DOM focus directly).
 *
 * Mounts `<Field>` inside the production spatial provider stack so the
 * conditional `<FocusZone>` body lights up its
 * `spatial_register_zone`-emitting branch. The Tauri `invoke` boundary
 * is mocked at the module level so we can inspect the registration
 * calls. Companion tests in `field.test.tsx` cover the
 * registration-side of the display registry; this file pins the
 * runtime contract that consumers depend on.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, act } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Tauri API mocks — must come before component imports.
// ---------------------------------------------------------------------------

const mockInvoke = vi.hoisted(() =>
  vi.fn(async (_cmd: string, _args?: unknown): Promise<unknown> => undefined),
);
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...a: unknown[]) => mockInvoke(...(a as [string, unknown?])),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
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

// ---------------------------------------------------------------------------
// Imports come after mocks.
// ---------------------------------------------------------------------------

import "@/components/fields/registrations";
import { Field } from "@/components/fields/field";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { FocusLayer } from "@/components/focus-layer";
import { FieldUpdateProvider } from "@/lib/field-update-context";
import { UIStateProvider } from "@/lib/ui-state-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import { CommandBusyProvider } from "@/lib/command-scope";
import {
  asSegment
} from "@/types/spatial";
import type { Entity, FieldDef } from "@/types/kanban";

const TITLE_FIELD: FieldDef = {
  id: "f-title",
  name: "title",
  type: { kind: "markdown", single_line: true },
  editor: "markdown",
  display: "text",
  section: "header",
};

// Reference field — `type.entity = "tag"` is what BadgeListDisplay reads to
// route resolution through the tag schema. `commit_display_names` flips it
// into "values are slugs" mode so a value like ["bug", "ui"] becomes two
// pills regardless of whether the tag entities exist in the store.
const TAGS_FIELD: FieldDef = {
  id: "f-tags",
  name: "tags",
  type: {
    kind: "reference",
    entity: "tag",
    multiple: true,
    commit_display_names: true,
  },
  editor: "multi-select",
  display: "badge-list",
  icon: "tag",
  section: "header",
};

// The tag schema is needed so MentionView can resolve `tag:bug` etc. Without
// it the resolved entity is undefined but `monikerId` falls back to the
// slug, so pills still render with the expected `tag:{slug}` moniker.
const TAG_SCHEMA = {
  entity: {
    name: "tag",
    fields: ["tag_name", "color"],
    mention_prefix: "#",
    mention_display_field: "tag_name",
  },
  fields: [
    {
      id: "t1",
      name: "tag_name",
      type: { kind: "text", single_line: true },
      editor: "markdown",
      display: "text",
      icon: "tag",
      section: "header",
    },
  ],
};

function makeTask(fields: Record<string, unknown>): Entity {
  return {
    entity_type: "task",
    id: "t1",
    moniker: "task:t1",
    fields,
  };
}

/**
 * Render a `<Field>` inside the production-shaped provider stack with
 * the spatial-nav layer present so the FocusZone body lights up.
 */
function FieldHarness({
  field,
  entity,
  editing = false,
  handleEvents,
}: {
  field: FieldDef;
  entity: Entity;
  editing?: boolean;
  handleEvents?: boolean;
}) {
  return (
    <CommandBusyProvider>
      <SpatialFocusProvider>
        <FocusLayer name={asSegment("window")}>
          <TooltipProvider>
            <SchemaProvider>
              <EntityStoreProvider entities={{ task: [entity] }}>
                <EntityFocusProvider>
                  <FieldUpdateProvider>
                    <UIStateProvider>
                      <Field
                        fieldDef={field}
                        entityType={entity.entity_type}
                        entityId={entity.id}
                        mode="full"
                        editing={editing}
                        handleEvents={handleEvents}
                      />
                    </UIStateProvider>
                  </FieldUpdateProvider>
                </EntityFocusProvider>
              </EntityStoreProvider>
            </SchemaProvider>
          </TooltipProvider>
        </FocusLayer>
      </SpatialFocusProvider>
    </CommandBusyProvider>
  );
}

const TASK_SCHEMA = {
  entity: {
    name: "task",
    entity_type: "task",
    search_display_field: "title",
  },
  fields: [TITLE_FIELD, TAGS_FIELD],
};

const SCHEMAS: Record<string, unknown> = {
  task: TASK_SCHEMA,
  tag: TAG_SCHEMA,
};

/** Default invoke responses for the mount-time IPCs the providers fire. */
async function defaultInvokeImpl(
  cmd: string,
  args?: unknown,
): Promise<unknown> {
  if (cmd === "list_entity_types") return ["task", "tag"];
  if (cmd === "get_entity_schema") {
    const entityType = (args as { entityType?: string })?.entityType;
    return SCHEMAS[entityType ?? ""] ?? TASK_SCHEMA;
  }
  if (cmd === "get_ui_state")
    return {
      palette_open: false,
      palette_mode: "command",
      keymap_mode: "cua",
      scope_chain: [],
      open_boards: [],
      windows: {},
      recent_boards: [],
    };
  if (cmd === "get_undo_state") return { can_undo: false, can_redo: false };
  if (cmd === "dispatch_command") return undefined;
  if (cmd === "list_commands_for_scope") return [];
  return undefined;
}

/** Collect every `spatial_register_zone` payload. */
function registerZoneCalls(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_register_zone")
    .map((c) => c[1] as Record<string, unknown>);
}

/** Wait two ticks so mount-time effects flush before assertions. */
async function flushSetup() {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 50));
  });
}

describe("Field (spatial-nav)", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("registers a FocusZone with moniker field:{type}:{id}.{name} on mount", async () => {
    let result!: ReturnType<typeof render>;
    await act(async () => {
      result = render(
        <FieldHarness
          field={TITLE_FIELD}
          entity={makeTask({ title: "Hello" })}
        />,
      );
    });
    await flushSetup();

    const zones = registerZoneCalls().filter(
      (c) => c.segment === "field:task:t1.title",
    );
    expect(zones).toHaveLength(1);

    // The DOM exposes the moniker via data-moniker for e2e selectors.
    const node = result.container.querySelector(
      "[data-segment='field:task:t1.title']",
    );
    expect(node).not.toBeNull();
  });

  it("keeps the FocusZone wrap in edit mode so spatial focus stays on the field moniker", async () => {
    // The editor element takes DOM focus via its own ref-driven `.focus()`
    // call; the surrounding zone marks the moniker without interfering
    // because its click handler short-circuits on `INPUT/TEXTAREA/SELECT`
    // and `[contenteditable]` targets. Spatial focus stays at the field-
    // zone moniker while the user types — leaving and re-entering edit
    // mode does not lose the zone's identity in the DOM.
    let result!: ReturnType<typeof render>;
    await act(async () => {
      result = render(
        <FieldHarness
          field={TITLE_FIELD}
          entity={makeTask({ title: "Hello" })}
          editing={true}
        />,
      );
    });
    await flushSetup();

    const zones = registerZoneCalls().filter(
      (c) => c.segment === "field:task:t1.title",
    );
    expect(zones).toHaveLength(1);

    // The DOM exposes the moniker via data-moniker for e2e selectors,
    // even in edit mode.
    const node = result.container.querySelector(
      "[data-segment='field:task:t1.title']",
    );
    expect(node).not.toBeNull();
  });

  it("badge-list field renders one FocusScope leaf per pill, nested under the field zone", async () => {
    let result!: ReturnType<typeof render>;
    await act(async () => {
      result = render(
        <FieldHarness
          field={TAGS_FIELD}
          entity={makeTask({ tags: ["bug", "ui"] })}
        />,
      );
    });
    await flushSetup();

    // The field zone is registered.
    const zoneNode = result.container.querySelector(
      "[data-segment='field:task:t1.tags']",
    );
    expect(zoneNode).not.toBeNull();

    // Each pill is a `<FocusScope>` leaf nested inside the field zone.
    // BadgeList renders MentionView, which wraps each item in FocusScope
    // with `tag:{slug}` monikers.
    const pills = zoneNode!.querySelectorAll('[data-segment^="tag:"]');
    expect(pills.length).toBe(2);
    const monikers = Array.from(pills).map((p) =>
      p.getAttribute("data-moniker"),
    );
    expect(monikers).toEqual(["tag:bug", "tag:ui"]);
  });

  it("handleEvents={false} skips FocusZone click ownership (cell parent keeps the click)", async () => {
    // Smoke test — when handleEvents is false the zone still registers
    // (so command-scope chrome works) but its outer `<div>` carries no
    // click handler that would `e.stopPropagation()`. We assert the
    // zone is registered and rely on the existing `<FocusZone>` test
    // suite for the click-handling behaviour.
    await act(async () => {
      render(
        <FieldHarness
          field={TITLE_FIELD}
          entity={makeTask({ title: "Hello" })}
          handleEvents={false}
        />,
      );
    });
    await flushSetup();

    const zones = registerZoneCalls().filter(
      (c) => c.segment === "field:task:t1.title",
    );
    expect(zones).toHaveLength(1);
  });
});
