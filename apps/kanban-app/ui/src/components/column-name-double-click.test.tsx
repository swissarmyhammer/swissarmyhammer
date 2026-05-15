/**
 * Spike test for the column-name surface's double-click behaviour after
 * collapsing the synthetic `column:<id>.name` `<FocusScope>` wrap into
 * the inner `<Field>` zone.
 *
 * After the refactor, the column-name surface is registered exactly
 * once — by the inner `<Field>` component as a
 * `<FocusScope moniker="field:column:<id>.name">`. `<Field>` always wraps
 * itself in `<Inspectable>` (which dispatches `ui.inspect` on
 * double-click), so the question this test pins is: does double-clicking
 * the column name still enter edit mode without dispatching an inspector
 * command for the field moniker?
 *
 * The answer relies on two existing primitives:
 *
 *   1. `FieldDisplayContent` wraps the rendered text in
 *      `<div onClick={onEdit}>`. The first click fires `onEdit()`,
 *      which sets `editingName(true)` and re-renders the field with an
 *      `<input>` host.
 *   2. `<Inspectable>`'s double-click handler skips the dispatch when
 *      the gesture lands on an editable surface (`<input>`,
 *      `<textarea>`, `<select>`, `[contenteditable]`). The browser's
 *      `dblclick` event uses the second click's target, which by then
 *      is the mounted input.
 *
 * Together they protect double-click on the column name from opening
 * the inspector. This spike verifies the protection holds for the
 * post-refactor wiring (no outer `<FocusScope>` swallowing the gesture
 * via `onClickCapture`). If this test fails, the refactor must fall
 * back to Option B (add a `disableSpatial` prop to `<Field>` so the
 * outer `<FocusScope>` can keep the navigation identity while
 * suppressing the inner zone's `<Inspectable>`).
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, fireEvent, waitFor } from "@testing-library/react";

// Schema mirrors production `builtin/definitions/name.yaml` for the
// `name` field — `editor: "markdown"` and `display: "text"`. The
// markdown editor mounts a CodeMirror surface (`.cm-editor`) when the
// field enters edit mode; the text display renders the value as a
// `<span>` wrapped in a `cursor-text` div via `FieldDisplayContent`.
const COLUMN_SCHEMA = {
  entity: {
    name: "column",
    fields: ["name"],
    commands: [],
  },
  fields: [
    {
      id: "name",
      name: "name",
      type: { kind: "markdown", single_line: true },
      section: "header",
      display: "text",
      editor: "markdown",
    },
  ],
};

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const mockInvoke = vi.fn(async (...args: any[]): Promise<unknown> => {
  if (args[0] === "list_entity_types") return ["column"];
  if (args[0] === "get_entity_schema") return COLUMN_SCHEMA;
  if (args[0] === "get_ui_state")
    return {
      palette_open: false,
      palette_mode: "command",
      keymap_mode: "cua",
      scope_chain: [],
      open_boards: [],
      windows: {},
      recent_boards: [],
    };
  if (args[0] === "list_commands_for_scope") return [];
  return "ok";
});

vi.mock("@tauri-apps/api/core", () => ({
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  invoke: (...args: any[]) => mockInvoke(...args),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(() => Promise.resolve()),
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
import { ColumnView } from "./column-view";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { ActiveBoardPathProvider } from "@/lib/command-scope";
import { TooltipProvider } from "@/components/ui/tooltip";
import { FocusLayer } from "./focus-layer";
import { FocusScope } from "./focus-scope";
import { asSegment } from "@/types/spatial";
import type { Entity } from "@/types/kanban";

function makeColumn(id = "col-1", name = "To Do"): Entity {
  return {
    entity_type: "column",
    id,
    moniker: `column:${id}`,
    fields: { name },
  };
}

/**
 * Mount a `<ColumnView>` in the production-shaped provider stack,
 * wrapped in a parent `ui:board` `<FocusScope>` so the column has a real
 * parent zone — mirroring its role inside `<BoardView>`.
 */
function renderColumnInBoard(ui: React.ReactElement, column: Entity) {
  return render(
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <EntityFocusProvider>
          <SchemaProvider>
            <EntityStoreProvider entities={{ column: [column] }}>
              <TooltipProvider>
                <ActiveBoardPathProvider value="/test/board">
                  <FocusScope moniker={asSegment("ui:board")}>{ui}</FocusScope>
                </ActiveBoardPathProvider>
              </TooltipProvider>
            </EntityStoreProvider>
          </SchemaProvider>
        </EntityFocusProvider>
      </FocusLayer>
    </SpatialFocusProvider>,
  );
}

describe("Column name double-click", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
  });

  it("double-click enters edit mode, not the inspector", async () => {
    const column = makeColumn("col-doing", "To Do");
    const { container } = renderColumnInBoard(
      <ColumnView column={column} tasks={[]} />,
      column,
    );

    // The schema loads asynchronously; wait for the field zone to mount.
    let fieldZone: HTMLElement | null = null;
    await waitFor(() => {
      fieldZone = container.querySelector(
        "[data-segment='field:column:col-doing.name']",
      );
      expect(fieldZone).toBeTruthy();
    });

    // Pre-flight sanity: the synthetic `column:col-doing.name` scope
    // must not appear in the DOM either — the same regression guard
    // pinned in column-view.spatial-nav.test.tsx, but at the rendered
    // DOM level so a regression can't slip through by registering at
    // the kernel level only.
    const syntheticEls = container.querySelectorAll(
      "[data-segment='column:col-doing.name']",
    );
    expect(syntheticEls).toHaveLength(0);

    // The displayed name lives inside the field zone — clicking it
    // hits `FieldDisplayContent`'s `onClick={onEdit}` wrapper. Locate
    // the click surface (the wrapper div with `cursor-text`).
    const editSurface = fieldZone!.querySelector(
      ".cursor-text",
    ) as HTMLElement | null;
    expect(editSurface).toBeTruthy();

    // Fire the double-click sequence the browser delivers: a `click`
    // (which enters edit mode and remounts the inner host as a
    // CodeMirror editor), followed by a `dblclick` whose target is
    // inside the editor surface. The `<Inspectable>`'s editable-surface
    // skip checks the dblclick's target — the editor's contenteditable
    // host element triggers the skip via the `[contenteditable]`
    // ancestor check.
    fireEvent.click(editSurface!);

    // Wait for the markdown editor to mount.
    let editor: HTMLElement | null = null;
    await waitFor(() => {
      editor = fieldZone!.querySelector(".cm-editor");
      expect(editor).toBeTruthy();
    });

    // The dblclick lands on the editor's contenteditable host — same
    // way the browser would deliver it after the first click swapped
    // the host. The `<Inspectable>` listens via `onDoubleClick`; its
    // handler skips when the target has a `[contenteditable]`
    // ancestor.
    const ceHost = (editor! as HTMLElement).querySelector(
      '[contenteditable="true"]',
    ) as HTMLElement | null;
    expect(ceHost).toBeTruthy();
    fireEvent.doubleClick(ceHost!);

    // The double-click must NOT have dispatched a `dispatch_command`
    // call for `ui.inspect` against the field moniker — `<Inspectable>`'s
    // editable-surface skip is the safety net.
    const inspectCalls = mockInvoke.mock.calls.filter(
      (c) =>
        c[0] === "dispatch_command" &&
        typeof c[1] === "object" &&
        c[1] !== null &&
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        (c[1] as any).name === "ui.inspect",
    );
    expect(
      inspectCalls,
      "ui.inspect must not be dispatched on double-click of the column name (the editor takes the gesture)",
    ).toHaveLength(0);

    // Edit mode is active — the editor is mounted. The state
    // `editingName=true` is what controls this; the test asserts it via
    // the DOM shape, which is the user-observable contract.
    expect(editor).toBeTruthy();
  });
});
