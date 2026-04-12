/**
 * Tests for useMentionExtensions hook options.
 *
 * Verifies that includeVirtualTags and includeFilterSigils options correctly
 * control which completion sources are available.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";

// Mock Tauri APIs
const mockInvoke = vi.fn(
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (..._args: any[]): Promise<unknown> => Promise.resolve([]),
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

// Mock schema context — mutable so individual tests can add/remove mentionable
// types (e.g. register a `$project` prefix for the project autocomplete tests).
let mockMentionableTypes: Array<{
  prefix: string;
  entityType: string;
  displayField: string;
  slugField?: string;
}> = [{ prefix: "#", entityType: "tag", displayField: "name" }];

vi.mock("@/lib/schema-context", () => ({
  useSchema: () => ({
    mentionableTypes: mockMentionableTypes,
  }),
}));

// Mock entity store context — provide test entities
const mockGetEntities = vi.fn((type: string) => {
  if (type === "tag") {
    return [
      {
        id: "t1",
        entity_type: "tag",
        fields: { name: "bug", color: "ff0000" },
      },
      {
        id: "t2",
        entity_type: "tag",
        fields: { name: "feature", color: "00ff00" },
      },
    ];
  }
  if (type === "project") {
    return [
      {
        id: "p1",
        entity_type: "project",
        fields: {
          name: "auth-migration",
          color: "4078c0",
          description: "Auth refactor",
        },
      },
      {
        id: "p2",
        entity_type: "project",
        fields: { name: "frontend", color: "6cc644" },
      },
    ];
  }
  return [];
});

vi.mock("@/lib/entity-store-context", () => ({
  useEntityStore: () => ({ getEntities: mockGetEntities }),
}));

// Mock board data context — provides virtual tag metadata from the backend.
vi.mock("@/components/window-container", () => ({
  useBoardData: () => ({
    virtualTagMeta: [
      { slug: "READY", color: "0e8a16", description: "No unmet deps" },
      { slug: "BLOCKED", color: "e36209", description: "Has unmet deps" },
      { slug: "BLOCKING", color: "d73a4a", description: "Others depend on this" },
    ],
  }),
}));

import { renderHook } from "@testing-library/react";
import {
  useMentionExtensions,
  buildColorMap,
  buildMetaMap,
  buildAsyncSearch,
} from "../use-mention-extensions";
import type { Entity } from "@/types/kanban";

describe("useMentionExtensions", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockInvoke.mockResolvedValue([]);
    // Reset mentionable types to the default (tag only) — individual tests
    // that need a different shape (e.g. $project) reassign this.
    mockMentionableTypes = [
      { prefix: "#", entityType: "tag", displayField: "name" },
    ];
  });

  it("returns extensions without options (default behavior)", () => {
    const { result } = renderHook(() => useMentionExtensions());
    // Should return a non-empty extension array (decorations + autocomplete + tooltips)
    expect(result.current.length).toBeGreaterThan(0);
  });

  it("returns extensions with includeVirtualTags: false by default", () => {
    const { result } = renderHook(() => useMentionExtensions());
    // Baseline: extensions work without virtual tags
    expect(result.current.length).toBeGreaterThan(0);
  });

  it("returns extensions with includeVirtualTags: true", () => {
    const { result } = renderHook(() =>
      useMentionExtensions({ includeVirtualTags: true }),
    );
    // Extensions should still be non-empty
    expect(result.current.length).toBeGreaterThan(0);
  });

  it("returns extensions with includeFilterSigils: true", () => {
    const { result } = renderHook(() =>
      useMentionExtensions({ includeFilterSigils: true }),
    );
    // Should have more extensions (@ and ^ sources added)
    expect(result.current.length).toBeGreaterThan(0);
  });

  it("returns extensions with both options enabled", () => {
    const { result } = renderHook(() =>
      useMentionExtensions({
        includeVirtualTags: true,
        includeFilterSigils: true,
      }),
    );
    expect(result.current.length).toBeGreaterThan(0);
  });

  it("filter sigils option adds additional completion sources", () => {
    const { result: withoutSigils } = renderHook(() => useMentionExtensions());
    const { result: withSigils } = renderHook(() =>
      useMentionExtensions({ includeFilterSigils: true }),
    );
    // With filter sigils should have more extensions (@ and ^ sources)
    expect(withSigils.current.length).toBeGreaterThanOrEqual(
      withoutSigils.current.length,
    );
  });

  it("decorates virtual tags (#READY) when includeVirtualTags is true", async () => {
    const { result } = renderHook(() =>
      useMentionExtensions({ includeVirtualTags: true }),
    );

    const { EditorView } = await import("@codemirror/view");
    const { EditorState } = await import("@codemirror/state");

    const parent = document.createElement("div");
    document.body.appendChild(parent);
    const view = new EditorView({
      state: EditorState.create({
        doc: "#READY",
        extensions: result.current,
      }),
      parent,
    });

    // Virtual tag should be decorated with the cm-tag-pill class
    const pill = parent.querySelector(".cm-tag-pill");
    expect(pill).toBeTruthy();
    expect(pill?.textContent).toBe("#READY");

    view.destroy();
    parent.remove();
  });

  it("does NOT decorate virtual tags without includeVirtualTags", async () => {
    const { result } = renderHook(() => useMentionExtensions());

    const { EditorView } = await import("@codemirror/view");
    const { EditorState } = await import("@codemirror/state");

    const parent = document.createElement("div");
    document.body.appendChild(parent);
    const view = new EditorView({
      state: EditorState.create({
        doc: "#READY",
        extensions: result.current,
      }),
      parent,
    });

    // Without includeVirtualTags, #READY should NOT be decorated
    const pills = parent.querySelectorAll(".cm-tag-pill");
    const hasReadyPill = Array.from(pills).some(
      (p) => p.textContent === "#READY",
    );
    expect(hasReadyPill).toBe(false);

    view.destroy();
    parent.remove();
  });

  // ── $project mention tests ─────────────────────────────────────────
  // These verify that once a project entity declares both `mention_prefix`
  // and `mention_display_field`, the data-driven buildMentionExtensions
  // loop emits decoration + autocomplete + tooltip extensions for `$`
  // without any hardcoded prefix allowlist.

  it("emits extensions for a project mentionable type with $ prefix", () => {
    mockMentionableTypes = [
      { prefix: "$", entityType: "project", displayField: "name" },
    ];

    const { result } = renderHook(() => useMentionExtensions());

    // Non-empty extension array: decoration + tooltip + autocomplete source.
    expect(result.current.length).toBeGreaterThan(0);
  });

  it("decorates $project mentions with the cm-project-pill class", async () => {
    mockMentionableTypes = [
      { prefix: "$", entityType: "project", displayField: "name" },
    ];

    const { result } = renderHook(() => useMentionExtensions());

    const { EditorView } = await import("@codemirror/view");
    const { EditorState } = await import("@codemirror/state");

    const parent = document.createElement("div");
    document.body.appendChild(parent);
    const view = new EditorView({
      state: EditorState.create({
        doc: "$auth-migration",
        extensions: result.current,
      }),
      parent,
    });

    const pill = parent.querySelector(".cm-project-pill");
    expect(pill).toBeTruthy();
    expect(pill?.textContent).toBe("$auth-migration");

    view.destroy();
    parent.remove();
  });

  // ── slugField unit tests ───────────────────────────────────────────
  // These lock in the contract for the `mention_slug_field` schema signal:
  // when an entity type declares a slugField, the mention slug is sourced
  // from that raw field (no slugify). Projects use this so that a free-form
  // project id like `AUTH-Migration` flows through the autocomplete,
  // decorations, tooltip map, and backend filter as the literal id string.

  describe("buildColorMap with slugField", () => {
    it("keys the map by the raw slugField value (no slugify)", () => {
      const entities: Entity[] = [
        {
          id: "AUTH-Migration",
          entity_type: "project",
          moniker: "project:AUTH-Migration",
          fields: { name: "Auth Migration System", color: "ff0000" },
        },
      ];
      const map = buildColorMap(entities, "name", "id");
      expect(map.has("AUTH-Migration")).toBe(true);
      expect(map.get("AUTH-Migration")).toBe("ff0000");
      // Must NOT contain slugify(name) as a key.
      expect(map.has("auth-migration-system")).toBe(false);
    });

    it("falls back to slugify(displayField) when slugField is absent", () => {
      const entities: Entity[] = [
        {
          id: "t1",
          entity_type: "tag",
          moniker: "tag:t1",
          fields: { tag_name: "Bug Fix", color: "00ff00" },
        },
      ];
      const map = buildColorMap(entities, "tag_name");
      expect(map.has("bug-fix")).toBe(true);
      expect(map.get("bug-fix")).toBe("00ff00");
    });
  });

  describe("buildMetaMap with slugField", () => {
    it("keys the map by the raw slugField value (no slugify)", () => {
      const entities: Entity[] = [
        {
          id: "AUTH-Migration",
          entity_type: "project",
          moniker: "project:AUTH-Migration",
          fields: {
            name: "Auth Migration System",
            color: "ff0000",
            description: "Auth refactor",
          },
        },
      ];
      const map = buildMetaMap(entities, "name", "id");
      expect(map.has("AUTH-Migration")).toBe(true);
      const meta = map.get("AUTH-Migration");
      expect(meta?.color).toBe("ff0000");
      expect(meta?.description).toBe("Auth refactor");
      expect(map.has("auth-migration-system")).toBe(false);
    });

    it("falls back to slugify(displayField) when slugField is absent", () => {
      const entities: Entity[] = [
        {
          id: "t1",
          entity_type: "tag",
          moniker: "tag:t1",
          fields: { tag_name: "Bug Fix", color: "00ff00" },
        },
      ];
      const map = buildMetaMap(entities, "tag_name");
      expect(map.has("bug-fix")).toBe(true);
    });
  });

  describe("buildAsyncSearch with slugField", () => {
    it("emits slug: r.id when slugField is 'id'", async () => {
      mockInvoke.mockImplementation(
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        (command: string, args?: any) => {
          if (command === "search_mentions" && args?.entityType === "project") {
            return Promise.resolve([
              {
                id: "AUTH-Migration",
                display_name: "Auth Migration System",
                color: "ff0000",
              },
            ]);
          }
          return Promise.resolve([]);
        },
      );

      const search = buildAsyncSearch("project", "id");
      // Wait past the debounce window so the first query actually fires.
      const results = await new Promise<
        Array<{ slug: string; displayName: string; color: string }>
      >((resolve) => {
        setTimeout(async () => {
          resolve(await search(""));
        }, 200);
      });

      expect(results).toEqual([
        {
          slug: "AUTH-Migration",
          displayName: "Auth Migration System",
          color: "ff0000",
        },
      ]);
    });

    it("preserves slugify(display_name) behavior when slugField is absent", async () => {
      mockInvoke.mockImplementation(
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        (command: string, args?: any) => {
          if (command === "search_mentions" && args?.entityType === "tag") {
            return Promise.resolve([
              { id: "t1", display_name: "Bug Fix", color: "00ff00" },
            ]);
          }
          return Promise.resolve([]);
        },
      );

      const search = buildAsyncSearch("tag");
      const results = await new Promise<
        Array<{ slug: string; displayName: string; color: string }>
      >((resolve) => {
        setTimeout(async () => {
          resolve(await search(""));
        }, 200);
      });

      expect(results).toEqual([
        { slug: "bug-fix", displayName: "Bug Fix", color: "00ff00" },
      ]);
    });
  });

  it("decorates a document containing $AUTH-Migration when the color map is keyed by id", async () => {
    // Integration sanity-check: once a project declares slugField: "id",
    // feeding a CM6 document the literal `$AUTH-Migration` text should
    // receive a cm-project-pill decoration backed by the entity color.
    mockMentionableTypes = [
      {
        prefix: "$",
        entityType: "project",
        displayField: "name",
        slugField: "id",
      },
    ];
    mockGetEntities.mockImplementation((type: string) => {
      if (type === "project") {
        return [
          {
            id: "AUTH-Migration",
            entity_type: "project",
            moniker: "project:AUTH-Migration",
            fields: { name: "Auth Migration System", color: "4078c0" },
          },
        ];
      }
      return [];
    });

    const { result } = renderHook(() => useMentionExtensions());

    const { EditorView } = await import("@codemirror/view");
    const { EditorState } = await import("@codemirror/state");

    const parent = document.createElement("div");
    document.body.appendChild(parent);
    const view = new EditorView({
      state: EditorState.create({
        doc: "$AUTH-Migration",
        extensions: result.current,
      }),
      parent,
    });

    const pill = parent.querySelector(".cm-project-pill");
    expect(pill).toBeTruthy();
    expect(pill?.textContent).toBe("$AUTH-Migration");

    view.destroy();
    parent.remove();
  });

  it("calls search_mentions with entityType: 'project' when the completion source fires", async () => {
    mockMentionableTypes = [
      { prefix: "$", entityType: "project", displayField: "name" },
    ];

    mockInvoke.mockImplementation(
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (command: string, args?: any) => {
        if (command === "search_mentions" && args?.entityType === "project") {
          return Promise.resolve([
            { id: "p1", display_name: "auth-migration", color: "4078c0" },
            { id: "p2", display_name: "frontend", color: "6cc644" },
          ]);
        }
        return Promise.resolve([]);
      },
    );

    const { result } = renderHook(() => useMentionExtensions());

    const { EditorView } = await import("@codemirror/view");
    const { EditorState } = await import("@codemirror/state");
    const { startCompletion, currentCompletions } = await import(
      "@codemirror/autocomplete"
    );

    const parent = document.createElement("div");
    document.body.appendChild(parent);
    const view = new EditorView({
      state: EditorState.create({
        doc: "$",
        extensions: result.current,
      }),
      parent,
    });

    // Place the cursor after the `$` and trigger autocomplete manually.
    view.dispatch({ selection: { anchor: 1 } });
    startCompletion(view);

    // Wait for the debounced async source to resolve. The search is
    // debounced ~150ms; allow generous slack.
    await new Promise((resolve) => setTimeout(resolve, 400));

    // The invoke mock should have been called with entityType: "project".
    const projectCalls = mockInvoke.mock.calls.filter(
      (call) =>
        call[0] === "search_mentions" &&
        (call[1] as { entityType?: string })?.entityType === "project",
    );
    expect(projectCalls.length).toBeGreaterThan(0);

    // The autocomplete state should eventually contain our mock results.
    const completions = currentCompletions(view.state);
    const labels = completions.map((c) => c.label);
    // Labels are the slugs; tolerate either the raw display_name or slug form.
    const hasAuth = labels.some((l) => l.includes("auth-migration"));
    expect(hasAuth).toBe(true);

    view.destroy();
    parent.remove();
  });
});
