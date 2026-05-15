/**
 * Spatial-nav integration tests for `<ColumnView>`.
 *
 * Mounts a column inside the production-shaped provider stack
 * (`<SpatialFocusProvider>` + `<FocusLayer name="window">`) so the column's
 * `<FocusScope>` and the inner column-name-field `<FocusScope>`
 * register through the live spatial primitives. The Tauri `invoke` boundary
 * is mocked at the module level so we can inspect the `spatial_register_scope`
 * and `spatial_register_scope` calls each emits on mount.
 *
 * Companion file: `column-view.guards.node.test.ts` pins the source-level
 * invariants (no `ClaimPredicate` import, no neighbor-moniker plumbing, no
 * column-level keydown listener). This file pins the runtime contract:
 *
 *   - The column body registers as a zone with moniker `column:{id}`.
 *   - Its `parentZone` is the surrounding `<FocusScope>` (e.g. `ui:board`)
 *     when one is present, and `null` when the column is mounted directly
 *     under the layer root.
 *   - The column header registers as a leaf with `parentZone` equal to the
 *     column's zone key.
 *   - Each task card registers as a navigable container (`<FocusScope>`)
 *     parented at the column zone — cards are zones because they hold
 *     multiple focusable atoms (drag handle, Field rows, inspect button)
 *     and the kernel's path-prefix scope-is-leaf invariant rejects a
 *     `<FocusScope>` whose FQM is a strict prefix of any descendant.
 *   - No claim-predicate registration calls are emitted for the column or
 *     its header.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, act } from "@testing-library/react";
import type { Entity } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Tauri API mocks — must come before any module that imports them.
//
// `mockInvoke` is hoisted so the SpatialFocusProvider's invoke calls
// (`spatial_push_layer`, `spatial_register_scope`, …) flow through it and
// tests can assert against them.
// ---------------------------------------------------------------------------

// Schema for the column entity. The column-name surface is rendered by
// `<Field>`, which only mounts a `<FocusScope>` when the schema's
// `getFieldDef("column", "name")` returns a definition — without a
// schema the column header falls back to a bare `<span>` with no
// spatial-nav participation, and the field-zone registration assertions
// below cannot fire.
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
      type: { kind: "text" },
      section: "header",
      display: "text",
      editor: "text",
    },
  ],
};

const mockInvoke = vi.hoisted(() => {
  const fn = vi.fn(async (...args: unknown[]) => {
    if (args[0] === "list_entity_types") return ["column", "task"];
    if (args[0] === "get_entity_schema") {
      return COLUMN_SCHEMA;
    }
    if (args[0] === "get_ui_state") {
      return {
        palette_open: false,
        palette_mode: "command",
        keymap_mode: "cua",
        scope_chain: [],
        open_boards: [],
        windows: {},
        recent_boards: [],
      };
    }
    if (args[0] === "list_commands_for_scope") return [];
    return undefined;
  });
  return fn;
});

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
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

vi.mock("@tauri-apps/plugin-log", () => ({
  error: vi.fn(),
  warn: vi.fn(),
  info: vi.fn(),
  debug: vi.fn(),
  trace: vi.fn(),
  attachConsole: vi.fn(() => Promise.resolve()),
}));

// ---------------------------------------------------------------------------
// Imports come after mocks.
// ---------------------------------------------------------------------------

import { ColumnView } from "./column-view";
import { FocusLayer } from "./focus-layer";
import { FocusScope } from "./focus-scope";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { ActiveBoardPathProvider } from "@/lib/command-scope";
import { TooltipProvider } from "@/components/ui/tooltip";
import { asSegment } from "@/types/spatial";

// ---------------------------------------------------------------------------
// Test fixtures
// ---------------------------------------------------------------------------

function makeColumn(id = "col-1", name = "To Do"): Entity {
  return {
    entity_type: "column",
    id,
    moniker: `column:${id}`,
    fields: { name },
  };
}

function makeTask(id: string, column = "col-1"): Entity {
  return {
    entity_type: "task",
    id,
    moniker: `task:${id}`,
    fields: {
      title: `Task ${id}`,
      position_column: column,
      position_ordinal: "a0",
    },
  };
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Flush microtasks queued by the spatial-primitive register effects.
 *
 * `<FocusScope>` / `<FocusScope>` perform their `spatial_register_*` invocations
 * inside `useEffect`, which React flushes asynchronously. Without this nudge
 * the assertions run before the register calls land in the mock.
 */
async function flushSetup() {
  await act(async () => {
    await Promise.resolve();
  });
}

/**
 * Render a `<ColumnView>` inside the production spatial stack and a
 * surrounding `ui:board` zone, so the column registers with a real parent
 * zone (mirroring its role inside `<BoardView>`).
 */
function renderColumnInBoard(ui: React.ReactElement) {
  return render(
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <EntityFocusProvider>
          <SchemaProvider>
            <EntityStoreProvider entities={{}}>
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

/**
 * Pull every `spatial_register_scope` call as a typed record.
 *
 * After parent task `01KQSDP4ZJY5ERAJ68TFPVFRRE` collapsed the legacy
 * split primitives into a single `<FocusScope>`, every spatial primitive
 * registers via `spatial_register_scope`. The legacy helpers
 * `registeredZones` / `registeredScopes` (which used to filter on a
 * separate zone command) now alias to the same single-source helper.
 */
function registeredScopes(): Array<{
  fq: string;
  segment: string;
  rect: unknown;
  layerFq: string;
  parentZone: string | null;
}> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_register_scope")
    .map(
      (c) =>
        c[1] as {
          fq: string;
          segment: string;
          rect: unknown;
          layerFq: string;
          parentZone: string | null;
        },
    );
}

/** Legacy alias kept while call sites are migrated. */
const registeredZones = registeredScopes;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("ColumnView (spatial-nav)", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
  });

  it("registers the column body as a zone with moniker column:{id}", async () => {
    const { unmount } = renderColumnInBoard(
      <ColumnView column={makeColumn("col-doing")} tasks={[]} />,
    );
    await flushSetup();

    const columnZones = registeredZones().filter(
      (z) => z.segment === "column:col-doing",
    );
    expect(columnZones).toHaveLength(1);

    unmount();
  });

  it("parents the column zone at the surrounding ui:board zone", async () => {
    const { unmount } = renderColumnInBoard(
      <ColumnView column={makeColumn("col-doing")} tasks={[]} />,
    );
    await flushSetup();

    const boardZone = registeredZones().find((z) => z.segment === "ui:board");
    expect(boardZone).toBeTruthy();

    const columnZone = registeredZones().find(
      (z) => z.segment === "column:col-doing",
    );
    expect(columnZone).toBeTruthy();
    expect(columnZone!.parentZone).toBe(boardZone!.fq);

    unmount();
  });

  it("registers the column-name field zone inside the column zone", async () => {
    // After collapsing the synthetic `column:<id>.name` `<FocusScope>` wrap,
    // the column-name surface is registered exactly once — by the inner
    // `<Field>` component as a `<FocusScope>` with moniker
    // `field:column:<id>.name`. The registration's `parentZone` is the
    // enclosing column zone, so beam search treats the column-name field
    // as an in-column candidate.
    const { unmount } = renderColumnInBoard(
      <ColumnView column={makeColumn("col-doing")} tasks={[]} />,
    );
    await flushSetup();

    const columnZone = registeredZones().find(
      (z) => z.segment === "column:col-doing",
    );
    expect(columnZone).toBeTruthy();

    const fieldZone = registeredZones().find(
      (z) => z.segment === "field:column:col-doing.name",
    );
    expect(fieldZone).toBeTruthy();
    expect(fieldZone!.parentZone).toBe(columnZone!.fq);

    unmount();
  });

  it("does not register a synthetic column-name scope", async () => {
    // Regression guard: after the refactor, the only spatial-nav
    // registration for the column-name surface is the `<Field>` zone
    // with moniker `field:column:<id>.name`. The previous synthetic
    // `<FocusScope moniker="column:<id>.name">` wrap is gone — its
    // existence created two registrations against the same DOM rect
    // (a leaf and a zone), two click handlers, and two debug overlays.
    // This test pins the absence of the synthetic moniker so a future
    // change cannot silently re-introduce the duplication.
    const { unmount } = renderColumnInBoard(
      <ColumnView column={makeColumn("col-doing")} tasks={[]} />,
    );
    await flushSetup();

    const syntheticScopes = registeredScopes().filter(
      (s) => s.segment === "column:col-doing.name",
    );
    expect(syntheticScopes).toHaveLength(0);

    const syntheticZones = registeredZones().filter(
      (z) => z.segment === "column:col-doing.name",
    );
    expect(syntheticZones).toHaveLength(0);

    unmount();
  });

  it("registers each task card as a FocusScope parented at the column scope", async () => {
    // Cards register as `<FocusScope>` containers because they hold
    // multiple focusable atoms (drag handle, Field rows, inspect
    // button). The card's `parentZone` is the enclosing column's scope
    // key so the kernel groups cards by column for cross-column nav.
    //
    // After parent task `01KQSDP4ZJY5ERAJ68TFPVFRRE` collapsed the
    // legacy split primitives into a single `<FocusScope>`, every
    // spatial primitive registers via `spatial_register_scope`; the
    // structural distinction between a container and a leaf is whether
    // the scope has child scopes, not a separate registration command.
    const tasks = [makeTask("t1"), makeTask("t2")];
    const { unmount } = renderColumnInBoard(
      <ColumnView column={makeColumn("col-doing")} tasks={tasks} />,
    );
    await flushSetup();

    const columnZone = registeredZones().find(
      (z) => z.segment === "column:col-doing",
    );
    expect(columnZone).toBeTruthy();

    for (const id of ["t1", "t2"]) {
      const taskZone = registeredZones().find(
        (z) => z.segment === `task:${id}`,
      );
      expect(taskZone, `task:${id} scope registered`).toBeTruthy();
      expect(taskZone!.parentZone).toBe(columnZone!.fq);
    }

    unmount();
  });

  it("registers no claim predicates for the column or header (push-only nav)", async () => {
    const tasks = [makeTask("t1"), makeTask("t2"), makeTask("t3")];
    const { unmount } = renderColumnInBoard(
      <ColumnView column={makeColumn("col-doing")} tasks={tasks} />,
    );
    await flushSetup();

    const claimCalls = mockInvoke.mock.calls.filter(
      (c) =>
        c[0] === "spatial_register_claim" ||
        c[0] === "register_claim_predicates",
    );
    expect(claimCalls).toHaveLength(0);

    unmount();
  });

  it("emits a wrapper element with data-moniker='column:{id}'", async () => {
    const { container, unmount } = renderColumnInBoard(
      <ColumnView column={makeColumn("col-doing")} tasks={[]} />,
    );
    await flushSetup();

    const node = container.querySelector("[data-segment='column:col-doing']");
    expect(node).not.toBeNull();

    unmount();
  });

});
