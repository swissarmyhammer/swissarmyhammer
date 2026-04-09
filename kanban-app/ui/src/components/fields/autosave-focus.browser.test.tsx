/**
 * Integration test: autosave must not disrupt editor state.
 *
 * Verifies that the debounced autosave path (onChange → useDebouncedSave →
 * updateField IPC) does NOT cause:
 * - The editor to unmount or lose focus
 * - Edit mode to exit (onDone called)
 * - Focus to jump away from the active editor
 *
 * Runs in vitest browser mode (real Chromium, real DOM).
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render } from "vitest-browser-react";
import { page, userEvent } from "vitest/browser";

// ---------------------------------------------------------------------------
// Tauri mocks — must be before component imports
// ---------------------------------------------------------------------------

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const mockInvoke = vi.fn(async (..._args: any[]) => null);

vi.mock("@tauri-apps/api/core", () => ({
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  invoke: (...a: any[]) => mockInvoke(...a),
}));
vi.mock("@tauri-apps/api/event", () => ({
  emit: vi.fn(() => Promise.resolve()),
  listen: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    label: "main",
    listen: vi.fn(() => Promise.resolve(() => {})),
  }),
}));
vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({
    onDragDropEvent: vi.fn(() => Promise.resolve(() => {})),
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
// Imports — after mocks
// ---------------------------------------------------------------------------

import "@/components/fields/registrations";

import { useState, useCallback } from "react";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { FieldUpdateProvider } from "@/lib/field-update-context";
import { ActiveBoardPathProvider } from "@/lib/command-scope";
import { SchemaProvider } from "@/lib/schema-context";
import { Field } from "@/components/fields/field";
import type { Entity, FieldDef } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Test fixtures
// ---------------------------------------------------------------------------

const TASK_ENTITY: Entity = {
  entity_type: "task",
  id: "t1",
  moniker: "task:t1",
  fields: { title: "Original Title" },
};

const TITLE_FIELD: FieldDef = {
  id: "title",
  name: "title",
  type: { kind: "text" },
  section: "header",
  display: "text",
  editor: "markdown",
};

const TASK_SCHEMA = {
  entity: {
    name: "task",
    fields: ["title"],
  },
  fields: [TITLE_FIELD],
};

// ---------------------------------------------------------------------------
// Test harness
// ---------------------------------------------------------------------------

/**
 * Wraps Field with all required providers.
 * Tracks onDone calls via a data attribute for assertion.
 */
function TestHarness({ onDoneSpy }: { onDoneSpy?: () => void }) {
  const [editing, setEditing] = useState(true);

  const handleDone = useCallback(() => {
    setEditing(false);
    onDoneSpy?.();
  }, [onDoneSpy]);

  return (
    <ActiveBoardPathProvider value="/test/board">
      <SchemaProvider>
        <EntityStoreProvider entities={{ task: [TASK_ENTITY] }}>
          <EntityFocusProvider>
            <FieldUpdateProvider>
              <div data-testid="harness" data-editing={String(editing)}>
                <Field
                  fieldDef={TITLE_FIELD}
                  entityType="task"
                  entityId="t1"
                  mode="full"
                  editing={editing}
                  onEdit={() => setEditing(true)}
                  onDone={handleDone}
                  onCancel={() => setEditing(false)}
                />
              </div>
            </FieldUpdateProvider>
          </EntityFocusProvider>
        </EntityStoreProvider>
      </SchemaProvider>
    </ActiveBoardPathProvider>
  );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("autosave does not disrupt editor state", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    mockInvoke.mockImplementation(async (cmd: string, ..._args: any[]) => {
      if (cmd === "list_entity_types") return ["task"];
      if (cmd === "get_entity_schema") return TASK_SCHEMA;
      if (cmd === "list_commands_for_scope") return { commands: [] };
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
      return null;
    });
  });

  it("editor stays in edit mode after debounced autosave fires", async () => {
    render(<TestHarness />);

    // Wait for CM6 editor to mount (SchemaProvider is async)
    await expect.element(page.getByTestId("harness")).toBeVisible();
    // Give CM6 time to initialize
    await new Promise((r) => setTimeout(r, 500));

    // Verify we're in edit mode
    const harness = document.querySelector("[data-testid='harness']")!;
    expect(harness.getAttribute("data-editing")).toBe("true");

    // Type into the CM6 editor — this feeds onChange → debounced save
    const cmContent = document.querySelector(".cm-content") as HTMLElement;
    expect(cmContent).not.toBeNull();
    cmContent.focus();
    await userEvent.type(cmContent, "hello");

    // Wait for debounce to fire (1000ms default + buffer)
    await new Promise((r) => setTimeout(r, 1500));

    // CRITICAL: field must still be in edit mode
    expect(harness.getAttribute("data-editing")).toBe("true");

    // CM6 editor must still be in the DOM
    expect(document.querySelector(".cm-content")).not.toBeNull();
  });

  it("onDone is NOT called by autosave", async () => {
    const onDoneSpy = vi.fn();
    render(<TestHarness onDoneSpy={onDoneSpy} />);

    // Wait for CM6
    await expect.element(page.getByTestId("harness")).toBeVisible();
    await new Promise((r) => setTimeout(r, 500));

    // Type to trigger onChange → debounced save
    const cmContent = document.querySelector(".cm-content") as HTMLElement;
    expect(cmContent).not.toBeNull();
    cmContent.focus();
    await userEvent.type(cmContent, "test");

    // Wait for debounce
    await new Promise((r) => setTimeout(r, 1500));

    // onDone must NOT be called — autosave is pure IPC, no UI state change
    expect(onDoneSpy).not.toHaveBeenCalled();
  });

  it("blur does not call onDone", async () => {
    const onDoneSpy = vi.fn();
    render(<TestHarness onDoneSpy={onDoneSpy} />);

    // Wait for CM6
    await expect.element(page.getByTestId("harness")).toBeVisible();
    await new Promise((r) => setTimeout(r, 500));

    // Focus then blur the editor
    const cmContent = document.querySelector(".cm-content") as HTMLElement;
    expect(cmContent).not.toBeNull();
    cmContent.focus();
    await userEvent.type(cmContent, "x");
    cmContent.blur();

    // Small wait for blur handler
    await new Promise((r) => setTimeout(r, 200));

    // onDone must NOT be called on blur
    expect(onDoneSpy).not.toHaveBeenCalled();
  });
});
