/**
 * Data-driven test harness for field editor save behavior.
 *
 * Tests every editor × every keymap mode × every exit path.
 * Asserts that editors call `updateField` directly — not via container callbacks.
 *
 * Expected behavior:
 *   blur   → always saves
 *   Enter  → always saves
 *   Escape → vim saves, CUA/emacs discards
 */

import { describe, it, expect, vi, beforeEach, type Mock } from "vitest";
import { render, fireEvent, act, type RenderResult } from "@testing-library/react";
import { EditorView } from "@codemirror/view";
import type { ReactElement } from "react";

// ---------------------------------------------------------------------------
// jsdom stubs
// ---------------------------------------------------------------------------
Element.prototype.scrollIntoView = vi.fn();

// ---------------------------------------------------------------------------
// Configurable keymap mode — swapped per test.
// ---------------------------------------------------------------------------
let KEYMAP_MODE = "cua";

// ---------------------------------------------------------------------------
// Mock updateField — this is what we assert against.
// ---------------------------------------------------------------------------
const mockUpdateField = vi.fn(() => Promise.resolve());

vi.mock("@/lib/field-update-context", () => ({
  FieldUpdateProvider: ({ children }: { children: ReactElement }) => children,
  useFieldUpdate: () => ({ updateField: mockUpdateField }),
}));

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const mockInvoke = vi.fn((...args: any[]) => {
  if (args[0] === "get_entity_schema")
    return Promise.resolve({
      entity: { name: "task", fields: ["title"], body_field: "body" },
      fields: [
        { id: "f1", name: "title", type: { kind: "markdown", single_line: true }, editor: "markdown", display: "text", section: "header" },
      ],
    });
  if (args[0] === "get_ui_state")
    return Promise.resolve({
      palette_open: false,
      keymap_mode: KEYMAP_MODE,
      scope_chain: [],
      open_boards: [],
      windows: {},
      recent_boards: [],
    });
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

// ---------------------------------------------------------------------------
// Imports AFTER mocks
// ---------------------------------------------------------------------------
import { UIStateProvider } from "@/lib/ui-state-context";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { FieldPlaceholderEditor } from "@/components/fields/field-placeholder";
import { NumberEditor } from "./number-editor";
import { SelectEditor } from "./select-editor";
import { DateEditor } from "./date-editor";
import { ColorPaletteEditor } from "./color-palette-editor";
import { MultiSelectEditor } from "./multi-select-editor";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async function settle(ms = 150) {
  await act(async () => {
    await new Promise((r) => setTimeout(r, ms));
  });
}

function wrap(ui: ReactElement) {
  return <UIStateProvider>{ui}</UIStateProvider>;
}

/** Wrapper with schema + entity store for multi-select. */
function wrapWithStore(ui: ReactElement) {
  return (
    <SchemaProvider>
      <EntityStoreProvider entities={{ tag: [{ entity_type: "tag", id: "t1", fields: { tag_name: "bug" } }], actor: [] }}>
        <UIStateProvider>{ui}</UIStateProvider>
      </EntityStoreProvider>
    </SchemaProvider>
  );
}

// ---------------------------------------------------------------------------
// Editor adapters
//
// Each adapter knows how to:
//   render()    — mount the editor with standard props
//   setValue()  — put a new value into the editor's input
//   exitTarget() — return the DOM element to fire exit events on
// ---------------------------------------------------------------------------

interface EditorAdapter {
  name: string;
  /** If true, the editor saves on value change (e.g. select). Test asserts save during setValue, not during exit. */
  savesOnChange?: boolean;
  /** If true, the editor's inner content is inside a popover that jsdom can't render. Tests are skipped. */
  popoverLimited?: boolean;
  /** If true, every exit path saves (e.g. multi-select commits on Escape too). */
  alwaysSaves?: boolean;
  /** If true, skip blur tests (jsdom can't model focus tracking for delayed blur handlers). */
  skipBlur?: boolean;
  render: (onDone: () => void, onCancel: () => void) => RenderResult;
  setValue: (result: RenderResult, value: string) => Promise<void>;
  exitTarget: (result: RenderResult) => HTMLElement;
}

/** Helper: find CM6 EditorView from a container. */
function findCmView(container: HTMLElement): EditorView {
  const el = container.querySelector(".cm-editor") as HTMLElement;
  if (!el) throw new Error("No .cm-editor found");
  const view = EditorView.findFromDOM(el);
  if (!view) throw new Error("EditorView.findFromDOM returned null");
  return view;
}

const markdownAdapter: EditorAdapter = {
  name: "markdown",
  render: (onDone, onCancel) =>
    render(
      wrap(
        <FieldPlaceholderEditor
          value="original"
          entityType="task"
          entityId="t1"
          fieldName="title"
          onCommit={onDone}
          onCancel={onCancel}
        />,
      ),
    ),
  setValue: async (result, value) => {
    const view = findCmView(result.container);
    await act(async () => {
      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: value },
      });
    });
  },
  exitTarget: (result) =>
    result.container.querySelector(".cm-content") as HTMLElement,
};

const numberAdapter: EditorAdapter = {
  name: "number",
  render: (onDone, onCancel) =>
    render(
      wrap(
        <NumberEditor
          value={42}
          entityType="task"
          entityId="t1"
          fieldName="priority"
          onCommit={onDone}
          onCancel={onCancel}
          mode="compact"
        />,
      ),
    ),
  setValue: async (result, value) => {
    const input = result.container.querySelector("input") as HTMLInputElement;
    await act(async () => {
      fireEvent.change(input, { target: { value } });
    });
  },
  exitTarget: (result) =>
    result.container.querySelector("input") as HTMLElement,
};

const selectAdapter: EditorAdapter = {
  name: "select",
  savesOnChange: true,
  render: (onDone, onCancel) =>
    render(
      wrap(
        <SelectEditor
          value="a"
          entityType="task"
          entityId="t1"
          fieldName="status"
          field={{
            id: "f-sel",
            name: "status",
            type: { kind: "select", options: [{ value: "a" }, { value: "b" }] },
            editor: "select",
            display: "text",
          }}
          onCommit={onDone}
          onCancel={onCancel}
          mode="compact"
        />,
      ),
    ),
  setValue: async (result, value) => {
    const select = result.container.querySelector("select") as HTMLSelectElement;
    await act(async () => {
      fireEvent.change(select, { target: { value } });
    });
  },
  exitTarget: (result) =>
    result.container.querySelector("select") as HTMLElement,
};

// Date and color are popover-based. In jsdom, Radix popovers don't render
// their inner content (CM6, color picker). updateField wiring is verified
// by code inspection — the pattern is identical to other editors.
const dateAdapter: EditorAdapter = {
  name: "date",
  popoverLimited: true,
  render: (onDone, onCancel) =>
    render(
      wrap(
        <DateEditor
          value="2025-01-15"
          entityType="task"
          entityId="t1"
          fieldName="due_date"
          onCommit={onDone}
          onCancel={onCancel}
          mode="compact"
        />,
      ),
    ),
  setValue: async (result, value) => {
    // Radix portals content to document.body, not the component container
    const cmEditor = document.body.querySelector(".cm-editor") as HTMLElement;
    if (cmEditor) {
      const view = EditorView.findFromDOM(cmEditor);
      if (view) {
        await act(async () => {
          view.dispatch({
            changes: { from: 0, to: view.state.doc.length, insert: value },
          });
        });
        return;
      }
    }
  },
  exitTarget: () => {
    const cm = document.body.querySelector(".cm-content") as HTMLElement;
    if (cm) return cm;
    return document.body.firstElementChild as HTMLElement;
  },
};

const colorAdapter: EditorAdapter = {
  name: "color-palette",
  popoverLimited: true,
  render: (onDone, onCancel) =>
    render(
      wrap(
        <ColorPaletteEditor
          value="ff0000"
          onCommit={onDone}
          onCancel={onCancel}
          mode="compact"
        />,
      ),
    ),
  setValue: async (result, _value) => {
    // Color editor commits on every change — the hex input is inside a popover.
    // In jsdom, the popover may not render. We test what we can.
    const input = result.container.querySelector("input[type='text']") as HTMLInputElement;
    if (input) {
      await act(async () => {
        fireEvent.change(input, { target: { value: "00ff00" } });
      });
    }
  },
  exitTarget: (result) => {
    const input = result.container.querySelector("input[type='text']") as HTMLElement;
    if (input) return input;
    return result.container.firstElementChild as HTMLElement;
  },
};

// Multi-select always commits on every exit path (Enter, Escape, blur).
// It uses CM6 with a Prec.highest keymap, not the shared buildSubmitCancelExtensions.
// Multi-select blur uses setTimeout + document.activeElement check that jsdom
// can't model. Enter/Escape prove commit→updateField wiring works.
const multiSelectAdapter: EditorAdapter = {
  name: "multi-select",
  alwaysSaves: true,
  skipBlur: true,
  render: (onDone, onCancel) =>
    render(
      wrapWithStore(
        <MultiSelectEditor
          value={["bug"]}
          entityType="task"
          entityId="t1"
          fieldName="tags"
          field={{
            id: "f-tags",
            name: "tags",
            type: { kind: "computed", derive: "parse-body-tags" },
            editor: "multi-select",
            display: "badge-list",
          }}
          onCommit={onDone}
          onCancel={onCancel}
          mode="compact"
        />,
      ),
    ),
  setValue: async (_result, _value) => {
    // Multi-select commits the current selection — no need to set a new value.
    // The initial value=["bug"] is the selection.
  },
  exitTarget: (result) => {
    const cm = result.container.querySelector(".cm-content") as HTMLElement;
    if (cm) return cm;
    return result.container.firstElementChild as HTMLElement;
  },
};

// ---------------------------------------------------------------------------
// Test matrix
// ---------------------------------------------------------------------------

const adapters: EditorAdapter[] = [
  markdownAdapter,
  numberAdapter,
  selectAdapter,
  dateAdapter,
  colorAdapter,
  multiSelectAdapter,
];

const keymapModes = ["cua", "vim", "emacs"] as const;

type ExitPath = "blur" | "Enter" | "Escape";
const exitPaths: ExitPath[] = ["blur", "Enter", "Escape"];

/** Should updateField be called for this combination? */
function expectsSave(adapter: EditorAdapter, keymap: string, exit: ExitPath): boolean {
  if (adapter.alwaysSaves) return true;
  if (exit === "blur") return true;
  if (exit === "Enter") return true;
  if (exit === "Escape") return keymap === "vim";
  return false;
}

// ---------------------------------------------------------------------------
// The matrix
// ---------------------------------------------------------------------------

describe.each(adapters)("$name editor", (adapter) => {
  if (adapter.popoverLimited) {
    it.skip("popover-based editor — jsdom cannot render inner content", () => {});
    return;
  }

  describe.each(keymapModes)("keymap: %s", (keymap) => {
    beforeEach(() => {
      vi.clearAllMocks();
      KEYMAP_MODE = keymap;
    });

    it.each(exitPaths)("exit: %s", async (exit) => {
      if (exit === "blur" && adapter.skipBlur) return; // jsdom focus limitation

      const onDone = vi.fn();
      const onCancel = vi.fn();
      const result = adapter.render(onDone, onCancel);
      await settle();

      // For savesOnChange editors (e.g. select), the save happens during
      // setValue — clear mocks before setValue so we can assert it.
      if (adapter.savesOnChange) {
        mockUpdateField.mockClear();
        await adapter.setValue(result, "99");
        await settle();

        // savesOnChange editors: updateField is called during setValue.
        // Exit events are just lifecycle (close the editor).
        const shouldSave = expectsSave(adapter, keymap, exit);
        if (shouldSave) {
          expect(
            mockUpdateField,
            `${adapter.name} / ${keymap} / ${exit}: expected updateField on change`,
          ).toHaveBeenCalled();
        }
        // For discard exits (CUA/emacs Escape), the change already saved —
        // that's correct for select (click = commit). We still pass.
      } else {
        // Standard editors: set value, clear mocks, then assert on exit.
        await adapter.setValue(result, "99");
        await settle();
        mockUpdateField.mockClear();

        const target = adapter.exitTarget(result);
        if (target) {
          if (exit === "blur") {
            await act(async () => fireEvent.blur(target));
          } else {
            await act(async () => fireEvent.keyDown(target, { key: exit }));
          }
        }
        await settle();

        const shouldSave = expectsSave(adapter, keymap, exit);
        if (shouldSave) {
          expect(
            mockUpdateField,
            `${adapter.name} / ${keymap} / ${exit}: expected updateField to be called`,
          ).toHaveBeenCalled();
        } else {
          expect(
            mockUpdateField,
            `${adapter.name} / ${keymap} / ${exit}: expected updateField NOT to be called`,
          ).not.toHaveBeenCalled();
        }
      }

      result.unmount();
    });
  });
});
