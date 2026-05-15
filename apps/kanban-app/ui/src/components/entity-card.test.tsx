import { describe, it, expect, vi, beforeEach } from "vitest";
import {
  render,
  screen,
  fireEvent,
  act,
  waitFor,
} from "@testing-library/react";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const TASK_SCHEMA = {
  entity: {
    name: "task",
    body_field: "body",
    fields: ["title", "tags", "progress", "body"],
    commands: [
      {
        id: "ui.inspect",
        name: "Inspect {{entity.type}}",
        context_menu: true,
      },
    ],
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
    {
      id: "f3",
      name: "tags",
      type: { kind: "computed", derive: "parse-body-tags" },
      editor: "multi-select",
      display: "badge-list",
      icon: "tag",
      description: "Task tags",
      section: "header",
    },
    {
      id: "f4",
      name: "progress",
      type: { kind: "computed", derive: "parse-body-progress" },
      editor: "none",
      display: "progress",
      icon: "clock",
      section: "header",
    },
    {
      id: "f2",
      name: "body",
      type: { kind: "markdown", single_line: false },
      editor: "markdown",
      display: "markdown",
      section: "body",
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
  if (args[0] === "update_entity_field")
    return Promise.resolve({ id: "task-1" });
  if (args[0] === "list_commands_for_scope")
    return Promise.resolve([
      {
        id: "ui.inspect",
        name: "Inspect task",
        target: "task:task-1",
        group: "entity",
        context_menu: true,
        available: true,
      },
    ]);
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
import { EntityCard } from "./entity-card";
import { UIStateProvider } from "@/lib/ui-state-context";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { FieldUpdateProvider } from "@/lib/field-update-context";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { FocusLayer } from "@/components/focus-layer";
import { asSegment } from "@/types/spatial";

import { TooltipProvider } from "@/components/ui/tooltip";
import type { Entity } from "@/types/kanban";

/** Create a task Entity with sensible defaults and optional field overrides. */
function makeEntity(fieldOverrides: Record<string, unknown> = {}): Entity {
  return {
    entity_type: "task",
    id: "task-1",
    moniker: "task:task-1",
    fields: {
      title: "Hello **world**",
      body: "",
      tags: [],
      assignees: [],
      depends_on: [],
      position_column: "col-1",
      position_ordinal: "a0",
      ...fieldOverrides,
    },
  };
}

/** Track the current entity so the store can find it via useFieldValue. */
let currentEntity: Entity = makeEntity();

function renderCard(ui: React.ReactElement) {
  return render(
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <TooltipProvider delayDuration={0}>
          <SchemaProvider>
            <EntityStoreProvider entities={{ task: [currentEntity], tag: [] }}>
              <EntityFocusProvider>
                <FieldUpdateProvider>
                  <UIStateProvider>{ui}</UIStateProvider>
                </FieldUpdateProvider>
              </EntityFocusProvider>
            </EntityStoreProvider>
          </SchemaProvider>
        </TooltipProvider>
      </FocusLayer>
    </SpatialFocusProvider>,
  );
}

/** Render and wait for schema to load */
async function renderWithProvider(ui: React.ReactElement) {
  const result = renderCard(ui);
  await act(async () => {
    await new Promise((r) => setTimeout(r, 100));
  });
  return result;
}

describe("EntityCard", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
  });

  it("renders title as text via Field display", async () => {
    currentEntity = makeEntity();
    await renderWithProvider(<EntityCard entity={currentEntity} />);
    // TextDisplay renders plain text (display: "text"), not markdown
    expect(screen.getByText("Hello **world**")).toBeTruthy();
  });

  it("(i) button dispatches ui.inspect with explicit target moniker", async () => {
    currentEntity = makeEntity();
    const { container } = await renderWithProvider(
      <EntityCard entity={currentEntity} />,
    );
    mockInvoke.mockClear();
    const inspectBtn = container.querySelector("button[aria-label='Inspect']")!;
    await act(async () => {
      fireEvent.click(inspectBtn);
    });
    const inspectCall = mockInvoke.mock.calls.find(
      (c: unknown[]) =>
        c[0] === "dispatch_command" &&
        (c[1] as Record<string, unknown>)?.cmd === "ui.inspect",
    );
    expect(inspectCall).toBeTruthy();
    // Target must be passed explicitly so the backend uses ctx.target
    // instead of walking the scope chain (which depends on focus state).
    const params = inspectCall![1] as Record<string, unknown>;
    expect(params.target).toBe("task:task-1");
  });

  it("(i) button always renders", async () => {
    currentEntity = makeEntity();
    const { container } = await renderWithProvider(
      <EntityCard entity={currentEntity} />,
    );
    expect(
      container.querySelector("button[aria-label='Inspect']"),
    ).not.toBeNull();
  });

  it("enters edit mode when title is clicked", async () => {
    currentEntity = makeEntity();
    const { container } = await renderWithProvider(
      <EntityCard entity={currentEntity} />,
    );
    const titleEl = screen.getByText("Hello **world**");
    fireEvent.click(titleEl);
    expect(container.querySelector(".cm-editor")).toBeTruthy();
  });

  it("saving edited title calls dispatch_command with correct params", async () => {
    mockInvoke.mockClear();
    currentEntity = makeEntity({ title: "bug" });
    const { container } = await renderWithProvider(
      <EntityCard entity={currentEntity} />,
    );

    // Click to enter edit mode
    const titleEl = screen.getByText("bug");
    fireEvent.click(titleEl);
    const cmEditor = container.querySelector(".cm-editor") as HTMLElement;
    expect(cmEditor).toBeTruthy();

    // Get CM6 EditorView and replace doc text
    const { EditorView } = await import("@codemirror/view");
    const view = EditorView.findFromDOM(cmEditor);
    if (!view) return; // jsdom limitation — skip gracefully

    await act(async () => {
      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: "defect" },
      });
    });

    // CM6 manages focus internally. Call blur() on the contenteditable
    // element so CM6's DOMObserver detects the focus loss.
    const cmContent = container.querySelector(".cm-content") as HTMLElement;
    await act(async () => {
      cmContent.blur();
      // CM6's DOMObserver polls focus state — give it a tick
      await new Promise((r) => setTimeout(r, 50));
    });

    await waitFor(() => {
      const call = mockInvoke.mock.calls.find(
        (c) =>
          c[0] === "dispatch_command" &&
          (c[1] as Record<string, unknown>)?.cmd === "entity.update_field",
      );
      expect(call).toBeTruthy();
      expect(call![1]).toMatchObject({
        cmd: "entity.update_field",
        args: {
          entity_type: "task",
          id: "task-1",
          field_name: "title",
          value: "defect",
        },
      });
    });
  });

  it("entity.inspect command includes target moniker in context menu", async () => {
    currentEntity = makeEntity();
    const { container } = await renderWithProvider(
      <EntityCard entity={currentEntity} />,
    );
    const card = container.querySelector("[data-segment='task:task-1']")!;
    await act(async () => {
      fireEvent.contextMenu(card);
      // Flush the promise chain (list_commands_for_scope → show_context_menu)
      await new Promise((r) => setTimeout(r, 50));
    });
    // Context menu items carry cmd + target as separate fields
    const ctxCall = mockInvoke.mock.calls.find(
      (c) => c[0] === "show_context_menu",
    );
    expect(ctxCall).toBeTruthy();
    const items = ctxCall![1].items as {
      cmd: string;
      target?: string;
      name: string;
    }[];
    expect(
      items.find((i) => i.cmd === "ui.inspect" && i.target === "task:task-1"),
    ).toBeTruthy();
  });

  it("clicking card body does not trigger inspect", async () => {
    currentEntity = makeEntity();
    const { container } = await renderWithProvider(
      <EntityCard entity={currentEntity} />,
    );
    mockInvoke.mockClear();
    const card = container.querySelector(".rounded-md")!;
    fireEvent.click(card);
    // Click on card body should not dispatch ui.inspect — only the (i) button does
    const inspectCall = mockInvoke.mock.calls.find(
      (c: unknown[]) =>
        c[0] === "dispatch_command" &&
        (c[1] as Record<string, unknown>)?.cmd === "ui.inspect",
    );
    expect(inspectCall).toBeUndefined();
  });

  describe("field icon tooltips", () => {
    // After card 01KQAWV9C5F8Y3AA0KDDHHRRN1, card fields render through
    // `<Field withIcon />` (matching the inspector path). The icon badge is
    // `<FieldIconBadge>` rendered *inside* the field's `<FocusScope>` — its
    // outer span carries `data-slot="tooltip-trigger"` (set by Radix's
    // `<TooltipTrigger asChild>`) and the description text lives in the
    // separately-mounted `<TooltipContent>` rather than as `aria-label` on
    // the trigger span. Tests below are restated in terms of the new
    // shared shape.
    it("renders an icon badge whose tooltip body is the field's static description", async () => {
      currentEntity = makeEntity();
      const { container } = await renderWithProvider(
        <EntityCard entity={currentEntity} />,
      );
      // The tags field has icon=tag and description="Task tags" — the
      // `<FieldIconBadge>` trigger lives inside the field zone wrapper, and
      // hovering it must surface the description text via Radix's
      // `<TooltipContent>`. Pins the static-description path
      // (`field.description` → tooltip body) inside `resolveFieldIconAndTip`.
      const fieldZone = container.querySelector(
        '[data-segment="field:task:task-1.tags"]',
      ) as HTMLElement | null;
      expect(fieldZone).toBeTruthy();
      const trigger = fieldZone!.querySelector(
        'span[data-slot="tooltip-trigger"]',
      ) as HTMLElement | null;
      expect(trigger).toBeTruthy();

      await act(async () => {
        fireEvent.pointerEnter(trigger!);
        fireEvent.focus(trigger!);
        await new Promise((r) => setTimeout(r, 50));
      });

      // Radix renders tooltip content into a portal off the document body.
      const allText = document.body.textContent ?? "";
      expect(
        allText.includes("Task tags"),
        `tooltip body should include the field description "Task tags". document text: ${allText}`,
      ).toBe(true);
    });

    it("falls back to the humanized field name when the field has no description (e.g. progress)", async () => {
      currentEntity = makeEntity();
      const { container } = await renderWithProvider(
        <EntityCard entity={currentEntity} />,
      );
      // The progress field has icon=clock but no `description` — the
      // tooltip body must fall back to the humanized field name
      // (`field.name.replace(/_/g, " ")` → "progress"). Pins the fallback
      // branch in `resolveFieldIconAndTip`.
      const fieldZone = container.querySelector(
        '[data-segment="field:task:task-1.progress"]',
      ) as HTMLElement | null;
      expect(fieldZone).toBeTruthy();
      const trigger = fieldZone!.querySelector(
        'span[data-slot="tooltip-trigger"]',
      ) as HTMLElement | null;
      expect(trigger).toBeTruthy();

      await act(async () => {
        fireEvent.pointerEnter(trigger!);
        fireEvent.focus(trigger!);
        await new Promise((r) => setTimeout(r, 50));
      });

      const allText = document.body.textContent ?? "";
      expect(
        allText.includes("progress"),
        `tooltip body should include the humanized field name "progress". document text: ${allText}`,
      ).toBe(true);
    });

    it("does not render an icon badge for fields without an icon", async () => {
      currentEntity = makeEntity();
      const { container } = await renderWithProvider(
        <EntityCard entity={currentEntity} />,
      );
      // The title field has no icon — its field zone must not contain a
      // tooltip-trigger badge.
      const fieldZone = container.querySelector(
        '[data-segment="field:task:task-1.title"]',
      ) as HTMLElement | null;
      expect(fieldZone).toBeTruthy();
      expect(
        fieldZone!.querySelector('span[data-slot="tooltip-trigger"]'),
      ).toBeNull();
    });
  });

  describe("declarative on_card sections", () => {
    // Schema declaring three sections: header and dates are on_card, body is not.
    const SECTIONED_SCHEMA = {
      entity: {
        name: "task",
        body_field: "body",
        fields: ["title", "body", "due", "scheduled"],
        sections: [
          { id: "header", on_card: true },
          { id: "body" },
          { id: "dates", label: "Dates", on_card: true },
        ],
        commands: [
          {
            id: "ui.inspect",
            name: "Inspect {{entity.type}}",
            context_menu: true,
          },
        ],
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
        {
          id: "f2",
          name: "body",
          type: { kind: "markdown", single_line: false },
          editor: "markdown",
          display: "markdown",
          section: "body",
        },
        {
          id: "f3",
          name: "due",
          type: { kind: "date" },
          editor: "date",
          display: "date",
          icon: "calendar",
          section: "dates",
        },
        {
          id: "f4",
          name: "scheduled",
          type: { kind: "date" },
          editor: "date",
          display: "date",
          icon: "calendar-clock",
          section: "dates",
        },
      ],
    };

    it("renders on_card sections below header with a divider; non-on_card sections stay off", async () => {
      // Swap the schema mock for this test so the card uses sections. The
      // `any` cast matches the original mockInvoke's signature, which infers
      // a return-type union from its declaration site — our sectioned schema
      // shape isn't part of that union by construction, so we widen here.
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      mockInvoke.mockImplementation(((...args: any[]) => {
        if (args[0] === "list_entity_types") return Promise.resolve(["task"]);
        if (args[0] === "get_entity_schema")
          return Promise.resolve(SECTIONED_SCHEMA);
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
        if (args[0] === "update_entity_field")
          return Promise.resolve({ id: "task-1" });
        if (args[0] === "list_commands_for_scope") return Promise.resolve([]);
        if (args[0] === "show_context_menu") return Promise.resolve();
        return Promise.resolve("ok");
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
      }) as any);

      currentEntity = makeEntity({
        title: "Sectioned task",
        body: "Body text should NOT render on card",
        due: "2026-05-01",
        scheduled: "2026-04-20",
      });
      const { container } = await renderWithProvider(
        <EntityCard entity={currentEntity} />,
      );

      // Header section renders with its title field.
      const headerSection = container.querySelector(
        '[data-testid="card-section-header"]',
      );
      expect(headerSection).toBeTruthy();
      expect(screen.getByText("Sectioned task")).toBeTruthy();

      // Dates section renders below header and contains both date fields.
      const datesSection = container.querySelector(
        '[data-testid="card-section-dates"]',
      );
      expect(datesSection).toBeTruthy();

      // Body section (on_card: false) does NOT render on the card.
      expect(
        container.querySelector('[data-testid="card-section-body"]'),
      ).toBeNull();
      // Body text should not appear in the card.
      expect(
        screen.queryByText("Body text should NOT render on card"),
      ).toBeNull();

      // A divider sits between header and dates (only one divider since only
      // two on_card sections render).
      const dividers = container.querySelectorAll(
        "div.my-1\\.5.h-px.bg-border\\/50",
      );
      expect(dividers.length).toBe(1);

      // Cards never render section labels (labels belong to the inspector).
      expect(
        container.querySelector(
          '[data-testid="inspector-section-label-dates"]',
        ),
      ).toBeNull();
    });

    it("when entity declares no sections, only the header section renders (backcompat)", async () => {
      // TASK_SCHEMA at module scope has no `sections` key — restore that mock.
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      mockInvoke.mockImplementation(((...args: any[]) => {
        if (args[0] === "list_entity_types") return Promise.resolve(["task"]);
        if (args[0] === "get_entity_schema")
          return Promise.resolve(TASK_SCHEMA);
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
        if (args[0] === "update_entity_field")
          return Promise.resolve({ id: "task-1" });
        if (args[0] === "list_commands_for_scope") return Promise.resolve([]);
        if (args[0] === "show_context_menu") return Promise.resolve();
        return Promise.resolve("ok");
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
      }) as any);
      currentEntity = makeEntity();
      const { container } = await renderWithProvider(
        <EntityCard entity={currentEntity} />,
      );
      expect(
        container.querySelector('[data-testid="card-section-header"]'),
      ).toBeTruthy();
      // No other card-section-* elements should exist.
      const otherSections = Array.from(
        container.querySelectorAll("[data-testid^='card-section-']"),
      ).filter(
        (el) => el.getAttribute("data-testid") !== "card-section-header",
      );
      expect(otherSections.length).toBe(0);
    });
  });

  describe("progress bar", () => {
    it("shows progress bar when progress field has items", async () => {
      currentEntity = makeEntity({
        progress: { total: 3, completed: 1, percent: 33 },
      });
      const { container } = await renderWithProvider(
        <EntityCard entity={currentEntity} />,
      );
      const progressBar = container.querySelector('[role="progressbar"]');
      expect(progressBar).toBeTruthy();
      expect(progressBar!.getAttribute("aria-valuenow")).toBe("33");
    });

    it("shows 0% progress when no items are completed", async () => {
      currentEntity = makeEntity({
        progress: { total: 2, completed: 0, percent: 0 },
      });
      const { container } = await renderWithProvider(
        <EntityCard entity={currentEntity} />,
      );
      const progressBar = container.querySelector('[role="progressbar"]');
      expect(progressBar).toBeTruthy();
      expect(progressBar!.getAttribute("aria-valuenow")).toBe("0");
      expect(container.textContent).toContain("0%");
    });

    it("shows 100% progress when all items are completed", async () => {
      currentEntity = makeEntity({
        progress: { total: 2, completed: 2, percent: 100 },
      });
      const { container } = await renderWithProvider(
        <EntityCard entity={currentEntity} />,
      );
      const progressBar = container.querySelector('[role="progressbar"]');
      expect(progressBar).toBeTruthy();
      expect(progressBar!.getAttribute("aria-valuenow")).toBe("100");
    });

    it("does not show progress bar when total is zero", async () => {
      currentEntity = makeEntity({
        progress: { total: 0, completed: 0, percent: 0 },
      });
      const { container } = await renderWithProvider(
        <EntityCard entity={currentEntity} />,
      );
      const progressBar = container.querySelector('[role="progressbar"]');
      expect(progressBar).toBeNull();
    });

    it("does not show progress bar when progress field is null", async () => {
      currentEntity = makeEntity();
      const { container } = await renderWithProvider(
        <EntityCard entity={currentEntity} />,
      );
      const progressBar = container.querySelector('[role="progressbar"]');
      expect(progressBar).toBeNull();
    });
  });

  /**
   * Tests that mount the card inside the full spatial-focus stack so the
   * underlying `<FocusScope>` primitive registers with the Rust-side
   * spatial graph via the mocked `invoke`.
   *
   * The card body is a `<FocusScope>` (post-card-`01KQJDYJ4SDKK2G8FTAQ348ZHG`),
   * NOT a `<FocusScope>`. The card holds multiple focusable atoms (drag
   * handle, the `<Field>` rows with their own zones and pill leaves,
   * inspect button) and so is a zone by the kernel's three-peer
   * contract. The previous Scope shape silently broke the spatial
   * topology — Field zones rendered inside the card had `parent_zone`
   * skip the Scope (because Scopes don't push `FocusScopeContext`) and
   * point at the column zone instead, so the kernel saw fields as
   * siblings of cards rather than descendants. The path-prefix branch
   * of the kernel's scope-is-leaf invariant catches that shape directly
   * — see `swissarmyhammer-focus/tests/scope_is_leaf.rs`.
   */
  describe("spatial registration as a FocusScope", () => {
    /** Render with the full spatial-focus + focus-layer stack. */
    function renderCardWithSpatial(ui: React.ReactElement) {
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

    async function renderWithSpatial(ui: React.ReactElement) {
      const result = renderCardWithSpatial(ui);
      await act(async () => {
        await new Promise((r) => setTimeout(r, 100));
      });
      return result;
    }

    it("registers the card body as a FocusScope with the entity moniker", async () => {
      currentEntity = makeEntity();
      await renderWithSpatial(<EntityCard entity={currentEntity} />);
      const zoneCalls = mockInvoke.mock.calls
        .filter((c) => c[0] === "spatial_register_scope")
        .map((c) => c[1] as Record<string, unknown>);
      expect(zoneCalls.find((a) => a.segment === "task:task-1")).toBeTruthy();
    });

    // Pre-collapse this file pinned that the card root registered via
    // the legacy zone command but NOT via the leaf-scope command. After
    // parent task `01KQSDP4ZJY5ERAJ68TFPVFRRE` unified the primitives
    // into a single `<FocusScope>` driven by `spatial_register_scope`,
    // the negative half of that pair became vacuous — there is no
    // second command to be absent on. The positive half (the card body
    // registers with `task:task-1`) is covered by the preceding test.

    it("the card zone's parent_zone follows the enclosing FocusScope (null when none, here the layer root)", async () => {
      // The card is a zone (`<FocusScope>`). Its own `parentZone` is
      // whatever `useParentFocusScope()` returns at the call site. In this
      // isolated harness there is no surrounding `<FocusScope>`, so the
      // card's parent zone is null. In production the card is wrapped
      // by a `column:` `<FocusScope>` and that zone's spatial key flows
      // through here — pinning the column-as-parent contract that the
      // unified cascade's iter-1 escalation relies on (iter 1 reads the
      // card's `parentZone` to find the neighbouring column zone for
      // cross-column nav).
      currentEntity = makeEntity();
      await renderWithSpatial(<EntityCard entity={currentEntity} />);
      const cardZone = mockInvoke.mock.calls
        .filter((c) => c[0] === "spatial_register_scope")
        .map((c) => c[1] as Record<string, unknown>)
        .find((a) => a.segment === "task:task-1");
      expect(cardZone).toBeTruthy();
      expect(cardZone!.parentZone).toBeNull();
      // Anchored to the window layer the FocusLayer wrapper provides.
      expect(cardZone!.layerFq).toBeTruthy();
    });

    it("clicking the card body invokes spatial_focus and does not dispatch ui.inspect directly", async () => {
      currentEntity = makeEntity();
      const { container } = await renderWithSpatial(
        <EntityCard entity={currentEntity} />,
      );
      mockInvoke.mockClear();
      const card = container.querySelector("[data-segment='task:task-1']")!;
      fireEvent.click(card);
      // The primitive's click handler routes through `spatial_focus`.
      const focusCall = mockInvoke.mock.calls.find(
        (c) => c[0] === "spatial_focus",
      );
      expect(focusCall).toBeTruthy();
      // Inspect is now a separate Space-bound command at app level —
      // a bare card click must not dispatch ui.inspect.
      const inspectCall = mockInvoke.mock.calls.find(
        (c: unknown[]) =>
          c[0] === "dispatch_command" &&
          (c[1] as Record<string, unknown>)?.cmd === "ui.inspect",
      );
      expect(inspectCall).toBeUndefined();
    });
  });
});
