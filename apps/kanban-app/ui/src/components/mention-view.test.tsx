/**
 * Tests for MentionView — the CM6-based mention pill renderer.
 *
 * Verifies:
 * - Single mode renders a CM6 widget showing the entity's clipped display name
 * - Unknown id falls back to raw slug muted styling
 * - List mode renders one FocusScope per item with correct pill text
 * - List mode supports nav.left/nav.right keyboard navigation between pills
 * - Right-click context menu includes extraCommands (e.g. task.untag)
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, act, fireEvent, waitFor } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Tauri and plugin mocks — declared before importing component under test
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

import type { Entity } from "@/types/kanban";
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

/**
 * Shape returned by the backend `list_commands_for_scope`.
 * Used to build mock responses for context menu tests.
 */
interface ResolvedCommand {
  id: string;
  name: string;
  target?: string;
  group: string;
  context_menu: boolean;
  keys?: { vim?: string; cua?: string; emacs?: string };
  available: boolean;
}

/**
 * Helper: configure invoke mock to return the given commands when
 * `list_commands_for_scope` is called, and resolve for everything else.
 */
function mockListCommands(commands: ResolvedCommand[]) {
  mockInvoke.mockImplementation(
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (cmd: string, _args?: any): Promise<unknown> => {
      if (cmd === "list_commands_for_scope") return Promise.resolve(commands);
      return Promise.resolve(undefined);
    },
  );
}

// ---------------------------------------------------------------------------

import { MentionView } from "./mention-view";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { FocusScope } from "@/components/focus-scope";
import { FocusLayer } from "@/components/focus-layer";
import { TooltipProvider } from "@/components/ui/tooltip";
import { asSegment } from "@/types/spatial";

// ---------------------------------------------------------------------------
// Test fixtures
// ---------------------------------------------------------------------------

/** Reset mock state to a clean baseline — two entity types with known entities. */
function setupFixtures() {
  mockEntities = {
    project: [
      {
        id: "p1",
        entity_type: "project",
        moniker: "project:p1",
        fields: {
          name: "Spatial Focus Navigation",
          color: "6366f1",
        },
      },
    ],
    tag: [
      {
        id: "tag-1",
        entity_type: "tag",
        moniker: "tag:tag-1",
        fields: { tag_name: "bugfix", color: "ff0000" },
      },
      {
        id: "tag-2",
        entity_type: "tag",
        moniker: "tag:tag-2",
        fields: { tag_name: "feature", color: "00ff00" },
      },
    ],
    actor: [
      {
        id: "alice",
        entity_type: "actor",
        moniker: "actor:alice",
        fields: { name: "Alice Example", color: "aabbcc" },
      },
    ],
  };
  mockMentionableTypes = [
    { entityType: "project", prefix: "$", displayField: "name" },
    { entityType: "tag", prefix: "#", displayField: "tag_name" },
    { entityType: "actor", prefix: "@", displayField: "name" },
  ];
}

/**
 * Wrap children in the minimal provider tree `MentionView` needs. The
 * spatial provider stack (`SpatialFocusProvider` + `FocusLayer`) is
 * required since `MentionView` mounts a `<FocusScope>` per pill, and
 * the no-spatial-context fallback was removed in card
 * `01KQPVA127YMJ8D7NB6M824595`.
 */
function Providers({ children }: { children: React.ReactNode }) {
  return (
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <EntityFocusProvider>
          <TooltipProvider>{children}</TooltipProvider>
        </EntityFocusProvider>
      </FocusLayer>
    </SpatialFocusProvider>
  );
}

/** Flush microtasks and pending effects. */
async function flush() {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 0));
  });
}

describe("MentionView — single mode", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockInvoke.mockResolvedValue(undefined);
    setupFixtures();
  });

  it("renders a CM6 widget with the entity's clipped display name", async () => {
    const { container } = render(
      <Providers>
        <MentionView entityType="project" id="p1" />
      </Providers>,
    );
    await flush();

    // The CM6 mention widget renders with class cm-mention-pill.
    const widget = container.querySelector(".cm-mention-pill");
    expect(widget).toBeTruthy();
    expect(widget?.textContent).toBe("$Spatial Focus Navigation");
  });

  it("wraps the TextViewer in a FocusScope bearing the entity moniker", async () => {
    const { container } = render(
      <Providers>
        <MentionView entityType="project" id="p1" />
      </Providers>,
    );
    await flush();

    const scope = container.querySelector("[data-segment='project:p1']");
    expect(scope).toBeTruthy();
  });

  it("falls back to raw id with muted mark styling when entity is missing", async () => {
    mockEntities = { project: [] };
    const { container } = render(
      <Providers>
        <MentionView entityType="project" id="missing-project" />
      </Providers>,
    );
    await flush();

    // No widget, because widget requires metaMap entry with valid color.
    const widget = container.querySelector(".cm-mention-pill");
    expect(widget).toBeFalsy();

    // Raw slug text visible with the project-pill mark class (muted default).
    const mark = container.querySelector(".cm-project-pill");
    expect(mark).toBeTruthy();
    expect(mark?.textContent).toBe("$missing-project");
  });
});

describe("MentionView — list mode", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockInvoke.mockResolvedValue(undefined);
    setupFixtures();
  });

  it("renders one FocusScope per item with clipped display names across entity types", async () => {
    const { container } = render(
      <Providers>
        <MentionView
          items={[
            { entityType: "tag", id: "tag-1" },
            { entityType: "tag", id: "tag-2" },
            { entityType: "actor", id: "alice" },
          ]}
        />
      </Providers>,
    );
    await flush();

    // Three separate FocusScopes (one per item).
    const scopes = container.querySelectorAll("[data-segment]");
    expect(scopes.length).toBe(3);

    // Three CM6 widgets with the expected display names.
    const widgets = container.querySelectorAll(".cm-mention-pill");
    expect(widgets.length).toBe(3);
    const texts = Array.from(widgets).map((w) => w.textContent);
    expect(texts).toContain("#bugfix");
    expect(texts).toContain("#feature");
    expect(texts).toContain("@Alice Example");
  });

  it("wraps list items in a flex-wrap container", async () => {
    const { container } = render(
      <Providers>
        <MentionView
          items={[
            { entityType: "tag", id: "tag-1" },
            { entityType: "tag", id: "tag-2" },
          ]}
        />
      </Providers>,
    );
    await flush();

    // The list container should have the flex-wrap utility classes.
    const flex = container.querySelector(".flex.flex-wrap");
    expect(flex).toBeTruthy();
    expect(flex?.classList.contains("gap-1.5")).toBe(true);
  });

  it("renders empty without crashing when items is empty", async () => {
    const { container } = render(
      <Providers>
        <MentionView items={[]} />
      </Providers>,
    );
    await flush();

    const scopes = container.querySelectorAll("[data-segment]");
    expect(scopes.length).toBe(0);
  });

  it("renders one pill scope per item, nested inside the parent field row", async () => {
    // After the spatial-nav zone migration, within-list keyboard navigation
    // is handled by beam-search rule 1 in the Rust spatial graph rather
    // than by `claimWhen` predicates on each pill. The pills are simple
    // leaves nested under the parent field-row scope; the beam search
    // picks the next horizontal candidate by rect math. This test
    // verifies the structural surface — the actual rule-1 beam-search
    // behaviour is exercised by the Rust spatial-nav unit tests.
    const parentMoniker = "field:mixed";

    const { container } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asSegment("window")}>
          <EntityFocusProvider>
            <TooltipProvider>
              <FocusScope moniker={asSegment(parentMoniker)} commands={[]}>
                <MentionView
                  items={[
                    { entityType: "tag", id: "tag-1" },
                    { entityType: "tag", id: "tag-2" },
                    { entityType: "actor", id: "alice" },
                  ]}
                  mode="full"
                />
              </FocusScope>
            </TooltipProvider>
          </EntityFocusProvider>
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flush();

    // The parent field-row scope sits at the top of the tree.
    const fieldRow = container.querySelector(
      `[data-segment="${parentMoniker}"]`,
    );
    expect(fieldRow).toBeTruthy();

    // Three pill scopes — one per item — are nested inside it.
    const pills = fieldRow!.querySelectorAll(
      '[data-segment]:not([data-segment="field:mixed"])',
    ) as NodeListOf<HTMLElement>;
    const segments = Array.from(pills).map((p) =>
      p.getAttribute("data-segment"),
    );
    expect(segments).toEqual(["tag:tag-1", "tag:tag-2", "actor:alice"]);
  });
});

describe("MentionView — extraCommands", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockInvoke.mockResolvedValue(undefined);
    setupFixtures();
  });

  it("single mode right-click includes extraCommands (e.g. Remove Tag)", async () => {
    mockListCommands([
      {
        id: "entity.inspect",
        name: "Inspect Tag",
        target: "tag:tag-1",
        group: "entity",
        context_menu: true,
        available: true,
      },
      {
        id: "task.untag",
        name: "Remove Tag",
        group: "entity",
        context_menu: true,
        available: true,
      },
    ]);

    const { container } = render(
      <Providers>
        <MentionView
          entityType="tag"
          id="tag-1"
          extraCommands={[
            {
              id: "task.untag",
              name: "Remove Tag",
              contextMenu: true,
              args: { id: "task-99", tag: "bugfix" },
            },
          ]}
        />
      </Providers>,
    );
    await flush();

    const scope = container.querySelector(
      "[data-segment='tag:tag-1']",
    ) as HTMLElement | null;
    expect(scope).toBeTruthy();
    fireEvent.contextMenu(scope!);

    await waitFor(() => {
      const call = mockInvoke.mock.calls.find(
        (c: unknown[]) => c[0] === "show_context_menu",
      );
      expect(call).toBeTruthy();
      const items = (call![1] as { items: { cmd: string; name: string }[] })
        .items;
      expect(items.find((i) => i.cmd === "task.untag")).toBeTruthy();
    });
  });
});
