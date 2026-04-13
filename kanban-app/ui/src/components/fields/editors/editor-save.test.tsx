/**
 * Data-driven test harness for Field save behavior.
 *
 * Loads the REAL field definitions from YAML via vitest browser commands
 * (server-side Node.js), renders <Field> through real providers
 * (FieldUpdateProvider, EntityStoreProvider, SchemaProvider), and asserts
 * that invoke("dispatch_command") is called on save-worthy exit paths.
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
import { commands } from "vitest/browser";
import type { FieldDef } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Module-scoped fixture data — populated in beforeAll via browser commands
// ---------------------------------------------------------------------------

/** All field definitions loaded from builtin YAML. */
let allFieldDefs: FieldDef[] = [];

/** Entity definitions keyed by entity type. */
const entityDefs = new Map<
  string,
  { name: string; fields: string[]; body_field?: string }
>();

/** Get the FieldDefs for an entity type, filtered to editable, visible fields. */
function editableFieldsFor(entityType: string): FieldDef[] {
  const entityDef = entityDefs.get(entityType);
  if (!entityDef) return [];
  const defMap = new Map(allFieldDefs.map((d) => [d.name, d]));

  return entityDef.fields
    .map((name) => defMap.get(name))
    .filter(
      (d): d is FieldDef =>
        d !== undefined &&
        d.editor !== "none" &&
        d.editor !== undefined &&
        d.editor !== "attachment" &&
        d.section !== "hidden",
    );
}

/** Get ALL FieldDefs for an entity type (excludes display: none fields). */
function allFieldsFor(entityType: string): FieldDef[] {
  const entityDef = entityDefs.get(entityType);
  if (!entityDef) return [];
  const defMap = new Map(allFieldDefs.map((d) => [d.name, d]));

  return entityDef.fields
    .map((name) => defMap.get(name))
    .filter((d): d is FieldDef => d !== undefined && d.display !== "none");
}

// ---------------------------------------------------------------------------
// Configurable keymap mode — swapped per test.
// ---------------------------------------------------------------------------
let KEYMAP_MODE = "cua";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const mockInvoke = vi.fn((...args: any[]) => {
  if (args[0] === "list_entity_types")
    return Promise.resolve(["task", "tag", "actor", "column", "board"]);
  if (args[0] === "get_entity_schema") {
    // Return real schema data for the requested entity type
    const entityType = args[1]?.entityType as string;
    const entityDef = entityDefs.get(entityType);
    if (entityDef) {
      const defMap = new Map(allFieldDefs.map((d) => [d.name, d]));
      const fields = entityDef.fields
        .map((name) => defMap.get(name))
        .filter((d): d is FieldDef => d !== undefined);
      return Promise.resolve({ entity: entityDef, fields });
    }
    return Promise.resolve({
      entity: { name: entityType, fields: [] },
      fields: [],
    });
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

// Preserve real exports (SERIALIZE_TO_IPC_FN, Resource, Channel, TauriEvent,
// …) so transitively-imported submodules like `window.js` / `dpi.js` can
// still resolve their re-exports. Only override `invoke` / `listen`.
vi.mock("@tauri-apps/api/core", async () => {
  const actual =
    await vi.importActual<typeof import("@tauri-apps/api/core")>(
      "@tauri-apps/api/core",
    );
  return {
    ...actual,
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    invoke: (...args: any[]) => mockInvoke(...args),
  };
});
vi.mock("@tauri-apps/api/event", async () => {
  const actual =
    await vi.importActual<typeof import("@tauri-apps/api/event")>(
      "@tauri-apps/api/event",
    );
  return {
    ...actual,
    listen: vi.fn(() => Promise.resolve(() => {})),
  };
});
// `window-container.tsx` calls `getCurrentWindow()` at module-load time.
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
  moniker: "task:test-task-1",
  fields: {
    title: "Test Title",
    body: "Test body with #tag",
    tags: ["bug"],
    assignees: ["actor-1"],
    depends_on: [],
    progress: { total: 2, completed: 1, percent: 50 },
    position_column: "todo",
    position_ordinal: "ffff8000",
    virtual_tags: ["READY"],
    status_date: { kind: "created", timestamp: "2026-04-10T00:00:00Z" },
    due: "2026-05-01",
    scheduled: "2026-04-20",
  },
};

const TEST_ENTITIES: Record<string, Entity[]> = {
  task: [TEST_ENTITY],
  tag: [
    {
      entity_type: "tag",
      id: "tag-1",
      moniker: "tag:tag-1",
      fields: { tag_name: "bug", color: "ff0000" },
    },
  ],
  actor: [
    {
      entity_type: "actor",
      id: "actor-1",
      moniker: "actor:actor-1",
      fields: { name: "Alice", color: "0000ff" },
    },
  ],
  column: [
    {
      entity_type: "column",
      id: "todo",
      moniker: "column:todo",
      fields: { name: "Todo", order: 0 },
    },
  ],
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const keymapModes = ["cua", "vim", "emacs"] as const;
const exitPaths = ["blur", "Enter", "Escape"] as const;
const modes = ["compact", "full"] as const;

/** Wait for async effects to settle. */
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
// Load fixture data from YAML via browser commands (Node.js server-side)
// ---------------------------------------------------------------------------

beforeAll(async () => {
  allFieldDefs = (await commands.loadFieldDefinitions()) as unknown as FieldDef[];

  const entityTypes = ["task", "tag", "actor", "column", "board"];
  for (const et of entityTypes) {
    const def = (await commands.loadEntityDefinition({ entityType: et })) as {
      name: string;
      fields: string[];
      body_field?: string;
    };
    entityDefs.set(et, def);
  }

  // Sanity check
  const editable = editableFieldsFor("task");
  if (editable.length === 0) {
    throw new Error(
      "No editable fields found for task entity. Check builtin YAML definitions.",
    );
  }
  console.log(
    `Loaded ${allFieldDefs.length} field defs, ${editable.length} editable task fields: ${editable.map((f) => `${f.name} (${f.editor})`).join(", ")}`,
  );
});

// ---------------------------------------------------------------------------
// The matrix — iterates over fields inside each test, since field data
// is loaded asynchronously and can't be used with describe.each.
// ---------------------------------------------------------------------------

describe("Field save behavior", () => {
  describe.each(modes)("mode: %s", (mode) => {
    describe.each(keymapModes)("keymap: %s", (keymap) => {
      beforeEach(() => {
        vi.clearAllMocks();
        KEYMAP_MODE = keymap;
      });

      // One matrix cell is a known gap: multi-select + vim + Enter. The CM6
      // vim keymap handles Enter in the capture phase before our submit
      // extension fires, so the harness's native KeyboardEvent dispatch
      // misses the commit path. Advertised below via `it.skip` so the test
      // runner reports it as skipped rather than silently dropped.
      const skipMultiSelect = keymap === "vim";

      it.each(exitPaths)("exit: %s", async (exit) => {
        const editableFields = editableFieldsFor("task").filter(
          (f) =>
            !(skipMultiSelect && exit === "Enter" && f.editor === "multi-select"),
        );
        const failures: string[] = [];

        for (const fieldDef of editableFields) {
          const { container, unmount } = renderField(fieldDef, mode, true);
          await settle();
          mockInvoke.mockClear();

          // Popover-based editors (date) render their CM6 input inside a
          // Radix portal attached to document.body. The save/cancel path
          // runs through the PopoverContent's Escape handler and the
          // Popover.onOpenChange commit. Drive them through that native
          // surface instead of the generic contenteditable target.
          if (fieldDef.editor === "date") {
            const popover = document.body.querySelector<HTMLElement>(
              "[data-slot='popover-content'], [data-radix-popper-content-wrapper]",
            );
            const cm = popover?.querySelector<HTMLElement>(".cm-content");
            if (exit === "blur") {
              // Real UX: user clicks outside → Popover.onOpenChange(false)
              // → commitResolved fires and saves the currently-resolved
              // value. Simulate by firing pointerdown on document.body
              // outside the popover.
              await act(async () => {
                document.body.dispatchEvent(
                  new PointerEvent("pointerdown", {
                    bubbles: true,
                    pointerType: "mouse",
                  }),
                );
              });
              await settle(50);
              unmount();
              await settle(50);
            } else if (cm) {
              // Enter/Escape go through the CM6 keymap, which listens on
              // the contentDOM. Dispatch there so CM's keymap fires first;
              // for Escape the event then bubbles to Radix DismissableLayer
              // (closing the popover) but DateEditor's cancel/commit refs
              // will have already run and set committedRef, so the
              // onOpenChange → commitResolved chain short-circuits.
              await act(async () => cm.focus());
              await act(async () =>
                cm.dispatchEvent(
                  new KeyboardEvent("keydown", {
                    key: exit,
                    bubbles: true,
                    cancelable: true,
                  }),
                ),
              );
              await settle();
              unmount();
            } else {
              unmount();
            }
          } else {
            const target =
              container.querySelector<HTMLElement>(".cm-content") ??
              container.querySelector<HTMLElement>("input") ??
              container.querySelector<HTMLElement>("select") ??
              (container.firstElementChild as HTMLElement | null);

            if (target) {
              if (exit === "blur") {
                // In a real browser, ensure the element is focused first so
                // .blur() actually fires a blur event. Then unmount to flush
                // the debounced save (1000ms debounce would be too slow to wait).
                await act(async () => {
                  target.focus();
                });
                await act(async () => {
                  target.blur();
                  await new Promise((r) => setTimeout(r, 50));
                });
                // Unmount flushes pending debounced saves immediately
                unmount();
                await settle(50);
              } else {
                await act(async () => fireEvent.keyDown(target, { key: exit }));
                await settle();
                unmount();
              }
            } else {
              unmount();
            }
          }

          const shouldSave = expectsSave(keymap, exit);
          if (shouldSave) {
            const calls = getUpdateCalls();
            if (calls.length === 0) {
              failures.push(
                `${fieldDef.name} (${fieldDef.editor}): expected save but got none`,
              );
            } else {
              const args = calls[0][1].args;
              if (args.entity_type !== "task" || args.id !== "test-task-1") {
                failures.push(
                  `${fieldDef.name}: wrong entity identity in save call`,
                );
              }
            }
          } else {
            const calls = getUpdateCalls();
            if (calls.length > 0) {
              failures.push(
                `${fieldDef.name} (${fieldDef.editor}): expected NO save but got ${calls.length}`,
              );
            }
          }
        }

        expect(
          failures,
          `${mode} / ${keymap} / ${exit} failures:\n${failures.join("\n")}`,
        ).toHaveLength(0);
      });

      // Advertised skip — pairs with the `skipMultiSelect` filter above.
      // When the kanban card lands, delete this and the filter.
      if (skipMultiSelect) {
        it.skip(
          "exit: Enter × editor: multi-select — TODO(kanban:01KP2DQW57CAXBGC5GT68PFYPB) harness cannot drive CM6 vim capture-phase Enter",
          () => {},
        );
      }
    });
  });
});

// ---------------------------------------------------------------------------
// Display tests — every field type renders something in both modes
// ---------------------------------------------------------------------------

describe("Field display behavior", () => {
  describe.each(modes)("mode: %s", (mode) => {
    it("every field renders display content", async () => {
      KEYMAP_MODE = "cua";
      const allFields = allFieldsFor("task");
      const missing: string[] = [];

      for (const fieldDef of allFields) {
        const { container, unmount } = renderField(fieldDef, mode, false);
        await settle();

        if (!container.firstElementChild) {
          missing.push(`${fieldDef.name} (${fieldDef.display ?? "text"})`);
        }

        unmount();
      }

      expect(
        missing,
        `task / ${mode}: fields missing display: ${missing.join(", ")}`,
      ).toHaveLength(0);
    });
  });
});

// ---------------------------------------------------------------------------
// Per-entity tests — all fields on an entity render and edit correctly
// ---------------------------------------------------------------------------

describe("Entity field coverage", () => {
  describe.each(modes)("mode: %s", (mode) => {
    it("every field displays", async () => {
      KEYMAP_MODE = "cua";
      const fields = allFieldsFor("task");
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
        `task / ${mode}: fields missing display: ${missing.join(", ")}`,
      ).toHaveLength(0);
    });

    it("every editable field enters edit mode", async () => {
      KEYMAP_MODE = "cua";
      const editable = editableFieldsFor("task");
      const missing: string[] = [];

      for (const fieldDef of editable) {
        const { container, unmount } = renderField(fieldDef, mode, true);
        await settle();

        // Popover/Select-based editors (date, select) render their editor
        // surface inside a Radix portal attached to document.body, so we
        // must look there too — not just in the React render container.
        const hasEditor =
          container.querySelector(".cm-editor") ??
          container.querySelector("input") ??
          container.querySelector("select") ??
          container.querySelector("[role='combobox']") ??
          document.body.querySelector("[data-radix-popper-content-wrapper]") ??
          document.body.querySelector("[data-radix-portal]") ??
          document.body.querySelector(".cm-editor");

        if (!hasEditor) {
          missing.push(`${fieldDef.name} (${fieldDef.editor})`);
        }

        unmount();
      }

      expect(
        missing,
        `task / ${mode}: fields missing editor: ${missing.join(", ")}`,
      ).toHaveLength(0);
    });
  });
});
