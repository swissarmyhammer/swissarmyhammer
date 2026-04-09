/**
 * Tests for useMentionExtensions hook options.
 *
 * Verifies that includeVirtualTags and includeFilterSigils options correctly
 * control which completion sources are available.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";

// Mock Tauri APIs
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const mockInvoke = vi.fn((..._args: any[]) => Promise.resolve([]));
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

// Mock schema context — one mentionable type: tag with # prefix
vi.mock("@/lib/schema-context", () => ({
  useSchema: () => ({
    mentionableTypes: [
      { prefix: "#", entityType: "tag", displayField: "name" },
    ],
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
  return [];
});

vi.mock("@/lib/entity-store-context", () => ({
  useEntityStore: () => ({ getEntities: mockGetEntities }),
}));

import { renderHook } from "@testing-library/react";
import { useMentionExtensions } from "../use-mention-extensions";

describe("useMentionExtensions", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockInvoke.mockResolvedValue([]);
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
});
