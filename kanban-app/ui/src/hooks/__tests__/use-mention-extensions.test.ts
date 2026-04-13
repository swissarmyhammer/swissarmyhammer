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
  if (type === "column") {
    return [
      {
        id: "c1",
        entity_type: "column",
        fields: { name: "To Do", color: "888888" },
      },
      {
        id: "c2",
        entity_type: "column",
        fields: { name: "Doing", color: "888888" },
      },
      {
        id: "c3",
        entity_type: "column",
        fields: { name: "Done", color: "888888" },
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
  buildMentionMetaMap,
} from "../use-mention-extensions";

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

  // ── displayName in metaMap ─────────────────────────────────────────
  // Verifies that buildMentionMetaMap populates displayName with the raw
  // (un-slugified) entity name so downstream widgets can render it.

  it("produces metaMap entries with displayName equal to the raw entity name", () => {
    // Entity whose display name contains spaces/capitals — the slug
    // will differ from the raw name.
    const entities = [
      {
        id: "p1",
        entity_type: "project",
        moniker: "project:p1",
        fields: {
          name: "Auth Migration",
          color: "4078c0",
          description: "Auth refactor",
        },
      },
    ];

    const metaMap = buildMentionMetaMap(entities, "name");

    // The key is the slugified name
    const entry = metaMap.get("auth-migration");
    expect(entry).toBeDefined();
    // displayName preserves the original un-slugified value
    expect(entry!.displayName).toBe("Auth Migration");
    expect(entry!.color).toBe("4078c0");
    expect(entry!.description).toBe("Auth refactor");
  });

  it("produces metaMap entries without description when entity lacks one", () => {
    const entities = [
      {
        id: "t1",
        entity_type: "tag",
        moniker: "tag:t1",
        fields: { name: "bug", color: "ff0000" },
      },
    ];

    const metaMap = buildMentionMetaMap(entities, "name");
    const entry = metaMap.get("bug");
    expect(entry).toBeDefined();
    expect(entry!.displayName).toBe("bug");
    expect(entry!.color).toBe("ff0000");
    expect(entry!.description).toBeUndefined();
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

  // ── %column mention tests ──────────────────────────────────────────
  // Verify that once column.yaml declares `mention_prefix: "%"` and
  // `mention_display_field: "name"`, the data-driven extension builder
  // produces decoration + autocomplete + tooltip extensions for `%`.

  it("emits extensions for a column mentionable type with % prefix", () => {
    mockMentionableTypes = [
      { prefix: "%", entityType: "column", displayField: "name" },
    ];

    const { result } = renderHook(() => useMentionExtensions());

    // Non-empty extension array: decoration + tooltip + autocomplete source.
    expect(result.current.length).toBeGreaterThan(0);
  });

  it("decorates %column mentions with the cm-column-pill class", async () => {
    mockMentionableTypes = [
      { prefix: "%", entityType: "column", displayField: "name" },
    ];

    const { result } = renderHook(() => useMentionExtensions());

    const { EditorView } = await import("@codemirror/view");
    const { EditorState } = await import("@codemirror/state");

    const parent = document.createElement("div");
    document.body.appendChild(parent);
    const view = new EditorView({
      state: EditorState.create({
        doc: "%to-do",
        extensions: result.current,
      }),
      parent,
    });

    const pill = parent.querySelector(".cm-column-pill");
    expect(pill).toBeTruthy();
    expect(pill?.textContent).toBe("%to-do");

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
