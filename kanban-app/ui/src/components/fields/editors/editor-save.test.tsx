/**
 * Data-driven test harness for Field save behavior.
 *
 * Loads the REAL field definitions from YAML, renders <Field> through real
 * providers (FieldUpdateProvider, EntityStoreProvider, SchemaProvider),
 * and asserts that invoke("dispatch_command") is called on save-worthy
 * exit paths.
 *
 * Matrix: editable fields × keymap modes × exit paths × modes
 *
 * Expected behavior:
 *   blur   → always saves
 *   Enter  → always saves
 *   Escape → vim saves, CUA/emacs discards
 */

import { describe, it, expect, vi, beforeEach, beforeAll } from "vitest";
import { render, fireEvent, act } from "@testing-library/react";
import fs from "node:fs";
import path from "node:path";
import yaml from "js-yaml";
import type { FieldDef } from "@/types/kanban";

// ---------------------------------------------------------------------------
// jsdom stubs
// ---------------------------------------------------------------------------
Element.prototype.scrollIntoView = vi.fn();

// ---------------------------------------------------------------------------
// Load REAL field definitions from YAML — the source of truth.
// ---------------------------------------------------------------------------

const DEFINITIONS_DIR = path.resolve(
  __dirname,
  "../../../../../../swissarmyhammer-kanban/builtin/fields/definitions",
);
const ENTITIES_DIR = path.resolve(
  __dirname,
  "../../../../../../swissarmyhammer-kanban/builtin/fields/entities",
);

/** Load a YAML file and return its parsed content. */
function loadYaml<T>(filePath: string): T {
  return yaml.load(fs.readFileSync(filePath, "utf8")) as T;
}

/** Load all field definitions from the builtin YAML. */
function loadAllFieldDefs(): FieldDef[] {
  const files = fs
    .readdirSync(DEFINITIONS_DIR)
    .filter((f: string) => f.endsWith(".yaml"));
  return files.map((f: string) =>
    loadYaml<FieldDef>(path.join(DEFINITIONS_DIR, f)),
  );
}

/** Load an entity definition to get its field list. */
function loadEntityDef(entityType: string): {
  name: string;
  fields: string[];
  body_field?: string;
} {
  return loadYaml(path.join(ENTITIES_DIR, `${entityType}.yaml`));
}

/** Get the FieldDefs for an entity type, filtered to editable, visible fields. */
function editableFieldsFor(entityType: string): FieldDef[] {
  const entityDef = loadEntityDef(entityType);
  const allDefs = loadAllFieldDefs();
  const defMap = new Map(allDefs.map((d) => [d.name, d]));

  return entityDef.fields
    .map((name) => defMap.get(name))
    .filter(
      (d): d is FieldDef =>
        d !== undefined &&
        d.editor !== "none" &&
        d.editor !== undefined &&
        d.editor !== "attachment" && // attachment editor uses button-based saves, not blur/Enter/Escape
        d.section !== "hidden",
    );
}

/** Get ALL FieldDefs for an entity type. */
function allFieldsFor(entityType: string): FieldDef[] {
  const entityDef = loadEntityDef(entityType);
  const allDefs = loadAllFieldDefs();
  const defMap = new Map(allDefs.map((d) => [d.name, d]));

  return entityDef.fields
    .map((name) => defMap.get(name))
    .filter((d): d is FieldDef => d !== undefined);
}

// ---------------------------------------------------------------------------
// Configurable keymap mode — swapped per test.
// ---------------------------------------------------------------------------
let KEYMAP_MODE = "cua";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const mockInvoke = vi.fn((...args: any[]) => {
  if (args[0] === "list_entity_types")
    return Promise.resolve([
      "task",
      "tag",
      "actor",
      "column",
      "swimlane",
      "board",
    ]);
  if (args[0] === "get_entity_schema") {
    // Return real schema data for the requested entity type
    const entityType = args[1]?.entityType as string;
    try {
      const entityDef = loadEntityDef(entityType);
      const allDefs = loadAllFieldDefs();
      const defMap = new Map(allDefs.map((d) => [d.name, d]));
      const fields = entityDef.fields
        .map((name) => defMap.get(name))
        .filter((d): d is FieldDef => d !== undefined);
      return Promise.resolve({ entity: entityDef, fields });
    } catch {
      return Promise.resolve({
        entity: { name: entityType, fields: [] },
        fields: [],
      });
    }
  }
  if (args[0] === "get_ui_state")
    return Promise.resolve({
      palette_open: false,
      palette_mode: "command",
      keymap_mode: KEYMAP_MODE,
      scope_chain: [],
      open_boards: [],
      windows: {},
      recent_boards: [],
    });
  if (args[0] === "dispatch_command")
    return Promise.resolve({ result: "ok", undoable: true });
  return Promise.resolve(null);
});

vi.mock("@tauri-apps/api/core", () => ({
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  invoke: (...args: any[]) => mockInvoke(...args),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock("@tauri-apps/plugin-log", () => ({
  error: vi.fn(),
  warn: vi.fn(),
  info: vi.fn(),
  debug: vi.fn(),
  trace: vi.fn(),
  attachConsole: vi.fn(() => Promise.resolve()),
}));
vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({
    onDragDropEvent: vi.fn(() => Promise.resolve(() => {})),
  }),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(() => Promise.resolve(null)),
}));

// ---------------------------------------------------------------------------
// Imports AFTER mocks — NO mock of useFieldUpdate, we use the real one
// ---------------------------------------------------------------------------
// Field type registrations — must be imported so editors/displays are registered
import "@/components/fields/registrations";

import { UIStateProvider } from "@/lib/ui-state-context";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";

import { FieldUpdateProvider } from "@/lib/field-update-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import { Field } from "@/components/fields/field";
import { FileDropProvider } from "@/lib/file-drop-context";
import type { Entity } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Test entity — has a value for every possible field
// ---------------------------------------------------------------------------
const TEST_ENTITY: Entity = {
  entity_type: "task",
  id: "test-task-1",
  fields: {
    title: "Test Title",
    body: "Test body with #tag",
    tags: ["bug"],
    assignees: ["actor-1"],
    depends_on: [],
    progress: { total: 2, completed: 1, percent: 50 },
    position_column: "todo",
    position_swimlane: "default",
    position_ordinal: "ffff8000",
  },
};

const TEST_ENTITIES: Record<string, Entity[]> = {
  task: [TEST_ENTITY],
  tag: [
    {
      entity_type: "tag",
      id: "tag-1",
      fields: { tag_name: "bug", color: "ff0000" },
    },
  ],
  actor: [
    {
      entity_type: "actor",
      id: "actor-1",
      fields: { name: "Alice", color: "0000ff" },
    },
  ],
  column: [
    { entity_type: "column", id: "todo", fields: { name: "Todo", order: 0 } },
  ],
  swimlane: [
    {
      entity_type: "swimlane",
      id: "default",
      fields: { name: "Default", order: 0 },
    },
  ],
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async function settle(ms = 150) {
  await act(async () => {
    await new Promise((r) => setTimeout(r, ms));
  });
}

/** Render a Field inside all required providers — real context, no mocked hooks. */
function renderField(
  fieldDef: FieldDef,
  mode: "compact" | "full",
  editing: boolean,
) {
  const onEdit = vi.fn();
  const onDone = vi.fn();
  const onCancel = vi.fn();

  const result = render(
    <TooltipProvider>
      <FileDropProvider>
        <SchemaProvider>
          <EntityStoreProvider entities={TEST_ENTITIES}>
            <EntityFocusProvider>
              <FieldUpdateProvider>
                <UIStateProvider>
                  <Field
                    fieldDef={fieldDef}
                    entityType="task"
                    entityId="test-task-1"
                    mode={mode}
                    editing={editing}
                    onEdit={onEdit}
                    onDone={onDone}
                    onCancel={onCancel}
                  />
                </UIStateProvider>
              </FieldUpdateProvider>
            </EntityFocusProvider>
          </EntityStoreProvider>
        </SchemaProvider>
      </FileDropProvider>
    </TooltipProvider>,
  );

  return { ...result, onEdit, onDone, onCancel };
}

/** Get dispatch_command calls for entity.update_field. */
function getUpdateCalls() {
  return mockInvoke.mock.calls.filter(
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (c: any[]) =>
      c[0] === "dispatch_command" && c[1]?.cmd === "entity.update_field",
  );
}

/** Should updateField be called for this combination? */
function expectsSave(keymap: string, exit: string): boolean {
  if (exit === "blur") return true;
  if (exit === "Enter") return true;
  if (exit === "Escape") return keymap === "vim";
  return false;
}

// ---------------------------------------------------------------------------
// Load the test matrix from real YAML
// ---------------------------------------------------------------------------

const editableFields = editableFieldsFor("task");
const allFields = allFieldsFor("task");
const keymapModes = ["cua", "vim", "emacs"] as const;
const exitPaths = ["blur", "Enter", "Escape"] as const;
const modes = ["compact", "full"] as const;

// Sanity check — if no editable fields loaded, something is wrong
if (editableFields.length === 0) {
  throw new Error(
    `No editable fields found for task entity. Check YAML at ${DEFINITIONS_DIR}`,
  );
}

// ---------------------------------------------------------------------------
// The matrix
// ---------------------------------------------------------------------------

describe("Field save behavior", () => {
  // Log what we're testing
  beforeAll(() => {
    const names = editableFields.map((f) => `${f.name} (${f.editor})`);
    console.log(`Testing ${names.length} editable fields: ${names.join(", ")}`);
  });

  describe.each(
    editableFields.map((f) => ({
      fieldDef: f,
      fieldName: f.name,
      editor: f.editor,
    })),
  )("field: $fieldName (editor: $editor)", ({ fieldDef }) => {
    describe.each(modes)("mode: %s", (mode) => {
      describe.each(keymapModes)("keymap: %s", (keymap) => {
        beforeEach(() => {
          vi.clearAllMocks();
          KEYMAP_MODE = keymap;
        });

        it.each(exitPaths)("exit: %s", async (exit) => {
          const { container, unmount } = renderField(fieldDef, mode, true);
          await settle();

          // Clear any calls from rendering / entering edit mode
          mockInvoke.mockClear();

          // TODO: each field type needs an interaction adapter to:
          //   1. Set a new value in the editor
          //   2. Find the right DOM target for exit events
          // For now, try to find any interactive element and trigger the exit.
          const target =
            container.querySelector(".cm-content") ??
            container.querySelector("input") ??
            container.querySelector("select") ??
            container.firstElementChild;

          if (target) {
            if (exit === "blur") {
              await act(async () => fireEvent.blur(target));
            } else {
              await act(async () => fireEvent.keyDown(target, { key: exit }));
            }
          }
          await settle();

          const shouldSave = expectsSave(keymap, exit);
          if (shouldSave) {
            const calls = getUpdateCalls();
            expect(
              calls.length,
              `${fieldDef.name} / ${mode} / ${keymap} / ${exit}: expected dispatch_command(entity.update_field) to be called`,
            ).toBeGreaterThanOrEqual(1);

            // Verify correct entity identity
            if (calls.length > 0) {
              const args = calls[0][1].args;
              expect(args.entity_type).toBe("task");
              expect(args.id).toBe("test-task-1");
              expect(args.field_name).toBe(fieldDef.name);
            }
          } else {
            const calls = getUpdateCalls();
            expect(
              calls.length,
              `${fieldDef.name} / ${mode} / ${keymap} / ${exit}: expected NO dispatch_command(entity.update_field)`,
            ).toBe(0);
          }

          unmount();
        });
      });
    });
  });
});

// ---------------------------------------------------------------------------
// Display tests — every field type renders something in both modes
// ---------------------------------------------------------------------------

describe("Field display behavior", () => {
  beforeAll(() => {
    const names = allFields.map((f) => `${f.name} (${f.display ?? "text"})`);
    console.log(`Testing ${names.length} field displays: ${names.join(", ")}`);
  });

  describe.each(
    allFields.map((f) => ({
      fieldDef: f,
      fieldName: f.name,
      display: f.display ?? "text",
    })),
  )("field: $fieldName (display: $display)", ({ fieldDef }) => {
    describe.each(modes)("mode: %s", (mode) => {
      it("renders display content", async () => {
        KEYMAP_MODE = "cua";
        const { container, unmount } = renderField(fieldDef, mode, false);
        await settle();

        // Field should render something — not be empty
        expect(
          container.innerHTML.length,
          `${fieldDef.name} / ${mode}: Field display should render content`,
        ).toBeGreaterThan(0);

        // Field should have produced actual visible content, not just a wrapper
        const inner = container.firstElementChild;
        expect(
          inner,
          `${fieldDef.name} / ${mode}: Field should render a DOM element`,
        ).toBeTruthy();

        unmount();
      });
    });
  });
});

// ---------------------------------------------------------------------------
// Per-entity tests — all fields on an entity render and edit correctly
// ---------------------------------------------------------------------------

const entityTypes = ["task"] as const; // extend as we add entity support

describe("Entity field coverage", () => {
  describe.each(entityTypes)("entity: %s", (entityType) => {
    const fields = allFieldsFor(entityType);
    const editable = editableFieldsFor(entityType);

    describe.each(modes)("mode: %s", (mode) => {
      it("every field displays", async () => {
        KEYMAP_MODE = "cua";
        const missing: string[] = [];

        for (const fieldDef of fields) {
          const { container, unmount } = renderField(fieldDef, mode, false);
          await settle();

          if (!container.firstElementChild) {
            missing.push(`${fieldDef.name} (${fieldDef.display ?? "text"})`);
          }

          unmount();
        }

        expect(
          missing,
          `${entityType} / ${mode}: fields missing display: ${missing.join(", ")}`,
        ).toHaveLength(0);
      });

      it("every editable field enters edit mode", async () => {
        KEYMAP_MODE = "cua";
        const missing: string[] = [];

        for (const fieldDef of editable) {
          const { container, unmount } = renderField(fieldDef, mode, true);
          await settle();

          const hasEditor =
            container.querySelector(".cm-editor") ??
            container.querySelector("input") ??
            container.querySelector("select") ??
            container.querySelector("[data-radix-popper-content-wrapper]");

          if (!hasEditor) {
            missing.push(`${fieldDef.name} (${fieldDef.editor})`);
          }

          unmount();
        }

        expect(
          missing,
          `${entityType} / ${mode}: fields missing editor: ${missing.join(", ")}`,
        ).toHaveLength(0);
      });
    });
  });
});
