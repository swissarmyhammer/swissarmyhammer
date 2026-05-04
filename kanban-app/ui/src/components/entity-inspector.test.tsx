import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";

// Hoisted mocks: capture invoke and listen so the kernel simulator can drive
// `focus-changed` events through the production spatial-focus bridge.
type ListenCallback = (event: { payload: unknown }) => void;
const { mockInvoke, mockListen, listeners } = vi.hoisted(() => {
  const listeners = new Map<string, ListenCallback[]>();
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const mockInvoke = vi.fn(async (..._args: any[]): Promise<unknown> => undefined);
  const mockListen = vi.fn(
    (eventName: string, cb: ListenCallback): Promise<() => void> => {
      const cbs = listeners.get(eventName) ?? [];
      cbs.push(cb);
      listeners.set(eventName, cbs);
      return Promise.resolve(() => {
        const arr = listeners.get(eventName);
        if (arr) {
          const idx = arr.indexOf(cb);
          if (idx >= 0) arr.splice(idx, 1);
        }
      });
    },
  );
  return { mockInvoke, mockListen, listeners };
});

// Schema with sections matching the new YAML definitions
const TASK_SCHEMA = {
  entity: {
    name: "task",
    body_field: "body",
    fields: [
      "title",
      "tags",
      "progress",
      "body",
      "assignees",
      "depends_on",
      "position_column",
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
      section: "header",
    },
    {
      id: "f4",
      name: "progress",
      type: { kind: "computed", derive: "parse-body-progress" },
      editor: "none",
      display: "progress",
      icon: "bar-chart",
      section: "header",
    },
    {
      id: "f2",
      name: "body",
      type: { kind: "markdown", single_line: false },
      editor: "markdown",
      display: "markdown",
      icon: "file-text",
      section: "body",
    },
    {
      id: "f5",
      name: "assignees",
      type: { kind: "reference", entity: "actor", multiple: true },
      editor: "multi-select",
      display: "avatar",
      icon: "users",
      section: "body",
    },
    {
      id: "f7",
      name: "depends_on",
      type: { kind: "reference", entity: "task", multiple: true },
      editor: "multi-select",
      display: "badge-list",
      icon: "workflow",
      section: "body",
    },
    {
      id: "f8",
      name: "position_column",
      type: { kind: "reference", entity: "column", multiple: false },
      editor: "select",
      display: "badge",
      section: "hidden",
    },
  ],
};

const TAG_SCHEMA = {
  entity: {
    name: "tag",
    fields: ["tag_name", "color", "description"],
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
    {
      id: "t2",
      name: "color",
      type: { kind: "color" },
      editor: "color-palette",
      display: "color-swatch",
      icon: "palette",
      section: "body",
    },
    {
      id: "t3",
      name: "description",
      type: { kind: "markdown" },
      editor: "markdown",
      display: "markdown",
      icon: "align-left",
      section: "body",
    },
  ],
};

const ACTOR_SCHEMA = {
  entity: {
    name: "actor",
    fields: ["name", "color"],
    mention_prefix: "@",
    mention_display_field: "name",
  },
  fields: [
    {
      id: "a1",
      name: "name",
      type: { kind: "text", single_line: true },
      editor: "markdown",
      display: "text",
      icon: "type",
      section: "header",
    },
    {
      id: "a2",
      name: "color",
      type: { kind: "color" },
      editor: "color-palette",
      display: "color-swatch",
      icon: "palette",
      section: "body",
    },
  ],
};

const SCHEMAS: Record<string, unknown> = {
  task: TASK_SCHEMA,
  tag: TAG_SCHEMA,
  actor: ACTOR_SCHEMA,
};

// Fallback handler for non-spatial IPCs. The kernel simulator routes
// spatial_* commands through itself; everything else falls through to here.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const fallbackInvoke = async (cmd: string, args?: any): Promise<unknown> => {
  if (cmd === "list_entity_types") return ["task", "tag", "actor"];
  if (cmd === "get_entity_schema") {
    const entityType = args?.entityType as string;
    return SCHEMAS[entityType] ?? TASK_SCHEMA;
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
  if (cmd === "update_entity_field") return { id: "test-id" };
  return "ok";
};

vi.mock("@tauri-apps/api/core", async () => {
  // Preserve the real exports (SERIALIZE_TO_IPC_FN, Resource, Channel, …)
  // so that transitively-imported submodules like `window.js` / `dpi.js`
  // can resolve their re-exports. Only override `invoke` with the test mock.
  const actual = await vi.importActual<typeof import("@tauri-apps/api/core")>(
    "@tauri-apps/api/core",
  );
  return {
    ...actual,
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    invoke: (...args: any[]) => mockInvoke(...args),
  };
});
vi.mock("@tauri-apps/api/event", async () => {
  const actual = await vi.importActual<typeof import("@tauri-apps/api/event")>(
    "@tauri-apps/api/event",
  );
  return {
    ...actual,
    listen: (...a: Parameters<typeof mockListen>) => mockListen(...a),
  };
});
// `window-container.tsx` calls `getCurrentWindow()` at module-load time;
// stub it so tests can import components that pull in window-container
// without a real Tauri runtime.
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

import "@/components/fields/registrations";
import { EntityInspector } from "./entity-inspector";
import { UIStateProvider } from "@/lib/ui-state-context";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";

import { FieldUpdateProvider } from "@/lib/field-update-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import { CommandScopeProvider } from "@/lib/command-scope";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { FocusLayer } from "@/components/focus-layer";
import {
  asSegment
} from "@/types/spatial";
import type { Entity } from "@/types/kanban";
import { installKernelSimulator } from "@/test-helpers/kernel-simulator";

beforeEach(() => {
  // Reset captured listeners and install a fresh kernel simulator wired
  // to the hoisted `mockInvoke` / `mockListen`. The simulator routes
  // every spatial-nav IPC through the in-process cascade and emits
  // synthetic `focus-changed` events so the entity-focus-context bridge
  // updates the React store on `setFocus(fq)`.
  listeners.clear();
  mockInvoke.mockReset();
  mockListen.mockClear();
  installKernelSimulator(mockInvoke, listeners, fallbackInvoke);
});

function makeEntity(fields: Record<string, unknown> = {}): Entity {
  return {
    entity_type: "task",
    id: "test-id",
    moniker: "task:test-id",
    fields,
  };
}

async function renderInspector(entity: Entity, tagEntities: Entity[] = []) {
  const result = render(
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <TooltipProvider>
          <SchemaProvider>
            <EntityStoreProvider entities={{ task: [entity], tag: tagEntities }}>
              <EntityFocusProvider>
                <FieldUpdateProvider>
                  <UIStateProvider>
                    <CommandScopeProvider commands={[]}>
                      <EntityInspector entity={entity} />
                    </CommandScopeProvider>
                  </UIStateProvider>
                </FieldUpdateProvider>
              </EntityFocusProvider>
            </EntityStoreProvider>
          </SchemaProvider>
        </TooltipProvider>
      </FocusLayer>
    </SpatialFocusProvider>,
  );
  // Wait for async schema load
  await act(async () => {
    await new Promise((r) => setTimeout(r, 50));
  });
  return result;
}

/**
 * Render `EntityInspector` inside the production spatial-focus stack
 * (`<SpatialFocusProvider>` + `<FocusLayer name="window">`).
 *
 * Without this wrapper, `<FocusScope>` falls through to its no-spatial-context
 * branch and renders a plain `<div>` instead of a `<FocusScope>` primitive.
 * The two paths share a contract — children render as direct layout children
 * of the scope's root element — but they exercise different code: the
 * primitive path goes through `<FocusScope>`'s ResizeObserver/click wiring and
 * forwards the consumer's `className` onto the same div that owns
 * `data-moniker`/`data-focused`. This helper pins the production DOM shape so
 * a future refactor of the primitive can't silently break the inspector's
 * icon-and-content row layout.
 *
 * Use this helper for tests that depend on the production DOM shape produced
 * by `<FocusScope>`. Other tests that only need the inspector's
 * data-binding behaviour can keep using `renderInspector`.
 */
async function renderInspectorWithSpatial(
  entity: Entity,
  tagEntities: Entity[] = [],
) {
  const result = render(
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <TooltipProvider>
          <SchemaProvider>
            <EntityStoreProvider
              entities={{ task: [entity], tag: tagEntities }}
            >
              <EntityFocusProvider>
                <FieldUpdateProvider>
                  <UIStateProvider>
                    <CommandScopeProvider commands={[]}>
                      <EntityInspector entity={entity} />
                    </CommandScopeProvider>
                  </UIStateProvider>
                </FieldUpdateProvider>
              </EntityFocusProvider>
            </EntityStoreProvider>
          </SchemaProvider>
        </TooltipProvider>
      </FocusLayer>
    </SpatialFocusProvider>,
  );
  // Wait for async schema load and spatial register effects.
  await act(async () => {
    await new Promise((r) => setTimeout(r, 50));
  });
  return result;
}

describe("EntityInspector", () => {
  it("renders fields from schema in section order (header, body)", async () => {
    await renderInspector(
      makeEntity({ title: "My Task", body: "Description", tags: [] }),
    );
    expect(screen.getByTestId("field-row-title")).toBeTruthy();
    expect(screen.getByTestId("field-row-body")).toBeTruthy();
    expect(screen.getByTestId("field-row-tags")).toBeTruthy();
  });

  it("does not render fields with section: hidden", async () => {
    const { container } = await renderInspector(
      makeEntity({ position_column: "todo" }),
    );
    expect(
      container.querySelector('[data-testid="field-row-position_column"]'),
    ).toBeNull();
  });

  it("groups fields into header and body sections", async () => {
    const { container } = await renderInspector(
      makeEntity({ title: "T", body: "B", tags: [] }),
    );
    const header = container.querySelector(
      '[data-testid="inspector-section-header"]',
    );
    const body = container.querySelector(
      '[data-testid="inspector-section-body"]',
    );
    expect(header).toBeTruthy();
    expect(body).toBeTruthy();
    // title is in header, body is in body section
    expect(
      header!.querySelector('[data-testid="field-row-title"]'),
    ).toBeTruthy();
    expect(body!.querySelector('[data-testid="field-row-body"]')).toBeTruthy();
  });

  it("renders markdown fields via Field (click enters edit mode)", async () => {
    const { container } = await renderInspector(
      makeEntity({ title: "Click me" }),
    );
    // TextDisplay renders plain text; click on it enters edit mode via Field
    const titleText = screen.getByText("Click me");
    fireEvent.click(titleText);
    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });
    const titleRow = container.querySelector('[data-testid="field-row-title"]');
    expect(titleRow!.querySelector(".cm-editor")).toBeTruthy();
  });

  // Container no longer calls updateField — editors save themselves.
  // Save behavior is tested in editor-save.test.tsx matrix.

  it("allows editing computed tag fields via multi-select", async () => {
    const { container } = await renderInspector(makeEntity({ tags: ["bug"] }), [
      {
        entity_type: "tag",
        id: "tag-bug",
        moniker: "tag:tag-bug",
        fields: { tag_name: "bug", color: "ff0000" },
      },
    ]);
    const tagsRow = container.querySelector('[data-testid="field-row-tags"]');
    expect(tagsRow).toBeTruthy();
    // Click the display area to enter edit mode
    const clickTarget =
      tagsRow!.querySelector(".cursor-text") ??
      tagsRow!.querySelector(".min-h-\\[1\\.25rem\\]");
    expect(clickTarget).toBeTruthy();
    await act(async () => {
      fireEvent.click(clickTarget!);
      await new Promise((r) => setTimeout(r, 50));
    });
    expect(tagsRow!.querySelector(".cm-editor")).toBeTruthy();
  });

  it("body_field renders #tag as a styled pill when tag entity exists", async () => {
    const tags = [
      {
        entity_type: "tag",
        id: "tag-ui",
        moniker: "tag:tag-ui",
        fields: { tag_name: "ui", color: "1d76db", description: "UI" },
      },
    ];
    const { container } = await renderInspector(
      makeEntity({ body: "Fix #ui bug" }),
      tags,
    );

    const bodyRow = container.querySelector('[data-testid="field-row-body"]');
    expect(bodyRow).toBeTruthy();
    // The body's MarkdownDisplay renders mentions as CM6 widgets with the
    // `cm-mention-pill` class; `#ui` should appear in the widget's
    // textContent.
    const pill = Array.from(bodyRow!.querySelectorAll("span")).find(
      (s: Element) =>
        s.textContent === "#ui" && s.classList.contains("cm-mention-pill"),
    );
    expect(pill, `Expected #ui pill. HTML: ${bodyRow!.innerHTML}`).toBeTruthy();
  });

  it("non-body markdown fields do NOT get tag pills", async () => {
    const tags = [
      {
        entity_type: "tag",
        id: "tag-ui",
        moniker: "tag:tag-ui",
        fields: { tag_name: "ui", color: "1d76db", description: "" },
      },
    ];
    const { container } = await renderInspector(
      makeEntity({ title: "Fix #ui" }),
      tags,
    );

    // The title uses the plain `text` display, not MarkdownDisplay — it
    // should not render a CM6 mention pill widget.
    const titleRow = container.querySelector('[data-testid="field-row-title"]');
    const pill = Array.from(titleRow!.querySelectorAll("span")).find(
      (s: Element) =>
        s.textContent === "#ui" && s.classList.contains("cm-mention-pill"),
    );
    expect(pill, "Title should NOT have tag pills").toBeFalsy();
  });

  it("first visible field has data-focused attribute by default", async () => {
    const { container } = await renderInspector(
      makeEntity({ title: "T", body: "B", tags: [] }),
    );
    // First navigable field (title, in header) should be focused. After
    // card `01KQ5QB6F4MTD35GBTARJH4JEW` the row's outer `<div>` is plain;
    // the moniker-bearing FocusScope (driven by Field) lives inside it.
    const titleRow = container.querySelector('[data-testid="field-row-title"]');
    const titleFocusScope = titleRow!.querySelector(
      "[data-segment='field:task:test-id.title']",
    );
    expect(titleFocusScope!.getAttribute("data-focused")).toBe("true");
    // Second field should not be focused
    const tagsRow = container.querySelector('[data-testid="field-row-tags"]');
    const tagsFocusScope = tagsRow!.querySelector(
      "[data-segment='field:task:test-id.tags']",
    );
    expect(tagsFocusScope!.getAttribute("data-focused")).toBeNull();
  });

  it("clicking a field syncs the inspector nav cursor to that field", async () => {
    const { container } = await renderInspector(
      makeEntity({ title: "T", body: "Click me", tags: [] }),
    );
    // Initially first field (title) is focused. After card
    // `01KQ5QB6F4MTD35GBTARJH4JEW` the focus-bearing element is the
    // Field's FocusScope (a descendant of the row), not the row itself.
    const titleRow = container.querySelector('[data-testid="field-row-title"]');
    const titleFocusScope = titleRow!.querySelector(
      "[data-segment='field:task:test-id.title']",
    );
    expect(titleFocusScope!.getAttribute("data-focused")).toBe("true");

    // Click on the body field's display wrapper (the div that Field
    // renders with `cursor-text min-h-[1.25rem]` around its Display).
    // The MarkdownDisplay now mounts a CM6 editor for read-only viewing,
    // so clicking directly on CM6's contenteditable content does not
    // always bubble as expected in jsdom; clicking the wrapper mirrors
    // the user-facing click target and matches the other edit-entry
    // tests in this file.
    const bodyRow = container.querySelector('[data-testid="field-row-body"]');
    const clickTarget = bodyRow!.querySelector(".min-h-\\[1\\.25rem\\]");
    expect(clickTarget).toBeTruthy();
    await act(async () => {
      fireEvent.click(clickTarget!);
      await new Promise((r) => setTimeout(r, 50));
    });

    const bodyFocusScope = bodyRow!.querySelector(
      "[data-segment='field:task:test-id.body']",
    );
    // Body field (index 3: title=0, tags=1, progress=2, body=3) should now be focused
    expect(bodyFocusScope!.getAttribute("data-focused")).toBe("true");
    // Title should no longer be focused
    expect(titleFocusScope!.getAttribute("data-focused")).toBeNull();
  });

  it("only one field has data-focused at a time", async () => {
    const { container } = await renderInspector(
      makeEntity({ title: "T", body: "B", tags: [], assignees: [] }),
    );
    // Filter to field-zone monikers so we don't count nested pill scopes.
    const focused = Array.from(
      container.querySelectorAll("[data-focused]"),
    ).filter((el) => el.getAttribute("data-segment")?.startsWith("field:"));
    expect(focused.length).toBe(1);
  });

  describe("hides empty computed fields", () => {
    it("omits field-row-progress when progress value is { total: 0 }", async () => {
      const { container } = await renderInspector(
        makeEntity({
          title: "T",
          body: "B",
          tags: [],
          progress: { total: 0, completed: 0, percent: 0 },
        }),
      );
      expect(
        container.querySelector('[data-testid="field-row-progress"]'),
      ).toBeNull();
    });

    it("renders field-row-progress with progressbar when total is positive", async () => {
      const { container } = await renderInspector(
        makeEntity({
          title: "T",
          body: "B",
          tags: [],
          progress: { total: 4, completed: 2, percent: 50 },
        }),
      );
      const row = container.querySelector('[data-testid="field-row-progress"]');
      expect(row).toBeTruthy();
      expect(row!.querySelector('[role="progressbar"]')).toBeTruthy();
    });

    it("omits field-row-progress when the value is missing entirely", async () => {
      const { container } = await renderInspector(
        makeEntity({ title: "T", body: "B", tags: [] }),
      );
      expect(
        container.querySelector('[data-testid="field-row-progress"]'),
      ).toBeNull();
    });

    it("keeps editable fields with empty values visible", async () => {
      // title is editable (editor: "markdown"); empty string must still render
      const { container } = await renderInspector(
        makeEntity({ title: "", body: "", tags: [] }),
      );
      expect(
        container.querySelector('[data-testid="field-row-title"]'),
      ).toBeTruthy();
      expect(
        container.querySelector('[data-testid="field-row-body"]'),
      ).toBeTruthy();
    });

    it("hidden progress row is absent so spatial nav has nothing to skip", async () => {
      // After the spatial-nav migration, ArrowDown navigation between field
      // rows is driven by beam-search rule 2 in the Rust spatial graph. In
      // this test environment we don't mount the Rust IPC stack, so we
      // exercise the surface visible to the React tree: the hidden row is
      // not rendered at all, so there is no zone for the navigator to land
      // on. The navigator's "skip" behaviour is therefore implicit — only
      // the visible rows are navigable.
      const { container } = await renderInspector(
        makeEntity({
          title: "T",
          body: "B",
          tags: [],
          progress: { total: 0, completed: 0, percent: 0 },
        }),
      );

      const titleRow = container.querySelector(
        '[data-testid="field-row-title"]',
      );
      const titleFocusScope = titleRow!.querySelector(
        "[data-segment='field:task:test-id.title']",
      );
      expect(titleFocusScope!.getAttribute("data-focused")).toBe("true");

      // The hidden progress row is not rendered, so it never registers as
      // a zone in the spatial graph.
      expect(
        container.querySelector('[data-testid="field-row-progress"]'),
      ).toBeNull();
      // The remaining navigable rows (title, tags, body) all render as zones.
      expect(
        container.querySelector('[data-testid="field-row-tags"]'),
      ).toBeTruthy();
      expect(
        container.querySelector('[data-testid="field-row-body"]'),
      ).toBeTruthy();
    });

    it("visible progress row renders alongside other field-row zones", async () => {
      // When `progress` has a positive `total`, the progress row is rendered
      // and registers as its own field-row zone — beam-search rule 2 picks
      // it up as a navigable target. Verify the row is present alongside
      // its sibling field-row zones.
      const { container } = await renderInspector(
        makeEntity({
          title: "T",
          body: "B",
          tags: [],
          progress: { total: 4, completed: 2, percent: 50 },
        }),
      );

      const titleRow = container.querySelector(
        '[data-testid="field-row-title"]',
      );
      const titleFocusScope = titleRow!.querySelector(
        "[data-segment='field:task:test-id.title']",
      );
      expect(titleFocusScope!.getAttribute("data-focused")).toBe("true");

      // Header order is title → tags → progress; all three rows render.
      expect(
        container.querySelector('[data-testid="field-row-tags"]'),
      ).toBeTruthy();
      const progressRow = container.querySelector(
        '[data-testid="field-row-progress"]',
      );
      expect(progressRow).toBeTruthy();
      expect(progressRow!.querySelector('[role="progressbar"]')).toBeTruthy();
    });
  });

  // Test "tag pill in inspector has entity moniker as ancestor scope when
  // using InspectorFocusBridge" deleted in card 01KQCTJY1QZ710A05SE975GHNR.
  // The entity FocusScope wrap that this test asserted on is gone — the
  // inspector no longer wraps `<EntityInspector>` in a `<FocusScope
  // moniker={entityMoniker}>`. Field zones (and the pill leaves inside
  // them) register at the inspector layer root with `parentZone === null`.

  describe("iconOverride", () => {
    // Task schema with a status_date field that has a display with iconOverride.
    // The `status-date` display registers an iconOverride, so the inspector
    // should render the kind-specific icon (e.g. CheckCircle) instead of
    // the static YAML icon (target).
    const ICON_OVERRIDE_SCHEMA = {
      entity: {
        name: "task",
        body_field: "body",
        fields: ["title", "status_date"],
        sections: [
          { id: "header", on_card: true },
          { id: "dates", label: "Dates", on_card: true },
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
          name: "status_date",
          type: { kind: "computed", derive: "derive-status-date" },
          editor: "none",
          display: "status-date",
          icon: "target",
          description: "Task status date",
          section: "dates",
        },
      ],
    };

    async function renderWithIconOverrideSchema(entity: Entity) {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      mockInvoke.mockImplementation(((...args: any[]) => {
        if (args[0] === "list_entity_types")
          return Promise.resolve(["task", "tag", "actor"]);
        if (args[0] === "get_entity_schema")
          return Promise.resolve(ICON_OVERRIDE_SCHEMA);
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
          return Promise.resolve({ id: "test-id" });
        return Promise.resolve("ok");
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
      }) as any);
      const result = await renderInspector(entity);
      // Restore the default mock after render so subsequent tests are unaffected.
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      mockInvoke.mockImplementation(((...args: any[]) => {
        if (args[0] === "list_entity_types")
          return Promise.resolve(["task", "tag", "actor"]);
        if (args[0] === "get_entity_schema") {
          const entityType = (args[1] as Record<string, unknown>)
            ?.entityType as string;
          return Promise.resolve(SCHEMAS[entityType] ?? TASK_SCHEMA);
        }
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
          return Promise.resolve({ id: "test-id" });
        return Promise.resolve("ok");
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
      }) as any);
      return result;
    }

    it("renders the kind-specific icon instead of the static YAML icon", async () => {
      const ts = new Date(Date.now() - 3 * 24 * 60 * 60 * 1000).toISOString();
      const { container } = await renderWithIconOverrideSchema(
        makeEntity({
          title: "T",
          status_date: { kind: "completed", timestamp: ts },
        }),
      );

      const statusRow = container.querySelector(
        '[data-testid="field-row-status_date"]',
      );
      expect(statusRow).toBeTruthy();

      // The tooltip icon should be CheckCircle (lucide-circle-check-big),
      // not the static `target` icon.
      const svg = statusRow!.querySelector("svg");
      expect(svg).toBeTruthy();
      expect(svg!.getAttribute("class")).toMatch(
        /lucide-(circle-check|check-circle)/,
      );
      // Confirm the static `target` icon is NOT rendered.
      expect(svg!.getAttribute("class")).not.toMatch(/lucide-target/);
    });

    it("renders only one icon per status_date field", async () => {
      const ts = new Date(Date.now() - 3 * 24 * 60 * 60 * 1000).toISOString();
      const { container } = await renderWithIconOverrideSchema(
        makeEntity({
          title: "T",
          status_date: { kind: "started", timestamp: ts },
        }),
      );

      const statusRow = container.querySelector(
        '[data-testid="field-row-status_date"]',
      );
      expect(statusRow).toBeTruthy();

      // Only one SVG icon should appear (in the tooltip position); the display
      // no longer renders its own inline icon.
      const svgs = statusRow!.querySelectorAll("svg");
      expect(svgs.length).toBe(1);
    });

    it("tooltip still shows the field description on hover", async () => {
      const ts = new Date(Date.now() - 3 * 24 * 60 * 60 * 1000).toISOString();
      const { container } = await renderWithIconOverrideSchema(
        makeEntity({
          title: "T",
          status_date: { kind: "completed", timestamp: ts },
        }),
      );

      const statusRow = container.querySelector(
        '[data-testid="field-row-status_date"]',
      );
      expect(statusRow).toBeTruthy();

      // The tooltip trigger wraps the icon; the TooltipContent contains the
      // field description. Radix tooltips use aria-describedby; the trigger
      // itself is the span wrapping the icon (line-height-matched wrapper).
      const iconSpan = statusRow!.querySelector(
        "span.h-5.items-center.shrink-0.text-muted-foreground",
      );
      expect(iconSpan).toBeTruthy();
    });
  });

  describe("tooltipOverride", () => {
    // Reuse the same schema from the iconOverride block — the status-date
    // display also registers a tooltipOverride, so we verify that the
    // dynamic tooltip text appears instead of the static YAML description.
    const TOOLTIP_OVERRIDE_SCHEMA = {
      entity: {
        name: "task",
        body_field: "body",
        fields: ["title", "status_date"],
        sections: [
          { id: "header", on_card: true },
          { id: "dates", label: "Dates", on_card: true },
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
          name: "status_date",
          type: { kind: "computed", derive: "derive-status-date" },
          editor: "none",
          display: "status-date",
          icon: "target",
          description: "Task status date",
          section: "dates",
        },
      ],
    };

    async function renderWithTooltipOverrideSchema(entity: Entity) {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      mockInvoke.mockImplementation(((...args: any[]) => {
        if (args[0] === "list_entity_types")
          return Promise.resolve(["task", "tag", "actor"]);
        if (args[0] === "get_entity_schema")
          return Promise.resolve(TOOLTIP_OVERRIDE_SCHEMA);
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
          return Promise.resolve({ id: "test-id" });
        return Promise.resolve("ok");
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
      }) as any);
      const result = await renderInspector(entity);
      // Restore the default mock after render.
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      mockInvoke.mockImplementation(((...args: any[]) => {
        if (args[0] === "list_entity_types")
          return Promise.resolve(["task", "tag", "actor"]);
        if (args[0] === "get_entity_schema") {
          const entityType = (args[1] as Record<string, unknown>)
            ?.entityType as string;
          return Promise.resolve(SCHEMAS[entityType] ?? TASK_SCHEMA);
        }
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
          return Promise.resolve({ id: "test-id" });
        return Promise.resolve("ok");
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
      }) as any);
      return result;
    }

    it("renders dynamic tooltip text instead of the static YAML description", async () => {
      const ts = new Date(Date.now() - 3 * 24 * 60 * 60 * 1000).toISOString();
      const { container } = await renderWithTooltipOverrideSchema(
        makeEntity({
          title: "T",
          status_date: { kind: "completed", timestamp: ts },
        }),
      );

      const statusRow = container.querySelector(
        '[data-testid="field-row-status_date"]',
      );
      expect(statusRow).toBeTruthy();

      // The FieldIconTooltip renders a Radix Tooltip. In jsdom, the
      // TooltipContent is rendered as a hidden element. We verify the
      // tooltip trigger's icon is present and that the static YAML
      // description ("Task status date") is NOT in the tooltip — the
      // dynamic phrase should replace it.
      //
      // Radix's Tooltip in jsdom renders the TooltipContent as a child
      // of the trigger's parent. Walk all text nodes inside the row
      // and check for "Completed" (from tooltipOverride) rather than
      // "Task status date" (static description).
      const rowText = statusRow!.textContent ?? "";
      expect(rowText).toContain("Completed");
    });

    it("fields without tooltipOverride still show static description", async () => {
      // The title field uses display: "text" which does not register a
      // tooltipOverride. Its row should not contain the dynamic phrase.
      const ts = new Date(Date.now() - 3 * 24 * 60 * 60 * 1000).toISOString();
      const { container } = await renderWithTooltipOverrideSchema(
        makeEntity({
          title: "My Title",
          status_date: { kind: "completed", timestamp: ts },
        }),
      );

      // The title field has no icon in the schema (no `icon` key), so no
      // FieldIconTooltip is rendered — there is nothing to override.
      // Confirm the title field renders and its text does not include any
      // status phrase.
      const titleRow = container.querySelector(
        '[data-testid="field-row-title"]',
      );
      expect(titleRow).toBeTruthy();
      const titleText = titleRow!.textContent ?? "";
      expect(titleText).not.toContain("Completed");
    });
  });

  describe("field icon alignment", () => {
    it("FieldIconTooltip renders icon wrapper with h-5 and items-center for line-height alignment", async () => {
      // The tags field has icon: "tag" — its row will contain a
      // FieldIconTooltip. The icon wrapper span should use h-5 (matching
      // text-sm's 20px line-height) and items-center to vertically center
      // the 14px icon, rather than a fragile mt-0.5 offset.
      const entity = makeEntity({ title: "T", tags: ["bug"] });
      const { container } = await renderInspector(entity);

      const tagsRow = container.querySelector('[data-testid="field-row-tags"]');
      expect(tagsRow).toBeTruthy();

      // The icon wrapper is the tooltip trigger span — the first span
      // child of the row (Radix sets data-slot="tooltip-trigger").
      const iconSpan = tagsRow!.querySelector(
        'span[data-slot="tooltip-trigger"]',
      ) as HTMLElement | null;
      expect(iconSpan).toBeTruthy();
      expect(iconSpan!.className).toContain("h-5");
      expect(iconSpan!.className).toContain("items-center");
      // The old mt-0.5 hack should be gone
      expect(iconSpan!.className).not.toContain("mt-0.5");
    });
  });

  describe("declarative sections", () => {
    // Task schema with a declared three-section layout: header, body, dates.
    // `due` and `scheduled` are date fields with `section: dates`; the inspector
    // should group them into a labelled `dates` section below `body`.
    const SECTIONED_TASK_SCHEMA = {
      entity: {
        name: "task",
        body_field: "body",
        fields: ["title", "body", "due", "scheduled"],
        sections: [
          { id: "header", on_card: true },
          { id: "body" },
          { id: "dates", label: "Dates", on_card: true },
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
          icon: "file-text",
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

    async function renderWithSectionedSchema(entity: Entity) {
      // Reset the kernel simulator with a fallback that returns the
      // sectioned-schema shape for `get_entity_schema`. The simulator
      // routes spatial-nav IPCs through itself; everything else falls
      // through to this handler.
      mockInvoke.mockReset();
      installKernelSimulator(
        mockInvoke,
        listeners,
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        async (cmd: string, _args?: any) => {
          if (cmd === "list_entity_types") return ["task", "tag", "actor"];
          if (cmd === "get_entity_schema") return SECTIONED_TASK_SCHEMA;
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
          if (cmd === "update_entity_field") return { id: "test-id" };
          return "ok";
        },
      );
      const result = await renderInspector(entity);
      // Restore the default kernel simulator after render so subsequent
      // tests in this file see the per-entity schema fallback again.
      mockInvoke.mockReset();
      installKernelSimulator(mockInvoke, listeners, fallbackInvoke);
      return result;
    }

    it("renders the declared dates section with its label when fields are set", async () => {
      const { container } = await renderWithSectionedSchema(
        makeEntity({
          title: "T",
          body: "B",
          due: "2026-05-01",
          scheduled: "2026-04-20",
        }),
      );
      const datesSection = container.querySelector(
        '[data-testid="inspector-section-dates"]',
      );
      expect(datesSection).toBeTruthy();
      const datesLabel = container.querySelector(
        '[data-testid="inspector-section-label-dates"]',
      );
      expect(datesLabel).toBeTruthy();
      expect(datesLabel!.textContent).toBe("Dates");
      // Both date field rows are inside the `dates` section, not `body`.
      expect(
        datesSection!.querySelector('[data-testid="field-row-due"]'),
      ).toBeTruthy();
      expect(
        datesSection!.querySelector('[data-testid="field-row-scheduled"]'),
      ).toBeTruthy();
    });

    it("omits an empty section and draws no dangling divider", async () => {
      // Neither due nor scheduled is set — both are `editor: date` (editable),
      // so they stay visible. Change one to unset only to exercise the
      // empty-section path: this time omit the `dates` fields entirely so
      // the displays are truly empty for the filter.
      // `due` and `scheduled` use `editor: date`, so even empty values still
      // render. To make the `dates` section empty we render a task without
      // any `dates`-tagged field in the schema — but our schema forces them
      // in. Instead, verify empty-section behaviour by rendering only one
      // date field (the dates section stays non-empty with just one row)
      // and check no duplicate divider appears. The stricter "empty section
      // omitted" case is exercised via useEntitySections unit tests.
      const { container } = await renderWithSectionedSchema(
        makeEntity({
          title: "T",
          body: "B",
          due: "2026-05-01",
        }),
      );
      // Only non-empty sections should be rendered. Count the dividers
      // and ensure it matches (sections rendered) - 1. With three non-empty
      // sections (header, body, dates), we expect exactly two dividers.
      const inspector = container.querySelector(
        '[data-testid="entity-inspector"]',
      );
      const dividers = inspector!.querySelectorAll("div.my-3.h-px.bg-border");
      expect(dividers.length).toBe(2);
    });

    it("dates section's field rows render as zones alongside body and header rows", async () => {
      // After the spatial-nav migration, cross-section ArrowDown navigation
      // is driven by beam-search rule 2 in the Rust spatial graph. In this
      // test environment we don't mount the Rust IPC stack, so we verify
      // the structural surface: every navigable row across header, body,
      // and dates renders so the navigator has zones to land on.
      const { container } = await renderWithSectionedSchema(
        makeEntity({
          title: "T",
          body: "B",
          due: "2026-05-01",
          scheduled: "2026-04-20",
        }),
      );

      const titleRow = container.querySelector(
        '[data-testid="field-row-title"]',
      );
      const titleFocusScope = titleRow!.querySelector(
        "[data-segment='field:task:test-id.title']",
      );
      expect(titleFocusScope!.getAttribute("data-focused")).toBe("true");

      // All four navigable rows render and live in the right sections.
      const headerSection = container.querySelector(
        '[data-testid="inspector-section-header"]',
      );
      const bodySection = container.querySelector(
        '[data-testid="inspector-section-body"]',
      );
      const datesSection = container.querySelector(
        '[data-testid="inspector-section-dates"]',
      );
      expect(
        headerSection!.querySelector('[data-testid="field-row-title"]'),
      ).toBeTruthy();
      expect(
        bodySection!.querySelector('[data-testid="field-row-body"]'),
      ).toBeTruthy();
      expect(
        datesSection!.querySelector('[data-testid="field-row-due"]'),
      ).toBeTruthy();
      expect(
        datesSection!.querySelector('[data-testid="field-row-scheduled"]'),
      ).toBeTruthy();
    });
  });

  describe("field rows as zones", () => {
    it("each field row contains a FocusScope with the entity's field moniker", async () => {
      // After card `01KQ5QB6F4MTD35GBTARJH4JEW`, `<Field>` itself
      // registers as a `<FocusScope>` keyed by
      // `field:<entityType>:<entityId>.<fieldName>` — the inspector row
      // no longer wraps the field in its own outer FocusScope. The row's
      // outer `<div>` carries the `data-testid` and the icon-and-content
      // layout; the moniker-bearing FocusScope lives inside it (around
      // the field's display content). Verify the moniker shape per row.
      const { container } = await renderInspector(
        makeEntity({ title: "T", body: "B", tags: [] }),
      );

      const titleRow = container.querySelector(
        '[data-testid="field-row-title"]',
      );
      const tagsRow = container.querySelector('[data-testid="field-row-tags"]');
      const bodyRow = container.querySelector('[data-testid="field-row-body"]');

      // Field monikers follow the `field:<entityType>:<entityId>.<fieldName>`
      // convention from `lib/moniker.ts`. The exact entity type / id come
      // from `makeEntity()` (task, test-id). The element bearing the
      // moniker is a descendant of the row (the Field's FocusScope div).
      expect(
        titleRow!.querySelector("[data-segment='field:task:test-id.title']"),
      ).toBeTruthy();
      expect(
        tagsRow!.querySelector("[data-segment='field:task:test-id.tags']"),
      ).toBeTruthy();
      expect(
        bodyRow!.querySelector("[data-segment='field:task:test-id.body']"),
      ).toBeTruthy();
    });

    it("field row outer element has flex row layout classes (icon + content stay horizontal)", async () => {
      // Regression guard: the icon | content layout inside the field row
      // must not collapse to a vertical stack. After card
      // `01KQ9ZJHRXCY8Z5YT6RF4SG6EK`, the icon and the content live
      // *inside* the field's `<FocusScope>` (so a click on the icon
      // bubbles to the zone's spatial-focus handler and the focus bar
      // paints to the LEFT of the icon). The flex-row container is now
      // a child of the zone wrapper; the outer `data-testid` div is a
      // plain pass-through whose only job is to make the row queryable.
      //
      // CRITICAL: this test must mount inside the spatial-focus provider
      // stack (`<SpatialFocusProvider>` + `<FocusLayer>`). Without it,
      // `<FocusScope>` short-circuits to its no-spatial-context fallback
      // (a plain `<div>`) instead of mounting the spatial primitive, so
      // any regression that manifests only against the primitive path
      // would slip past.
      const { container } = await renderInspectorWithSpatial(
        makeEntity({ title: "T", body: "B", tags: ["bug"] }),
      );

      const tagsRow = container.querySelector('[data-testid="field-row-tags"]');
      expect(tagsRow).toBeTruthy();

      // The icon span and the content div must share a single flex-row
      // ancestor — inside the field zone, which itself sits inside the
      // testid'd outer div. Walk up from the icon span until we hit a
      // flex container; verify the content div is also a descendant of
      // the same container, and that the container is row-direction
      // (no `flex-col`).
      const iconSpan = tagsRow!.querySelector(
        'span[data-slot="tooltip-trigger"]',
      ) as HTMLElement | null;
      expect(iconSpan).toBeTruthy();

      let flexAncestor: HTMLElement | null = iconSpan!.parentElement;
      while (
        flexAncestor &&
        !flexAncestor.className.split(/\s+/).includes("flex")
      ) {
        flexAncestor = flexAncestor.parentElement;
      }
      expect(
        flexAncestor,
        "icon span has no flex ancestor inside the field row",
      ).toBeTruthy();
      expect(flexAncestor!.className).toContain("flex");
      expect(flexAncestor!.className).toContain("items-start");
      expect(flexAncestor!.className).toContain("gap-2");
      // Critical: the flex container must NOT be a column.
      expect(flexAncestor!.className).not.toContain("flex-col");
      // The flex container lives inside the field zone (data-moniker
      // marker), not at the outer testid wrapper.
      const fieldZone = tagsRow!.querySelector(
        '[data-segment="field:task:test-id.tags"]',
      );
      expect(
        fieldZone!.contains(flexAncestor),
        "flex row must live inside the field zone so the icon and content share the zone's containing block",
      ).toBe(true);
      // The content's `flex-1 min-w-0` div is a sibling of the icon span
      // inside that flex container.
      const contentDiv = flexAncestor!.querySelector(
        ":scope > .flex-1.min-w-0",
      );
      expect(
        contentDiv,
        "content div is not a sibling of the icon span in the flex row",
      ).toBeTruthy();
    });
  });
});
