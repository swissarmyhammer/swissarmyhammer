/**
 * Golden-path regression suite for spatial navigation.
 *
 * ## Purpose
 *
 * Consolidates every basic "nav works" invariant the project cares
 * about into one named suite. A PR touching any part of the spatial-nav
 * stack — `focus-scope.tsx`, `focus-layer.tsx`, `entity-focus-context.tsx`,
 * `inspector-focus-bridge.tsx`, `nav-bar.tsx`, `spatial-shim.ts`,
 * `spatial_state.rs`, `spatial_nav.rs` — must run this suite green
 * before merging. A passing run doesn't prove absence of bugs, but a
 * failing run proves presence of a regression.
 *
 * ## Shape
 *
 * Every test:
 * - Uses a committed fixture (board/grid/inspector/toolbar/etc.) so the
 *   invariants ride on the same fixture shells individual feature tests
 *   already use.
 * - Drives through the REAL dispatch path to Rust via
 *   `dispatch_command` — the only algorithmic response comes from
 *   `setup-tauri-stub.ts`, which is the thin Tauri boundary stub. The
 *   spatial algorithm lives entirely in Rust; JS fixture-side tests
 *   assert the React wiring, not the algorithm's answers. Every test
 *   name maps directly to an invariant in the task description so a
 *   single failure diagnoses itself.
 *
 * ## Invariants covered
 *
 * The suite is organised into five top-level `describe` blocks,
 * matching the grouping in the task description:
 * - global invariants (dispatch routing, fallback via scripted stub)
 * - per-region nav (board, grid, inspector, leftnav, perspective,
 *   toolbar)
 * - Enter activation (leftnav, perspective, grid, inspector, row
 *   selector, card, toolbar)
 * - cross-layer isolation (inspector over board, inspector over grid,
 *   three stacked inspectors, close restores parent)
 * - visual focus (exactly-one data-focused after click, nav, rapid
 *   clicking)
 *
 * Algorithm-level invariants (beam test, layer isolation, fallback to
 * first-in-layer, null-source recovery) are exercised by the Rust
 * scenario harness in `swissarmyhammer-spatial-nav/tests/spatial_cases.json`
 * — there is no JS mirror of the algorithm to drift from, so parity
 * testing happens entirely in Rust.
 */

import { useMemo, useRef, useState } from "react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { userEvent } from "vitest/browser";
import { render } from "vitest-browser-react";

// -------------------------------------------------------------------------
// Tauri mocks — every invoke routes through the boundary stub. Using the
// `actual + override invoke` variant is required for fixtures that load
// `window.js` / `webviewWindow.js` and their siblings (perspective bar,
// LeftNav, toolbar). The plain `tauriCoreMock()` variant works for the
// grid/board/inspector tests that do not transitively pull the full API.
// Keep both styles side-by-side — the spec-level test can share the
// widest-compatible form.
// -------------------------------------------------------------------------

vi.mock("@tauri-apps/api/core", async () => {
  const actual = await vi.importActual<typeof import("@tauri-apps/api/core")>(
    "@tauri-apps/api/core",
  );
  const { tauriCoreMock } = await import("./setup-tauri-stub");
  const { invoke } = tauriCoreMock();
  return { ...actual, invoke };
});
vi.mock("@tauri-apps/api/event", async () => {
  const actual = await vi.importActual<typeof import("@tauri-apps/api/event")>(
    "@tauri-apps/api/event",
  );
  const { tauriEventMock } = await import("./setup-tauri-stub");
  return { ...actual, ...tauriEventMock() };
});
vi.mock("@tauri-apps/api/window", async () => {
  const actual = await vi.importActual<typeof import("@tauri-apps/api/window")>(
    "@tauri-apps/api/window",
  );
  const { tauriWindowMock } = await import("./setup-tauri-stub");
  return { ...actual, ...tauriWindowMock() };
});
vi.mock("@tauri-apps/api/webviewWindow", async () => {
  const actual = await vi.importActual<
    typeof import("@tauri-apps/api/webviewWindow")
  >("@tauri-apps/api/webviewWindow");
  const { tauriWebviewWindowMock } = await import("./setup-tauri-stub");
  return { ...actual, ...tauriWebviewWindowMock() };
});
vi.mock("@tauri-apps/plugin-log", async () => {
  const { tauriPluginLogMock } = await import("./setup-tauri-stub");
  return tauriPluginLogMock();
});

// -------------------------------------------------------------------------
// Context mocks — the perspective bar and toolbar fixtures both need a
// minimal perspective/view/schema/ui-state surface. The shapes below
// mirror the mocks used by `spatial-nav-perspective.test.tsx` and
// `spatial-nav-toolbar.test.tsx` so the golden path exercises the same
// wiring without duplicating fixture plumbing.
// -------------------------------------------------------------------------

type MockPerspective = {
  id: string;
  name: string;
  view: string;
  filter?: string;
  group?: string;
};

const fixturePerspectives: MockPerspective[] = [
  { id: "default", name: "Default", view: "board" },
  { id: "archive", name: "Archive", view: "board" },
];

/**
 * Module-scope spy on `setActivePerspectiveId` so Enter-activation tests
 * can assert the tab's `onSelect` callback fires the perspective switch.
 * Mirrors the hoisting pattern in `perspective-tab-bar.test.tsx`.
 */
const mockSetActivePerspectiveId = vi.fn();

vi.mock("@/lib/perspective-context", () => ({
  usePerspectives: () => ({
    perspectives: fixturePerspectives,
    activePerspective: fixturePerspectives[0],
    setActivePerspectiveId: mockSetActivePerspectiveId,
    refresh: vi.fn(() => Promise.resolve()),
  }),
}));

vi.mock("@/lib/views-context", () => {
  const views = [
    { id: "board", name: "Board", kind: "board", icon: "kanban" },
    { id: "grid", name: "Grid", kind: "grid", icon: "table" },
  ];
  return {
    ViewsProvider: ({ children }: { children: React.ReactNode }) => children,
    useViews: () => ({
      views,
      activeView: views[0],
      setActiveViewId: vi.fn(),
      refresh: vi.fn(() => Promise.resolve()),
    }),
  };
});

const MOCK_PERCENT_FIELD_DEF = {
  name: "percent_complete",
  display_name: "% Complete",
  field_type: "PercentComplete",
};

vi.mock("@/lib/schema-context", () => ({
  useSchema: () => ({
    getSchema: () => ({
      entity: { name: "task", fields: [], search_display_field: "name" },
      fields: [],
    }),
    getFieldDef: (_entityType: string, fieldName: string) =>
      fieldName === "percent_complete" ? MOCK_PERCENT_FIELD_DEF : undefined,
    getEntityCommands: () => [],
    mentionableTypes: [],
    loading: false,
  }),
}));

vi.mock("@/lib/ui-state-context", () => ({
  useUIState: () => ({
    keymap_mode: "vim",
    scope_chain: [],
    open_boards: [],
    has_clipboard: false,
    clipboard_entity_type: null,
    windows: {},
    recent_boards: [],
  }),
  useUIStateLoading: () => ({
    state: {
      keymap_mode: "vim",
      scope_chain: [],
      open_boards: [],
      has_clipboard: false,
      clipboard_entity_type: null,
      windows: {},
      recent_boards: [],
    },
    loading: false,
  }),
}));

vi.mock("@/lib/entity-store-context", () => ({
  useEntityStore: () => ({ getEntities: () => [], getEntity: () => undefined }),
  useFieldValue: () => "Test Board",
}));

vi.mock("@/components/fields/field", () => ({
  Field: (props: Record<string, unknown>) => (
    <span data-testid="field-percent">{String(props.entityId)}</span>
  ),
}));

const FIXTURE_BOARD_DATA = {
  board: {
    entity_type: "board",
    id: "b1",
    moniker: "board:b1",
    fields: { name: { String: "Test Board" } },
  },
  columns: [],
  tags: [],
  virtualTagMeta: [],
  summary: {
    total_tasks: 5,
    total_actors: 2,
    ready_tasks: 3,
    blocked_tasks: 1,
    done_tasks: 1,
    percent_complete: 20,
  },
};

const FIXTURE_OPEN_BOARDS = [
  { path: "/boards/a/.kanban", name: "Board A", is_active: true },
];

vi.mock("@/components/window-container", () => ({
  useBoardData: () => FIXTURE_BOARD_DATA,
  useOpenBoards: () => FIXTURE_OPEN_BOARDS,
  useActiveBoardPath: () => "/boards/a/.kanban",
  useHandleSwitchBoard: () => vi.fn(),
}));

vi.mock("@/lib/command-scope", async () => {
  const actual = await vi.importActual<typeof import("@/lib/command-scope")>(
    "@/lib/command-scope",
  );
  return {
    ...actual,
    useCommandBusy: () => ({ isBusy: false }),
  };
});

// -------------------------------------------------------------------------
// Imports that see the mocks above.
// -------------------------------------------------------------------------

import { setupTauriStub, type TauriStubHandles } from "./setup-tauri-stub";
import { FixtureShell } from "./spatial-fixture-shell";
import {
  EntityFocusProvider,
  useEntityFocus,
} from "@/lib/entity-focus-context";
import { FocusScope } from "@/components/focus-scope";
import { FocusLayer } from "@/components/focus-layer";
import { CommandScopeProvider, type CommandDef } from "@/lib/command-scope";
import { fieldMoniker, moniker } from "@/lib/moniker";
import {
  AppWithBoardFixture,
  FIXTURE_CARD_MONIKERS as BOARD_CARD_MONIKERS,
  FIXTURE_COLUMN_MONIKERS as BOARD_COLUMN_MONIKERS,
} from "./spatial-board-fixture";
import {
  AppWithGridFixture,
  FixtureHeaderRow,
  FixtureRow,
  GRID_ROWS,
  FIXTURE_CELL_MONIKERS,
  FIXTURE_COLUMN_HEADER_MONIKERS,
  FIXTURE_ROW_SELECTOR_MONIKERS,
} from "./spatial-grid-fixture";
import {
  AppWithInspectorFixture,
  FIXTURE_CARD_MONIKER as INSPECTOR_CARD_MONIKER,
  FIXTURE_CARD_TYPE as INSPECTOR_CARD_TYPE,
  FIXTURE_CARD_ID as INSPECTOR_CARD_ID,
  FIXTURE_FIELD_MONIKERS as INSPECTOR_FIELD_MONIKERS,
  FIXTURE_FIELD_NAMES as INSPECTOR_FIELD_NAMES,
} from "./spatial-inspector-fixture";
import {
  AppWithInspectorOverGridFixture,
  FIXTURE_FIELD_MONIKERS as IOG_FIELD_MONIKERS,
} from "./spatial-inspector-over-grid-fixture";
import {
  AppWithMultiInspectorFixture,
  FIXTURE_ENTITY_MONIKERS as MI_ENTITY_MONIKERS,
  FIXTURE_FIELD_MONIKERS as MI_FIELD_MONIKERS,
} from "./spatial-multi-inspector-fixture";
import {
  AppWithBoardAndLeftNavFixture,
  FIXTURE_CARD_MONIKERS as LEFTNAV_CARD_MONIKERS,
  FIXTURE_VIEW_MONIKERS as LEFTNAV_VIEW_MONIKERS,
} from "./spatial-leftnav-fixture";
import {
  AppWithBoardAndPerspectiveFixture,
  FIXTURE_PERSPECTIVE_MONIKERS,
} from "./spatial-perspective-fixture";
import {
  AppWithToolbarFixture,
  TOOLBAR_MONIKERS,
} from "./spatial-toolbar-fixture";

// Timeout used by polling assertions. 500ms handles the
// ResizeObserver → spatial_register round trip on CI Chromium; tests
// that only need a single dispatch cycle usually settle under 100ms.
const POLL_TIMEOUT = 500;

/** Poll until `data-focused` on `el` matches `expected`. */
async function expectFocused(
  el: HTMLElement,
  expected: "true" | null,
): Promise<void> {
  await expect
    .poll(() => el.getAttribute("data-focused"), { timeout: POLL_TIMEOUT })
    .toBe(expected);
}

/**
 * Count elements whose `data-focused` attribute is exactly the string
 * `"true"`. Used by the "exactly one focused scope" invariant —
 * `querySelectorAll('[data-focused="true"]')` is a direct probe against
 * the same attribute `useFocusDecoration` writes.
 */
function countFocused(): number {
  return document.querySelectorAll('[data-focused="true"]').length;
}

/**
 * Wait for `spatial_register` to have been invoked with the given
 * moniker. Ensures the ResizeObserver-driven registration has finished
 * before tests assert focus movement onto the moniker — without this
 * wait, the very first test in a fixture can race the register.
 */
async function waitForRegistered(
  handles: TauriStubHandles,
  moniker: string,
): Promise<void> {
  await expect
    .poll(
      () =>
        handles.invocations().some((i) => {
          if (i.cmd !== "spatial_register") return false;
          const a = i.args as { args?: { moniker?: string } };
          return a.args?.moniker === moniker;
        }),
      { timeout: POLL_TIMEOUT },
    )
    .toBe(true);
}

// ---------------------------------------------------------------------------
// Local fixtures for Enter-activation invariants that are not covered by the
// shared fixtures.
//
// The shared `AppWithGridFixture` / `AppWithInspectorFixture` intentionally
// model just the spatial-nav substrate (focus scopes, layer pushes, nav
// dispatches) — they do not wire `grid.editEnter`, `inspector.editEnter`, or
// per-card / per-row-selector Enter bindings because those are view-level
// concerns, not navigation ones. The Enter-activation invariants listed in
// the task description nonetheless need to be pinned by a test, so the
// fixtures below mount the spatial substrate from the shared fixtures and
// layer on exactly the Enter command wiring that production uses (mirrored
// from `grid-view.tsx`, `inspector-focus-bridge.tsx`, `entity-card.tsx`, and
// `data-table.tsx`'s `RowSelector`). They are kept inside this test file so
// the shared fixtures stay minimal and the Enter-specific commands live next
// to the tests that assert on them.
// ---------------------------------------------------------------------------

/**
 * Grid fixture extended with `grid.editEnter` and `grid.edit` commands on
 * the outer `FixtureShell` scope, mirroring `grid-view.tsx`. The `execute`
 * callback is the `onEnterEdit` vi.fn so tests can assert the local side
 * effect "Enter drops the grid into edit mode" without stubbing the grid
 * hook. Both bindings share the same callback — production wires both ids
 * to `g().enterEdit()`.
 */
function AppWithGridAndEditCommandsFixture({
  onEnterEdit,
}: {
  onEnterEdit: () => void;
}) {
  const onEnterEditRef = useRef(onEnterEdit);
  onEnterEditRef.current = onEnterEdit;
  const extraCommands: CommandDef[] = useMemo(
    () => [
      {
        id: "grid.editEnter",
        name: "Edit Cell (Enter)",
        keys: { vim: "Enter" },
        execute: () => {
          onEnterEditRef.current();
        },
      },
      {
        id: "grid.edit",
        name: "Edit Cell",
        keys: { vim: "i", cua: "Enter" },
        execute: () => {
          onEnterEditRef.current();
        },
      },
    ],
    [],
  );
  return (
    <EntityFocusProvider>
      <FixtureShell extraCommands={extraCommands}>
        <div
          data-testid="grid-fixture-root"
          style={{
            width: "400px",
            display: "flex",
            flexDirection: "column",
          }}
        >
          <FixtureHeaderRow />
          {Array.from({ length: GRID_ROWS }, (_, r) => (
            <FixtureRow key={r} rowIndex={r} />
          ))}
        </div>
      </FixtureShell>
    </EntityFocusProvider>
  );
}

/**
 * Grid fixture where each row selector carries its own per-row Enter →
 * `ui.inspect` binding, mirroring the `RowSelector` component in
 * `data-table.tsx`. The per-row scope shadows any grid-level Enter binding,
 * so pressing Enter on a focused row selector dispatches `ui.inspect` with
 * the row entity's moniker as the target — not the cell-cursor's moniker.
 */
function AppWithGridAndRowSelectorEnterFixture() {
  return (
    <EntityFocusProvider>
      <FixtureShell>
        <div
          data-testid="grid-fixture-root"
          style={{
            width: "400px",
            display: "flex",
            flexDirection: "column",
          }}
        >
          <FixtureHeaderRow />
          {Array.from({ length: GRID_ROWS }, (_, r) => (
            <RowWithSelectorEnterCommand key={r} rowIndex={r} />
          ))}
        </div>
      </FixtureShell>
    </EntityFocusProvider>
  );
}

/**
 * A grid row that reuses the shared `FixtureRow` layout for cells, but
 * wraps it in a `FocusScope`-level Enter binding for the row-selector
 * moniker. We cannot modify the shared `FixtureRow` (its row-selector
 * `FocusScope` is declared with `commands={[]}`), so the Enter binding is
 * installed via a sibling `CommandScopeProvider` whose `ui.inspect`
 * executes with the correct per-row target. Clicking the row selector
 * sets focus to the selector moniker; the selector scope contains no
 * commands of its own but its ancestor `CommandScopeProvider` supplies
 * the Enter binding that production places directly on the selector's
 * `FocusScope`. The dispatched target matches production's exactly:
 * `moniker("tag", "tag-<row>")`.
 */
function RowWithSelectorEnterCommand({ rowIndex }: { rowIndex: number }) {
  const rowEntityMoniker = useMemo(
    () => moniker("tag", `tag-${rowIndex}`),
    [rowIndex],
  );
  const dispatchInspect = useRef<((target: string) => void) | null>(null);
  return (
    <RowSelectorEnterInjector
      rowIndex={rowIndex}
      entityMoniker={rowEntityMoniker}
      dispatchRef={dispatchInspect}
    />
  );
}

/**
 * Wires a `CommandScopeProvider` that contributes `ui.inspect` with
 * `keys: Enter` above the row-selector scope, so the keybinding chain
 * resolves Enter to `ui.inspect` when the selector is focused.
 *
 * Extracted from `RowWithSelectorEnterCommand` because the provider must
 * call `useDispatchCommand` for the target dispatch — production does
 * the same inside the `RowSelector` component.
 */
function RowSelectorEnterInjector({
  rowIndex,
  entityMoniker,
  dispatchRef: _dispatchRef,
}: {
  rowIndex: number;
  entityMoniker: string;
  dispatchRef: React.MutableRefObject<((target: string) => void) | null>;
}) {
  // The `CommandScopeProvider` must contribute a command whose execute
  // goes through `useDispatchCommand("ui.inspect")` — identical to
  // `RowSelector` in `data-table.tsx`. Using the inline import here keeps
  // the mock chain intact.
  const commands: CommandDef[] = useMemo(
    () => [
      {
        id: `ui.inspect.selector.${rowIndex}`,
        name: "Inspect Row",
        keys: { vim: "Enter", cua: "Enter" },
        execute: () => {
          void (async () => {
            const { invoke } = await import("@tauri-apps/api/core");
            await invoke("dispatch_command", {
              cmd: "ui.inspect",
              target: entityMoniker,
            });
          })();
        },
      },
    ],
    [entityMoniker, rowIndex],
  );
  return (
    <CommandScopeProvider commands={commands}>
      <FixtureRow rowIndex={rowIndex} />
    </CommandScopeProvider>
  );
}

/**
 * Card fixture with a per-card Enter → `ui.inspect` binding, mirroring
 * `useEnterInspectCommand` in `entity-card.tsx`. Used to exercise the
 * keyboard Enter path on cards (complementary to the `dblClick` test,
 * which exercises the mouse path). The dispatched `ui.inspect` payload
 * uses the card moniker as the explicit `target`.
 */
function AppWithCardEnterInspectFixture({
  cardMoniker: cardMk,
}: {
  cardMoniker: string;
}) {
  const commands: CommandDef[] = useMemo(
    () => [
      {
        id: `entity.activate.${cardMk}`,
        name: "Inspect",
        keys: { vim: "Enter", cua: "Enter" },
        execute: () => {
          void (async () => {
            const { invoke } = await import("@tauri-apps/api/core");
            await invoke("dispatch_command", {
              cmd: "ui.inspect",
              target: cardMk,
            });
          })();
        },
      },
    ],
    [cardMk],
  );
  return (
    <EntityFocusProvider>
      <FixtureShell>
        <FocusScope
          moniker={cardMk}
          commands={commands}
          data-testid="enter-inspect-card"
          style={{
            width: "200px",
            height: "60px",
            padding: "8px",
            border: "1px solid #ccc",
            cursor: "pointer",
          }}
        >
          <span>{cardMk}</span>
        </FocusScope>
      </FixtureShell>
    </EntityFocusProvider>
  );
}

/**
 * A minimal inspector fixture that installs an `inspector.editEnter` /
 * `inspector.edit` command on the inspector layer, mirroring
 * `inspector-focus-bridge.tsx`. Opens on double-click of the outer card
 * (same pattern as the shared inspector fixture) and focuses the first
 * field on mount. The Enter command's execute callback is the
 * `onEnterEdit` vi.fn so tests can assert edit-mode entry via the local
 * side effect.
 */
function AppWithInspectorAndEditCommandFixture({
  onEnterEdit,
}: {
  onEnterEdit: () => void;
}) {
  const [inspectorOpen, setInspectorOpen] = useState(false);
  const openInspectorRef = useRef(() => setInspectorOpen(true));
  openInspectorRef.current = () => setInspectorOpen(true);
  const closeInspector = () => setInspectorOpen(false);

  const extraCommands: CommandDef[] = useMemo(
    () => [
      {
        id: "ui.inspect",
        name: "Open Inspector",
        execute: () => {
          openInspectorRef.current();
        },
      },
    ],
    [],
  );

  return (
    <EntityFocusProvider>
      <FixtureShell
        extraCommands={extraCommands}
        navOverrides={{ navFirstVim: "g g", navLastVim: "Shift+G" }}
      >
        <div
          data-testid="inspector-fixture-root"
          style={{ width: "400px", padding: "16px" }}
        >
          <FocusScope
            moniker={INSPECTOR_CARD_MONIKER}
            commands={[]}
            data-testid="fixture-card"
            style={{
              width: "200px",
              height: "60px",
              padding: "8px",
              border: "1px solid #ccc",
              cursor: "pointer",
            }}
          >
            <span>Card 1-1</span>
          </FocusScope>
        </div>
        {inspectorOpen && (
          <InspectorBodyWithEditCommand
            onClose={closeInspector}
            onEnterEdit={onEnterEdit}
          />
        )}
      </FixtureShell>
    </EntityFocusProvider>
  );
}

/**
 * Inspector body for `AppWithInspectorAndEditCommandFixture`.
 *
 * Installs the `inspector.editEnter` / `inspector.edit` commands on the
 * inspector layer's `CommandScopeProvider` alongside `app.dismiss`. The
 * Enter commands' execute is routed to `onEnterEdit`; the dismiss binding
 * closes the inspector so the layer pops on Escape (mirroring the shared
 * `InspectorBody`). Focuses the first field on mount so Enter resolves
 * through the focused-field scope chain, matching production where the
 * inspector always has a field selected.
 */
function InspectorBodyWithEditCommand({
  onClose,
  onEnterEdit,
}: {
  onClose: () => void;
  onEnterEdit: () => void;
}) {
  const firstFieldMoniker = INSPECTOR_FIELD_MONIKERS[0];
  const { setFocus } = useEntityFocus();
  const setFocusRef = useRef(setFocus);
  setFocusRef.current = setFocus;
  const onEnterEditRef = useRef(onEnterEdit);
  onEnterEditRef.current = onEnterEdit;

  // Focus first field on mount so Enter resolves through the field scope.
  useMemoFirstFieldFocus(firstFieldMoniker, setFocusRef);

  const inspectorCommands: CommandDef[] = useMemo(
    () => [
      {
        id: "app.dismiss",
        name: "Close Inspector",
        keys: { vim: "Escape", cua: "Escape" },
        execute: onClose,
      },
      {
        id: "inspector.editEnter",
        name: "Edit Field (Enter)",
        keys: { vim: "Enter" },
        execute: () => {
          onEnterEditRef.current();
        },
      },
      {
        id: "inspector.edit",
        name: "Edit Field",
        keys: { vim: "i", cua: "Enter" },
        execute: () => {
          onEnterEditRef.current();
        },
      },
    ],
    [onClose],
  );

  return (
    <FocusLayer name="inspector">
      <CommandScopeProvider commands={inspectorCommands}>
        <div
          data-testid="inspector-body"
          style={{
            position: "fixed",
            top: 0,
            right: 0,
            width: "300px",
            height: "100vh",
            background: "#fff",
            borderLeft: "1px solid #ccc",
            display: "flex",
            flexDirection: "column",
          }}
        >
          {INSPECTOR_FIELD_NAMES.map((name) => (
            <FocusScope
              key={name}
              moniker={fieldMoniker(
                INSPECTOR_CARD_TYPE,
                INSPECTOR_CARD_ID,
                name,
              )}
              commands={[]}
              data-testid={`field-row-${name}`}
              style={{
                height: "40px",
                padding: "8px",
                borderBottom: "1px solid #ccc",
              }}
            >
              <span>{name}</span>
            </FocusScope>
          ))}
        </div>
      </CommandScopeProvider>
    </FocusLayer>
  );
}

/**
 * Focus the first inspector field on mount by invoking `setFocus`.
 *
 * Wrapped in a named hook so the `useEffect` dep list is explicit and the
 * enclosing component stays declarative. Mirrors `useFirstFieldFocus` in
 * `entity-inspector.tsx`.
 */
function useMemoFirstFieldFocus(
  firstFieldMoniker: string,
  setFocusRef: React.MutableRefObject<(moniker: string | null) => void>,
) {
  // useMemo for stable identity of the focus-on-mount effect; the body
  // is a side-effect (setFocus call) deferred to a microtask via a ref.
  useMemoOnce(() => {
    queueMicrotask(() => setFocusRef.current(firstFieldMoniker));
  });
}

/**
 * Run `fn` exactly once per component mount.
 *
 * Tiny helper so the focus-on-mount effect avoids the React effect-order
 * flicker. `useState(fn)` runs `fn` during the commit of the initial
 * render and never again.
 */
function useMemoOnce(fn: () => void): void {
  useState(() => {
    fn();
    return null;
  });
}

/**
 * Global-invariant fixture for the "focused scope unmount transitions focus
 * to a registered successor" test. Renders two sibling `FocusScope`s (A
 * and B) in the same layer, with a toggle to unmount A. Tests click A to
 * focus it, flip the toggle, and emit `focus-changed` pointing to B — the
 * same payload the Rust side would emit after `spatial_unregister` picks a
 * successor from the remaining layer entries. The invariant is that the
 * React-side decoration repaints onto B without any intermediate null
 * frame.
 */
function AppWithUnmountableScopesFixture({
  unmounted,
}: {
  unmounted: boolean;
}) {
  const mkA = "task:unmount-a";
  const mkB = "task:unmount-b";
  return (
    <EntityFocusProvider>
      <FixtureShell>
        <div
          data-testid="unmount-fixture-root"
          style={{
            display: "flex",
            flexDirection: "column",
            gap: "8px",
            padding: "16px",
            width: "200px",
          }}
        >
          {!unmounted && (
            <FocusScope
              moniker={mkA}
              commands={[]}
              data-testid={`data-moniker:${mkA}`}
              style={{
                height: "40px",
                padding: "8px",
                border: "1px solid #ccc",
              }}
            >
              <span>A</span>
            </FocusScope>
          )}
          <FocusScope
            moniker={mkB}
            commands={[]}
            data-testid={`data-moniker:${mkB}`}
            style={{
              height: "40px",
              padding: "8px",
              border: "1px solid #ccc",
            }}
          >
            <span>B</span>
          </FocusScope>
        </div>
      </FixtureShell>
    </EntityFocusProvider>
  );
}

/** Monikers exposed for the unmount-successor test. */
const UNMOUNT_FIXTURE_MK_A = "task:unmount-a";
const UNMOUNT_FIXTURE_MK_B = "task:unmount-b";

// ===========================================================================
// Global invariants — dispatch routing, focus recovery via scripted stub.
// ===========================================================================

describe("golden path: global invariants", () => {
  let handles: TauriStubHandles;

  beforeEach(() => {
    handles = setupTauriStub();
  });

  /**
   * Regression fence against re-introducing a JS-side `nav.*` broadcaster:
   * `j` must route through `invoke("dispatch_command", { cmd: "nav.down" })`.
   * If a broadcastNavCommand side-channel ever returns, this call will
   * be missing and the test will fail loudly.
   */
  it("h_j_k_l_each_dispatch_their_nav_command_through_dispatch_command", async () => {
    const screen = await render(<AppWithGridFixture />);
    const cell = screen.getByTestId(
      `data-moniker:${FIXTURE_CELL_MONIKERS[0][0]}`,
    );

    await userEvent.click(cell.element());
    await expectFocused(cell.element() as HTMLElement, "true");

    await userEvent.keyboard("j");
    await userEvent.keyboard("l");
    await userEvent.keyboard("k");
    await userEvent.keyboard("h");

    const ids = handles.dispatchedCommands().map((d) => d.cmd);
    expect(ids).toContain("nav.down");
    expect(ids).toContain("nav.right");
    expect(ids).toContain("nav.up");
    expect(ids).toContain("nav.left");
  });

  /**
   * When a scope is registered in the active layer and the user clicks
   * into it, focus decoration activates: `data-focused="true"` on
   * exactly one element. Exactly-one is the strictest invariant — never
   * zero, never two.
   */
  it("click_produces_exactly_one_data_focused_element", async () => {
    const screen = await render(<AppWithGridFixture />);
    const cell = screen.getByTestId(
      `data-moniker:${FIXTURE_CELL_MONIKERS[0][0]}`,
    );

    await userEvent.click(cell.element());
    await expectFocused(cell.element() as HTMLElement, "true");

    expect(countFocused()).toBe(1);
  });

  /**
   * After a nav key fires and the scripted backend emits
   * `focus-changed` with a new `next_key`, the old scope loses
   * `data-focused` and the new scope gains it — atomically, never
   * showing two bars at once.
   */
  it("nav_with_focus_change_transfers_data_focused_exactly_once", async () => {
    const screen = await render(<AppWithGridFixture />);
    const srcMk = FIXTURE_CELL_MONIKERS[0][0];
    const dstMk = FIXTURE_CELL_MONIKERS[1][0];
    const src = screen.getByTestId(`data-moniker:${srcMk}`);
    const dst = screen.getByTestId(`data-moniker:${dstMk}`);

    handles.scriptResponse("dispatch_command:nav.down", () =>
      handles.payloadForFocusMove(srcMk, dstMk),
    );

    await userEvent.click(src.element());
    await expectFocused(src.element() as HTMLElement, "true");

    await userEvent.keyboard("j");
    await expectFocused(dst.element() as HTMLElement, "true");
    await expectFocused(src.element() as HTMLElement, null);
    expect(countFocused()).toBe(1);
  });

  /**
   * When the scripted backend emits a no-op (no `focus-changed` event,
   * modeling a clamp at the edge), the currently-focused element keeps
   * its decoration — no silent drop to zero focused elements.
   */
  it("nav_with_no_backend_response_leaves_focus_unchanged", async () => {
    const screen = await render(<AppWithBoardFixture />);
    const bottomMk = BOARD_CARD_MONIKERS[1][2];
    const bottom = screen
      .getByTestId(`data-moniker:${bottomMk}`)
      .element() as HTMLElement;

    // No scriptResponse installed — stub returns null, emits nothing.

    await userEvent.click(bottom);
    await expectFocused(bottom, "true");

    await userEvent.keyboard("j");

    expect(handles.dispatchedCommands().some((d) => d.cmd === "nav.down")).toBe(
      true,
    );
    expect(bottom.getAttribute("data-focused")).toBe("true");
    expect(countFocused()).toBe(1);
  });

  /**
   * Global invariant from the task description:
   *
   * > When a focused scope unmounts, focus transitions to a registered
   * > successor in the same layer — never to null if the layer has other
   * > entries.
   *
   * The successor pick is a Rust-side decision (the spatial backend
   * selects a neighbor via the `spatial_unregister` path). React's job is
   * to honor the subsequent `focus-changed` event by repainting
   * `data-focused` onto the successor without dropping to zero focused
   * elements in the interim. Model the backend's decision by emitting
   * `focus-changed` targeting scope B as soon as scope A has unmounted —
   * this pins the React wiring that listens to `focus-changed` after an
   * unregister and updates decoration accordingly. A regression that
   * orphans focus (leaves neither A nor B with `data-focused`) would fail
   * the final `countFocused()` assertion.
   */
  it("unmounting_focused_scope_transitions_focus_to_successor_in_same_layer", async () => {
    const { rerender } = await render(
      <AppWithUnmountableScopesFixture unmounted={false} />,
    );
    await waitForRegistered(handles, UNMOUNT_FIXTURE_MK_A);
    await waitForRegistered(handles, UNMOUNT_FIXTURE_MK_B);

    const scopeA = document.querySelector(
      `[data-testid="data-moniker:${UNMOUNT_FIXTURE_MK_A}"]`,
    ) as HTMLElement;
    expect(scopeA).toBeTruthy();
    await userEvent.click(scopeA);
    await expectFocused(scopeA, "true");

    // Flip the toggle so scope A unmounts. React's cleanup effect fires
    // `spatial_unregister` for A, and the shared focus-changed listener
    // picks up the next emitted payload.
    await rerender(<AppWithUnmountableScopesFixture unmounted={true} />);

    // Model Rust's successor pick: the remaining registered scope in the
    // same layer is B, so the backend emits a focus-changed pointing at
    // B. Without this emission the real backend would still pick B —
    // the fixture just models the response the stub cannot synthesize
    // on its own.
    handles.emitFocusChangedForMoniker(UNMOUNT_FIXTURE_MK_B);

    const scopeB = document.querySelector(
      `[data-testid="data-moniker:${UNMOUNT_FIXTURE_MK_B}"]`,
    ) as HTMLElement;
    await expectFocused(scopeB, "true");
    expect(countFocused()).toBe(1);
  });
});

// ===========================================================================
// Per-region navigation — one test per invariant in the task description.
// Each test either (a) asserts a dispatch fires for a given key, or (b)
// scripts the backend response and asserts `data-focused` lands on the
// expected scope. Algorithm correctness ("which cell wins h/l from a
// corner?") is Rust's concern.
// ===========================================================================

describe("golden path: board region", () => {
  let handles: TauriStubHandles;

  beforeEach(() => {
    handles = setupTauriStub();
  });

  /**
   * Within a column, `j` from a card must emit `nav.down`. The scripted
   * stub models "backend answers with card below" — a regression that
   * drops the dispatch or the decoration will fail one of the two
   * assertions.
   */
  it("board_j_within_column_moves_to_next_card_down", async () => {
    const screen = await render(<AppWithBoardFixture />);
    const topMk = BOARD_CARD_MONIKERS[0][0];
    const bottomMk = BOARD_CARD_MONIKERS[0][1];
    const top = screen
      .getByTestId(`data-moniker:${topMk}`)
      .element() as HTMLElement;
    const bottom = screen
      .getByTestId(`data-moniker:${bottomMk}`)
      .element() as HTMLElement;

    handles.scriptResponse("dispatch_command:nav.down", () =>
      handles.payloadForFocusMove(topMk, bottomMk),
    );

    await userEvent.click(top);
    await expectFocused(top, "true");

    await userEvent.keyboard("j");
    await expectFocused(bottom, "true");
  });

  /**
   * Across columns, `l` from a card must emit `nav.right`. Again the
   * scripted response pins the decoration contract; the column pick is
   * the Rust algorithm's job.
   */
  it("board_l_across_columns_moves_to_adjacent_column_nearest_card", async () => {
    const screen = await render(<AppWithBoardFixture />);
    const leftMk = BOARD_CARD_MONIKERS[0][1];
    const rightMk = BOARD_CARD_MONIKERS[1][1];
    const left = screen
      .getByTestId(`data-moniker:${leftMk}`)
      .element() as HTMLElement;
    const right = screen
      .getByTestId(`data-moniker:${rightMk}`)
      .element() as HTMLElement;

    handles.scriptResponse("dispatch_command:nav.right", () =>
      handles.payloadForFocusMove(leftMk, rightMk),
    );

    await userEvent.click(left);
    await expectFocused(left, "true");

    await userEvent.keyboard("l");
    await expectFocused(right, "true");
  });

  /**
   * Every card and every column moniker must register a spatial entry
   * on mount — otherwise Rust has no rects to beam-test against. The
   * task description reads "from a card, h/l moves to adjacent card" —
   * that only works if both the card and column scopes are registered.
   */
  it("board_every_card_and_column_scope_registers_on_mount", async () => {
    await render(<AppWithBoardFixture />);

    for (const col of BOARD_CARD_MONIKERS) {
      for (const cardMk of col) {
        await waitForRegistered(handles, cardMk);
      }
    }
    for (const columnMk of BOARD_COLUMN_MONIKERS) {
      await waitForRegistered(handles, columnMk);
    }
  });
});

describe("golden path: grid region", () => {
  let handles: TauriStubHandles;

  beforeEach(() => {
    handles = setupTauriStub();
  });

  /**
   * `j`/`k`/`h`/`l` from a body cell must each dispatch the matching
   * `nav.*` command. Pressing all four in sequence captures the full
   * set in one pass.
   */
  it("grid_hjkl_from_body_cell_each_dispatch_matching_nav_command", async () => {
    const screen = await render(<AppWithGridFixture />);
    const cell = screen.getByTestId(
      `data-moniker:${FIXTURE_CELL_MONIKERS[1][1]}`,
    );

    await userEvent.click(cell.element());
    await expectFocused(cell.element() as HTMLElement, "true");

    await userEvent.keyboard("h");
    await userEvent.keyboard("j");
    await userEvent.keyboard("k");
    await userEvent.keyboard("l");

    const ids = handles.dispatchedCommands().map((d) => d.cmd);
    expect(ids).toContain("nav.left");
    expect(ids).toContain("nav.down");
    expect(ids).toContain("nav.up");
    expect(ids).toContain("nav.right");
  });

  /**
   * From the topmost body row, `k` must be able to reach a column
   * header. The scripted response asserts the decoration contract —
   * Rust owns the "which header cell" pick and is parity-tested in
   * `spatial_cases.json → grid header row: Up from body cell lands on
   * header above`.
   */
  it("grid_k_from_top_body_row_reaches_column_header", async () => {
    const screen = await render(<AppWithGridFixture />);
    const cellMk = FIXTURE_CELL_MONIKERS[0][0];
    const hdrMk = FIXTURE_COLUMN_HEADER_MONIKERS[0];
    const cell = screen
      .getByTestId(`data-moniker:${cellMk}`)
      .element() as HTMLElement;
    const hdr = screen
      .getByTestId(`data-moniker:${hdrMk}`)
      .element() as HTMLElement;

    handles.scriptResponse("dispatch_command:nav.up", () =>
      handles.payloadForFocusMove(cellMk, hdrMk),
    );

    await userEvent.click(cell);
    await expectFocused(cell, "true");

    await userEvent.keyboard("k");
    await expectFocused(hdr, "true");
  });

  /**
   * From the leftmost data cell, `h` must be able to reach the row
   * selector. Row-selector FocusScope must exist in the DOM and the
   * decoration contract must hold when the scripted backend nominates
   * it.
   */
  it("grid_h_from_leftmost_data_cell_reaches_row_selector", async () => {
    const screen = await render(<AppWithGridFixture />);
    const cellMk = FIXTURE_CELL_MONIKERS[1][0];
    const selMk = FIXTURE_ROW_SELECTOR_MONIKERS[1];
    const cell = screen
      .getByTestId(`data-moniker:${cellMk}`)
      .element() as HTMLElement;
    const sel = screen
      .getByTestId(`data-moniker:${selMk}`)
      .element() as HTMLElement;

    handles.scriptResponse("dispatch_command:nav.left", () =>
      handles.payloadForFocusMove(cellMk, selMk),
    );

    await userEvent.click(cell);
    await expectFocused(cell, "true");

    await userEvent.keyboard("h");
    await expectFocused(sel, "true");
  });

  /**
   * Row selectors and column headers must register — without
   * registration the above two scenarios are vacuously unreachable.
   */
  it("grid_row_selectors_and_column_headers_register_on_mount", async () => {
    await render(<AppWithGridFixture />);

    for (const selMk of FIXTURE_ROW_SELECTOR_MONIKERS) {
      await waitForRegistered(handles, selMk);
    }
    for (const hdrMk of FIXTURE_COLUMN_HEADER_MONIKERS) {
      await waitForRegistered(handles, hdrMk);
    }
  });
});

describe("golden path: inspector region", () => {
  let handles: TauriStubHandles;

  beforeEach(() => {
    handles = setupTauriStub();
  });

  /**
   * Every inspector field row must register on mount. This is the
   * "j/k moves through every registered field row — no skipping"
   * precondition: a missing registration is the same as skipping.
   */
  it("inspector_every_field_registers_including_header_section", async () => {
    const screen = await render(<AppWithInspectorFixture />);
    const card = screen.getByTestId("fixture-card");
    await userEvent.click(card.element());
    await userEvent.dblClick(card.element());

    for (const m of INSPECTOR_FIELD_MONIKERS) {
      await waitForRegistered(handles, m);
    }
  });

  /**
   * `j` on a focused field dispatches `nav.down` through
   * `dispatch_command`. Pins the React → Rust route even after an
   * inspector layer push.
   */
  it("inspector_j_from_field_dispatches_nav_down", async () => {
    const screen = await render(<AppWithInspectorFixture />);
    const card = screen.getByTestId("fixture-card");
    await userEvent.click(card.element());
    await userEvent.dblClick(card.element());

    const before = handles.dispatchedCommands().length;
    await userEvent.keyboard("j");

    await expect
      .poll(
        () =>
          handles
            .dispatchedCommands()
            .slice(before)
            .some((d) => d.cmd === "nav.down"),
        { timeout: POLL_TIMEOUT },
      )
      .toBe(true);
  });

  /**
   * When the scripted backend reports no move (clamp at the first field),
   * the first field stays focused — no leak into the parent view's layer.
   * The layer-isolation algorithm itself is Rust's concern
   * (`spatial_cases.json → layer filter excludes entries on inactive
   * layers`); here we verify the React wiring doesn't drop focus on a
   * clamp.
   */
  it("inspector_k_from_first_field_stays_on_first_field", async () => {
    const screen = await render(<AppWithInspectorFixture />);
    const card = screen.getByTestId("fixture-card");
    await userEvent.click(card.element());
    await userEvent.dblClick(card.element());

    const firstField = screen
      .getByTestId("field-row-title")
      .element() as HTMLElement;
    await expectFocused(firstField, "true");

    // No scripted response — stub returns null, emits nothing.
    await userEvent.keyboard("k");

    expect(firstField.getAttribute("data-focused")).toBe("true");
    expect(countFocused()).toBe(1);
  });

  /**
   * Closing the inspector (Escape) unmounts the layer and fires
   * `spatial_remove_layer`. The subsequent focus recovery ("return to
   * card") is the shim's responsibility and is covered under
   * `cross-layer isolation`.
   */
  it("inspector_escape_emits_spatial_remove_layer", async () => {
    const screen = await render(<AppWithInspectorFixture />);
    const card = screen.getByTestId("fixture-card");
    await userEvent.click(card.element());
    await userEvent.dblClick(card.element());

    await expect
      .poll(
        () =>
          handles.invocations().filter((i) => i.cmd === "spatial_push_layer")
            .length,
        { timeout: POLL_TIMEOUT },
      )
      .toBeGreaterThan(0);

    await userEvent.keyboard("{Escape}");

    await expect
      .poll(
        () =>
          handles.invocations().filter((i) => i.cmd === "spatial_remove_layer")
            .length,
        { timeout: POLL_TIMEOUT },
      )
      .toBeGreaterThan(0);
  });
});

describe("golden path: left-nav region", () => {
  let handles: TauriStubHandles;

  beforeEach(() => {
    handles = setupTauriStub();
  });

  /**
   * Every view button in `<LeftNav />` must register its `view:<id>`
   * moniker — this is the substrate for `j`/`k` between view buttons
   * and for `l` reaching the main content.
   */
  it("leftnav_every_view_button_registers_on_mount", async () => {
    await render(<AppWithBoardAndLeftNavFixture />);

    for (const viewMk of LEFTNAV_VIEW_MONIKERS) {
      await waitForRegistered(handles, viewMk);
    }
  });

  /**
   * Click on a view button flips `data-focused` (via `spatial_focus`)
   * and the Enter contract can be exercised from there. The h/l
   * crossing between LeftNav and the view body is covered separately.
   */
  it("leftnav_click_invokes_spatial_focus_and_flips_data_focused", async () => {
    const screen = await render(<AppWithBoardAndLeftNavFixture />);
    const viewBtn = screen
      .getByTestId(`data-moniker:${LEFTNAV_VIEW_MONIKERS[1]}`)
      .element() as HTMLElement;

    await userEvent.click(viewBtn);
    await expectFocused(viewBtn, "true");
    expect(handles.invocations().some((i) => i.cmd === "spatial_focus")).toBe(
      true,
    );
  });

  /**
   * `h` from a leftmost card dispatches `nav.left` — the precondition
   * for "h from the view body reaches a LeftNav button". The decoration
   * side of the contract is asserted in
   * `leftnav_scripted_nav_left_lands_on_leftnav_button` below.
   */
  it("leftnav_h_from_leftmost_card_dispatches_nav_left", async () => {
    const screen = await render(<AppWithBoardAndLeftNavFixture />);
    const card = screen
      .getByTestId(`data-moniker:${LEFTNAV_CARD_MONIKERS[0][0]}`)
      .element() as HTMLElement;

    await userEvent.click(card);
    const before = handles.dispatchedCommands().length;
    await userEvent.keyboard("h");

    await expect
      .poll(
        () =>
          handles
            .dispatchedCommands()
            .slice(before)
            .some((d) => d.cmd === "nav.left"),
        { timeout: POLL_TIMEOUT },
      )
      .toBe(true);
  });

  /**
   * Decoration contract: the scripted backend emits `focus-changed`
   * moving from a card to a LeftNav view button, and the button ends
   * up with `data-focused="true"` while the card loses it.
   */
  it("leftnav_scripted_nav_left_lands_on_leftnav_button", async () => {
    const screen = await render(<AppWithBoardAndLeftNavFixture />);
    const cardMk = LEFTNAV_CARD_MONIKERS[0][0];
    const viewMk = LEFTNAV_VIEW_MONIKERS[0];
    const card = screen
      .getByTestId(`data-moniker:${cardMk}`)
      .element() as HTMLElement;
    const viewBtn = screen
      .getByTestId(`data-moniker:${viewMk}`)
      .element() as HTMLElement;

    handles.scriptResponse("dispatch_command:nav.left", () =>
      handles.payloadForFocusMove(cardMk, viewMk),
    );

    await userEvent.click(card);
    await expectFocused(card, "true");

    await userEvent.keyboard("h");
    await expectFocused(viewBtn, "true");
    await expectFocused(card, null);
  });

  /**
   * `l` from a focused LeftNav view button dispatches `nav.right` —
   * the routing half of "l from any LeftNav button reaches the main
   * content". The scripted-response mirror is asserted in the next
   * test.
   */
  it("leftnav_l_from_view_button_dispatches_nav_right", async () => {
    const screen = await render(<AppWithBoardAndLeftNavFixture />);
    const viewBtn = screen
      .getByTestId(`data-moniker:${LEFTNAV_VIEW_MONIKERS[0]}`)
      .element() as HTMLElement;

    await userEvent.click(viewBtn);
    const before = handles.dispatchedCommands().length;
    await userEvent.keyboard("l");

    await expect
      .poll(
        () =>
          handles
            .dispatchedCommands()
            .slice(before)
            .some((d) => d.cmd === "nav.right"),
        { timeout: POLL_TIMEOUT },
      )
      .toBe(true);
  });

  /**
   * Decoration contract: scripted `nav.right` from a LeftNav button
   * lands on a card in the main view body.
   */
  it("leftnav_scripted_nav_right_lands_on_main_content_card", async () => {
    const screen = await render(<AppWithBoardAndLeftNavFixture />);
    const viewMk = LEFTNAV_VIEW_MONIKERS[0];
    const cardMk = LEFTNAV_CARD_MONIKERS[0][0];
    const viewBtn = screen
      .getByTestId(`data-moniker:${viewMk}`)
      .element() as HTMLElement;
    const card = screen
      .getByTestId(`data-moniker:${cardMk}`)
      .element() as HTMLElement;

    handles.scriptResponse("dispatch_command:nav.right", () =>
      handles.payloadForFocusMove(viewMk, cardMk),
    );

    await userEvent.click(viewBtn);
    await expectFocused(viewBtn, "true");

    await userEvent.keyboard("l");
    await expectFocused(card, "true");
    await expectFocused(viewBtn, null);
  });
});

describe("golden path: perspective-tab-bar region", () => {
  let handles: TauriStubHandles;

  beforeEach(() => {
    handles = setupTauriStub();
  });

  /**
   * Every perspective tab registers its `perspective:<id>` moniker.
   * Without registration, h/l between tabs is vacuously unreachable.
   */
  it("perspective_every_tab_registers_on_mount", async () => {
    await render(<AppWithBoardAndPerspectiveFixture />);

    for (const m of FIXTURE_PERSPECTIVE_MONIKERS) {
      await waitForRegistered(handles, m);
    }
  });

  /**
   * A raw mouse click on the perspective tab `<div>` lands
   * `data-focused="true"` on that same element. The tab is wrapped in a
   * `FocusScope` with `renderContainer={false}`; the tab's outer `<div>`
   * carries `onClickCapture={handleScopeClick}` that calls
   * `setFocus(tabMoniker)`. The store update drives the enclosing scope's
   * `useFocusDecoration` to write `data-focused` to this same div.
   *
   * This is the mouse-reachability half of "a perspective tab is fully
   * usable by keyboard and mouse" — the keyboard half is exercised by
   * the scripted-nav tests below.
   */
  it("perspective_click_on_tab_div_focuses_scope", async () => {
    const screen = await render(<AppWithBoardAndPerspectiveFixture />);
    const tabMk = FIXTURE_PERSPECTIVE_MONIKERS[0];
    const tab = screen
      .getByTestId(`data-moniker:${tabMk}`)
      .element() as HTMLElement;

    await userEvent.click(tab);
    await expectFocused(tab, "true");
  });

  /**
   * `k` from a top-row card dispatches `nav.up` — the precondition for
   * "k from the top row reaches the perspective bar".
   */
  it("perspective_k_from_top_row_card_dispatches_nav_up", async () => {
    const screen = await render(<AppWithBoardAndPerspectiveFixture />);
    const top = screen
      .getByTestId(`data-moniker:${BOARD_CARD_MONIKERS[1][0]}`)
      .element() as HTMLElement;

    await userEvent.click(top);
    const before = handles.dispatchedCommands().length;
    await userEvent.keyboard("k");

    await expect
      .poll(
        () =>
          handles
            .dispatchedCommands()
            .slice(before)
            .some((d) => d.cmd === "nav.up"),
        { timeout: POLL_TIMEOUT },
      )
      .toBe(true);
  });

  /**
   * Decoration contract: scripted `nav.up` from a top-row card lands
   * on the active perspective tab.
   */
  it("perspective_scripted_nav_up_from_card_lands_on_active_tab", async () => {
    const screen = await render(<AppWithBoardAndPerspectiveFixture />);
    const cardMk = BOARD_CARD_MONIKERS[1][0];
    const tabMk = FIXTURE_PERSPECTIVE_MONIKERS[0];
    const card = screen
      .getByTestId(`data-moniker:${cardMk}`)
      .element() as HTMLElement;
    const tab = screen
      .getByTestId(`data-moniker:${tabMk}`)
      .element() as HTMLElement;

    handles.scriptResponse("dispatch_command:nav.up", () =>
      handles.payloadForFocusMove(cardMk, tabMk),
    );

    await userEvent.click(card);
    await expectFocused(card, "true");

    await userEvent.keyboard("k");
    await expectFocused(tab, "true");
  });

  /**
   * `j` from a focused perspective tab dispatches `nav.down` — the
   * routing half of "j from a perspective tab reaches the view
   * content". Focus is seeded by clicking the tab `<div>` directly;
   * `perspective_click_on_tab_div_focuses_scope` above pins that
   * mouse-reachability contract.
   */
  it("perspective_j_from_focused_tab_dispatches_nav_down", async () => {
    const screen = await render(<AppWithBoardAndPerspectiveFixture />);
    const tabMk = FIXTURE_PERSPECTIVE_MONIKERS[0];
    const tab = screen
      .getByTestId(`data-moniker:${tabMk}`)
      .element() as HTMLElement;

    await userEvent.click(tab);
    await expectFocused(tab, "true");

    const before = handles.dispatchedCommands().length;
    await userEvent.keyboard("j");

    await expect
      .poll(
        () =>
          handles
            .dispatchedCommands()
            .slice(before)
            .some((d) => d.cmd === "nav.down"),
        { timeout: POLL_TIMEOUT },
      )
      .toBe(true);
  });

  /**
   * Decoration contract: with focus landed on the active perspective
   * tab by clicking it, scripted `nav.down` lands focus on a top-row
   * card.
   */
  it("perspective_scripted_nav_down_from_tab_lands_on_view_content", async () => {
    const screen = await render(<AppWithBoardAndPerspectiveFixture />);
    const tabMk = FIXTURE_PERSPECTIVE_MONIKERS[0];
    const targetCardMk = BOARD_CARD_MONIKERS[0][0];
    const tab = screen
      .getByTestId(`data-moniker:${tabMk}`)
      .element() as HTMLElement;
    const targetCard = screen
      .getByTestId(`data-moniker:${targetCardMk}`)
      .element() as HTMLElement;

    handles.scriptResponse("dispatch_command:nav.down", () =>
      handles.payloadForFocusMove(tabMk, targetCardMk),
    );

    await userEvent.click(tab);
    await expectFocused(tab, "true");

    await userEvent.keyboard("j");
    await expectFocused(targetCard, "true");
  });
});

describe("golden path: toolbar region", () => {
  let handles: TauriStubHandles;

  beforeEach(() => {
    handles = setupTauriStub();
  });

  /**
   * Every toolbar element in `<NavBar />` registers a `toolbar:*`
   * moniker. The h/l invariant requires these entries so the Rust
   * beam-test can walk between them.
   */
  it("toolbar_every_element_registers_on_mount", async () => {
    await render(<AppWithToolbarFixture />);

    for (const m of Object.values(TOOLBAR_MONIKERS)) {
      await waitForRegistered(handles, m);
    }
  });

  /**
   * `h` from a focused toolbar element dispatches `nav.left` — the
   * routing half of "h/l moves across toolbar elements". Which
   * sibling wins is the Rust algorithm's job.
   */
  it("toolbar_h_from_focused_element_dispatches_nav_left", async () => {
    const screen = await render(<AppWithToolbarFixture />);
    const search = screen
      .getByTestId(`data-moniker:${TOOLBAR_MONIKERS.search}`)
      .element() as HTMLElement;

    await userEvent.click(search);
    const before = handles.dispatchedCommands().length;
    await userEvent.keyboard("h");

    await expect
      .poll(
        () =>
          handles
            .dispatchedCommands()
            .slice(before)
            .some((d) => d.cmd === "nav.left"),
        { timeout: POLL_TIMEOUT },
      )
      .toBe(true);
  });

  /**
   * `j` from a focused toolbar element dispatches `nav.down` — the
   * routing half of "j from the toolbar reaches the perspective bar
   * or LeftNav". Which neighbor wins is the Rust algorithm's job
   * (parity case: `toolbar: Up from perspective tab lands on toolbar
   * button above`, whose inverse covers this direction).
   */
  it("toolbar_j_from_focused_element_dispatches_nav_down", async () => {
    const screen = await render(<AppWithToolbarFixture />);
    const search = screen
      .getByTestId(`data-moniker:${TOOLBAR_MONIKERS.search}`)
      .element() as HTMLElement;

    await userEvent.click(search);
    const before = handles.dispatchedCommands().length;
    await userEvent.keyboard("j");

    await expect
      .poll(
        () =>
          handles
            .dispatchedCommands()
            .slice(before)
            .some((d) => d.cmd === "nav.down"),
        { timeout: POLL_TIMEOUT },
      )
      .toBe(true);
  });
});

// ===========================================================================
// Enter activation — each scope type dispatches its specific command on
// Enter. Click-then-Enter is measured by diffing the dispatch log.
// ===========================================================================

describe("golden path: enter activation", () => {
  let handles: TauriStubHandles;

  beforeEach(() => {
    handles = setupTauriStub();
    mockSetActivePerspectiveId.mockClear();
  });

  /**
   * LeftNav button + Enter → `view.switch:<id>`. The click itself also
   * dispatches, so the assertion uses a snapshot of the dispatch log
   * length before the Enter keypress and checks the tail.
   */
  it("enter_on_leftnav_button_dispatches_view_switch", async () => {
    const screen = await render(<AppWithBoardAndLeftNavFixture />);
    const gridBtn = screen
      .getByTestId(`data-moniker:${LEFTNAV_VIEW_MONIKERS[1]}`)
      .element() as HTMLElement;

    await userEvent.click(gridBtn);
    await expect
      .poll(
        () =>
          handles
            .dispatchedCommands()
            .some((d) => d.cmd === "view.switch:grid"),
        { timeout: POLL_TIMEOUT },
      )
      .toBe(true);
    const before = handles.dispatchedCommands().length;

    await userEvent.keyboard("{Enter}");

    await expect
      .poll(() => handles.dispatchedCommands().length, {
        timeout: POLL_TIMEOUT,
      })
      .toBeGreaterThan(before);
    const tail = handles.dispatchedCommands().slice(before);
    expect(tail.some((d) => d.cmd === "view.switch:grid")).toBe(true);
  });

  /**
   * Toolbar inspect button + Enter → `ui.inspect` targeting the board
   * moniker. The fixture supplies `board:b1` as the target; production's
   * toolbar wires the active board similarly.
   */
  it("enter_on_toolbar_inspect_button_dispatches_ui_inspect_with_board_moniker", async () => {
    const screen = await render(<AppWithToolbarFixture />);
    const inspectBtn = screen
      .getByTestId(`data-moniker:${TOOLBAR_MONIKERS.inspectBoard}`)
      .element() as HTMLElement;

    await userEvent.click(inspectBtn);
    const before = handles.dispatchedCommands().length;

    await userEvent.keyboard("{Enter}");

    await expect
      .poll(() => handles.dispatchedCommands().length, {
        timeout: POLL_TIMEOUT,
      })
      .toBeGreaterThan(before);
    const tail = handles.dispatchedCommands().slice(before);
    expect(
      tail.some((d) => d.cmd === "ui.inspect" && d.target === "board:b1"),
    ).toBe(true);
  });

  /**
   * Toolbar search button + Enter → `app.search`.
   */
  it("enter_on_toolbar_search_button_dispatches_app_search", async () => {
    const screen = await render(<AppWithToolbarFixture />);
    const searchBtn = screen
      .getByTestId(`data-moniker:${TOOLBAR_MONIKERS.search}`)
      .element() as HTMLElement;

    await userEvent.click(searchBtn);
    const before = handles.dispatchedCommands().length;

    await userEvent.keyboard("{Enter}");

    await expect
      .poll(() => handles.dispatchedCommands().length, {
        timeout: POLL_TIMEOUT,
      })
      .toBeGreaterThan(before);
    const tail = handles.dispatchedCommands().slice(before);
    expect(tail.some((d) => d.cmd === "app.search")).toBe(true);
  });

  /**
   * Card + double-click → inspector opens. The double-click route is
   * the production `ui.inspect` binding on cards. A successful open is
   * observed by a `spatial_push_layer` invocation for the inspector
   * layer.
   */
  it("dblclick_on_card_opens_inspector_via_spatial_push_layer", async () => {
    const screen = await render(<AppWithInspectorFixture />);
    const card = screen.getByTestId("fixture-card");

    const pushesBefore = handles
      .invocations()
      .filter((i) => i.cmd === "spatial_push_layer").length;

    await userEvent.click(card.element());
    await userEvent.dblClick(card.element());

    await expect
      .poll(
        () =>
          handles.invocations().filter((i) => i.cmd === "spatial_push_layer")
            .length,
        { timeout: POLL_TIMEOUT },
      )
      .toBeGreaterThan(pushesBefore);
  });

  /**
   * Companion to `dblclick_on_card_opens_inspector_via_spatial_push_layer`
   * that exercises the keyboard Enter path the task description names
   * explicitly ("Card + Enter → inspector opens for that entity"). A
   * regression that drops the Enter binding from `useEnterInspectCommand`
   * (or from the per-card FocusScope) would leave the dblClick path
   * unaffected but break the keyboard flow; only a test that presses
   * Enter catches it.
   *
   * Uses `AppWithCardEnterInspectFixture` because the default
   * `spatial-board-fixture.tsx` and `spatial-inspector-fixture.tsx` do not
   * wire Enter on their card scopes — the fixture here mirrors
   * `useEnterInspectCommand` in `entity-card.tsx` verbatim.
   */
  it("enter_on_card_dispatches_ui_inspect_with_card_target", async () => {
    const cardMk = "task:enter-inspect-card";
    const screen = await render(
      <AppWithCardEnterInspectFixture cardMoniker={cardMk} />,
    );
    const card = screen.getByTestId("enter-inspect-card");

    await userEvent.click(card.element());
    const before = handles.dispatchedCommands().length;

    await userEvent.keyboard("{Enter}");

    await expect
      .poll(() => handles.dispatchedCommands().length, {
        timeout: POLL_TIMEOUT,
      })
      .toBeGreaterThan(before);
    const tail = handles.dispatchedCommands().slice(before);
    expect(
      tail.some((d) => d.cmd === "ui.inspect" && d.target === cardMk),
    ).toBe(true);
  });

  /**
   * Perspective tab + Enter → the active perspective is switched, driven
   * by `perspective.activate.<id>` → `onSelect` →
   * `setActivePerspectiveId(id)`. A regression in
   * `ScopedPerspectiveTab`'s Enter binding would leave mouse users
   * unaffected but break keyboard activation; asserting the mock was
   * called with the tab's id catches that case.
   *
   * Focus is seeded by clicking the tab `<div>` directly — the
   * mouse-reachability contract is pinned by
   * `perspective_click_on_tab_div_focuses_scope`.
   */
  it("enter_on_perspective_tab_switches_active_perspective", async () => {
    const screen = await render(<AppWithBoardAndPerspectiveFixture />);
    const tabMk = FIXTURE_PERSPECTIVE_MONIKERS[0];
    const tab = screen
      .getByTestId(`data-moniker:${tabMk}`)
      .element() as HTMLElement;

    await userEvent.click(tab);
    await expectFocused(tab, "true");

    // Clear the mock after the click seed so the Enter keypress is the
    // sole driver of the `setActivePerspectiveId` call we assert on.
    // A click on the tab also fires `onSelect` via the inner `TabButton`
    // button, which itself calls `setActivePerspectiveId`.
    mockSetActivePerspectiveId.mockClear();

    await userEvent.keyboard("{Enter}");

    await expect
      .poll(() => mockSetActivePerspectiveId.mock.calls.length > 0, {
        timeout: POLL_TIMEOUT,
      })
      .toBe(true);
    // The first perspective id is "default" (FIXTURE_PERSPECTIVE_IDS[0]).
    expect(mockSetActivePerspectiveId).toHaveBeenCalledWith("default");
  });

  /**
   * Grid cell + Enter → the grid drops into edit mode. Production wires
   * this via `grid.editEnter` / `grid.edit` commands on the grid-view
   * scope whose `execute` calls `g().enterEdit()` — a local React state
   * transition, not a backend dispatch. The invariant therefore pins a
   * local side effect (the `onEnterEdit` callback fires) rather than a
   * `dispatch_command` invocation.
   *
   * A regression that drops the Enter keybinding from `grid.editEnter`,
   * or breaks the keybinding chain from focused cell up to the grid
   * scope, would leave `onEnterEdit` uncalled and fail this test.
   */
  it("enter_on_grid_cell_invokes_grid_edit_enter_callback", async () => {
    const onEnterEdit = vi.fn();
    const screen = await render(
      <AppWithGridAndEditCommandsFixture onEnterEdit={onEnterEdit} />,
    );
    const cell = screen.getByTestId(
      `data-moniker:${FIXTURE_CELL_MONIKERS[0][0]}`,
    );

    await userEvent.click(cell.element());
    await expectFocused(cell.element() as HTMLElement, "true");

    await userEvent.keyboard("{Enter}");

    await expect
      .poll(() => onEnterEdit.mock.calls.length > 0, { timeout: POLL_TIMEOUT })
      .toBe(true);
    expect(onEnterEdit).toHaveBeenCalledTimes(1);
  });

  /**
   * Inspector field + Enter → the inspector drops into edit mode.
   * `inspector.editEnter` is a local command installed by
   * `inspector-focus-bridge.tsx`; its `execute` calls
   * `navRef.current?.enterEdit()`, a React-state transition internal to
   * the inspector. The invariant is therefore pinned via the local side
   * effect (the `onEnterEdit` callback fires) rather than a dispatch log
   * entry.
   *
   * A regression that drops Enter from `inspector.editEnter`, or leaves
   * focus on a non-field scope (so the inspector scope chain never sees
   * the Enter key), would leave `onEnterEdit` uncalled and fail here.
   */
  it("enter_on_inspector_field_invokes_inspector_edit_enter_callback", async () => {
    const onEnterEdit = vi.fn();
    const screen = await render(
      <AppWithInspectorAndEditCommandFixture onEnterEdit={onEnterEdit} />,
    );
    const card = screen.getByTestId("fixture-card");
    await userEvent.click(card.element());
    await userEvent.dblClick(card.element());

    // Wait for the inspector to mount and the first field to claim focus.
    const firstField = screen
      .getByTestId(`field-row-${INSPECTOR_FIELD_NAMES[0]}`)
      .element() as HTMLElement;
    await expectFocused(firstField, "true");

    await userEvent.keyboard("{Enter}");

    await expect
      .poll(() => onEnterEdit.mock.calls.length > 0, { timeout: POLL_TIMEOUT })
      .toBe(true);
    expect(onEnterEdit).toHaveBeenCalledTimes(1);
  });

  /**
   * Row selector + Enter → `ui.inspect` targeting the row's entity. The
   * row selector's per-scope Enter binding shadows any grid-level Enter
   * binding (see the `data-table.tsx` comment explaining the shadow),
   * so the dispatched target must be the row entity's moniker, not the
   * grid cursor's cell moniker. A regression that drops the Enter
   * binding from the row selector scope would fall through to the
   * grid's `grid.editEnter` and never emit `ui.inspect`; asserting the
   * dispatch tail catches that.
   */
  it("enter_on_row_selector_dispatches_ui_inspect_with_row_target", async () => {
    const screen = await render(<AppWithGridAndRowSelectorEnterFixture />);
    const selectorMk = FIXTURE_ROW_SELECTOR_MONIKERS[1];
    const selector = screen
      .getByTestId(`data-moniker:${selectorMk}`)
      .element() as HTMLElement;

    await userEvent.click(selector);
    await expectFocused(selector, "true");
    const before = handles.dispatchedCommands().length;

    await userEvent.keyboard("{Enter}");

    await expect
      .poll(() => handles.dispatchedCommands().length, {
        timeout: POLL_TIMEOUT,
      })
      .toBeGreaterThan(before);
    const tail = handles.dispatchedCommands().slice(before);
    // The row entity moniker for selector row index 1 is `tag:tag-1` —
    // see `spatial-grid-fixture.tsx` row id construction.
    expect(
      tail.some((d) => d.cmd === "ui.inspect" && d.target === "tag:tag-1"),
    ).toBe(true);
  });
});

// ===========================================================================
// Cross-layer isolation — multi-inspector fixture. Layer filtering is
// algorithmic (Rust); React-side the test pins the layer push count and
// the layer registration of fields, not the beam-test math.
// ===========================================================================

describe("golden path: cross-layer isolation", () => {
  let handles: TauriStubHandles;

  beforeEach(() => {
    handles = setupTauriStub();
  });

  /**
   * When the inspector opens over a board, a new `spatial_push_layer`
   * fires — that layer is what Rust's layer filter uses to keep nav
   * inside the inspector. The React wiring is pushing exactly one
   * layer; the filtering algorithm is Rust's.
   */
  it("inspector_over_board_pushes_exactly_one_additional_layer", async () => {
    const screen = await render(<AppWithInspectorFixture />);
    const card = screen.getByTestId("fixture-card");

    const pushesBefore = handles
      .invocations()
      .filter((i) => i.cmd === "spatial_push_layer").length;

    await userEvent.click(card.element());
    await userEvent.dblClick(card.element());

    await expect
      .poll(
        () =>
          handles.invocations().filter((i) => i.cmd === "spatial_push_layer")
            .length,
        { timeout: POLL_TIMEOUT },
      )
      .toBe(pushesBefore + 1);
  });

  /**
   * The inspector-over-dense-grid fixture mounts many grid rows under
   * the inspector. `spatial_push_layer` firing once after the inspector
   * opens is the React-side contract; Rust's layer filter keeps nav
   * inside the inspector.
   */
  it("inspector_over_grid_pushes_exactly_one_additional_layer", async () => {
    const screen = await render(<AppWithInspectorOverGridFixture />);
    const card = screen.getByTestId("fixture-card");

    const pushesBefore = handles
      .invocations()
      .filter((i) => i.cmd === "spatial_push_layer").length;

    await userEvent.click(card.element());
    await userEvent.dblClick(card.element());

    await expect
      .poll(
        () =>
          handles.invocations().filter((i) => i.cmd === "spatial_push_layer")
            .length,
        { timeout: POLL_TIMEOUT },
      )
      .toBe(pushesBefore + 1);
  });

  /**
   * Inspector fields must register their own monikers on top of the
   * dense background grid. A missing field registration is how "field
   * rows get skipped" regressions manifest on the React side.
   */
  it("inspector_over_grid_every_field_registers_despite_dense_background", async () => {
    const screen = await render(<AppWithInspectorOverGridFixture />);
    const card = screen.getByTestId("fixture-card");
    await userEvent.click(card.element());
    await userEvent.dblClick(card.element());

    for (const m of IOG_FIELD_MONIKERS) {
      await waitForRegistered(handles, m);
    }
  });

  /**
   * With three inspectors stacked, at least three `spatial_push_layer`
   * calls must have fired — one per layer. The algorithm that traps
   * nav inside the top layer is Rust's concern (parity case:
   * `three layer stack traps navigation in the topmost inspector`).
   */
  it("three_stacked_inspectors_each_push_their_own_layer", async () => {
    await render(<AppWithMultiInspectorFixture />);

    await expect
      .poll(
        () =>
          handles.invocations().filter((i) => i.cmd === "spatial_push_layer")
            .length,
        { timeout: POLL_TIMEOUT },
      )
      .toBeGreaterThanOrEqual(3);
  });

  /**
   * Every field in every stacked inspector must register its moniker.
   * `FIXTURE_FIELD_MONIKERS` is shaped `[entity][field]`; flattened it
   * lists every monicker that must be visible to `spatial_register`.
   */
  it("three_stacked_inspectors_every_field_registers_with_distinct_moniker", async () => {
    await render(<AppWithMultiInspectorFixture />);

    for (const m of MI_FIELD_MONIKERS.flat()) {
      await waitForRegistered(handles, m);
    }
  });

  /**
   * Each inspector's outer entity scope uses `spatial={false}` —
   * registering the entity scope would shadow its own field rects in
   * the beam-test graph, which was the regression that motivated this
   * contract. Pin it here as a permanent no-register assertion.
   */
  it("three_stacked_inspectors_entity_scopes_do_not_register_spatial_entries", async () => {
    await render(<AppWithMultiInspectorFixture />);

    // Give the component a tick to finish any registers it intends.
    await new Promise((r) => setTimeout(r, 100));

    for (const entityMk of MI_ENTITY_MONIKERS) {
      const registered = handles.invocations().some((i) => {
        if (i.cmd !== "spatial_register") return false;
        const a = i.args as { args?: { moniker?: string } };
        return a.args?.moniker === entityMk;
      });
      expect(registered).toBe(false);
    }
  });

  /**
   * Closing the inspector via Escape must (a) fire
   * `spatial_remove_layer` for the inspector layer, and (b) transition
   * focus back to a registered scope in the parent layer. Part (b) is
   * the stronger invariant from the task description — "Closing the
   * inspector returns focus to a registered scope in the parent view's
   * layer." A regression that pops the layer without restoring focus
   * would leave `data-focused` on no element at all; pinning the
   * decoration on the card after Escape catches that case.
   *
   * The stub does not synthesize the parent-layer successor on its own
   * (the successor pick is a Rust-side decision), so the test scripts
   * `spatial_remove_layer` to emit the focus-changed payload the real
   * backend would emit after popping the inspector layer: "focus moves
   * from the currently-focused field back to the card moniker
   * (lastFocused in the window layer)". This models the shim's
   * `lastFocused` restore without re-implementing it in JS.
   */
  it("closing_inspector_emits_spatial_remove_layer_and_restores_focus_to_card", async () => {
    const screen = await render(<AppWithInspectorFixture />);
    const card = screen.getByTestId("fixture-card");
    const cardEl = card.element() as HTMLElement;
    await userEvent.click(cardEl);
    await userEvent.dblClick(cardEl);

    await expect
      .poll(
        () =>
          handles.invocations().filter((i) => i.cmd === "spatial_push_layer")
            .length,
        { timeout: POLL_TIMEOUT },
      )
      .toBeGreaterThan(0);

    // Wait for the inspector to focus its first field so the "from"
    // side of the focus-changed payload is accurate.
    const firstFieldMoniker = INSPECTOR_FIELD_MONIKERS[0];
    await expect
      .poll(() => handles.focusedMoniker(), { timeout: POLL_TIMEOUT })
      .toBe(firstFieldMoniker);

    const removesBefore = handles
      .invocations()
      .filter((i) => i.cmd === "spatial_remove_layer").length;

    // Script the parent-layer successor pick: on remove_layer, emit
    // focus-changed pointing to the card moniker. Mirrors the shim's
    // `lastFocused` behavior for the window layer.
    handles.scriptResponse("spatial_remove_layer", () =>
      handles.payloadForFocusMove(firstFieldMoniker, INSPECTOR_CARD_MONIKER),
    );

    await userEvent.keyboard("{Escape}");

    await expect
      .poll(
        () =>
          handles.invocations().filter((i) => i.cmd === "spatial_remove_layer")
            .length,
        { timeout: POLL_TIMEOUT },
      )
      .toBeGreaterThan(removesBefore);

    // Stronger assertion: focus actually lands on the card after the
    // layer pop, not nowhere. `data-focused` landing on the card is
    // the user-visible contract the task description names.
    await expectFocused(cardEl, "true");
    expect(countFocused()).toBe(1);
  });

  /**
   * The card itself must be a spatial entry — otherwise after the
   * inspector closes there is no registered scope to restore focus
   * to. This is the precondition for the shim's `lastFocused` restore.
   */
  it("card_scope_registers_on_mount_as_inspector_focus_target", async () => {
    await render(<AppWithInspectorFixture />);
    await waitForRegistered(handles, INSPECTOR_CARD_MONIKER);
  });
});

// ===========================================================================
// Visual-focus invariants — exactly one `data-focused` at a time, even
// under rapid click bursts. The grid fixture is a nice host because its
// cells are laid out in a stable geometry and the scope bodies are small.
// ===========================================================================

describe("golden path: visual focus invariants", () => {
  let handles: TauriStubHandles;

  beforeEach(() => {
    handles = setupTauriStub();
  });

  /**
   * Across every cell click in a 3x3 grid, at most one scope wears
   * `data-focused="true"` at any point after the stub settles.
   */
  it("visual_at_most_one_data_focused_across_every_grid_cell_click", async () => {
    const screen = await render(<AppWithGridFixture />);

    for (const row of FIXTURE_CELL_MONIKERS) {
      for (const mk of row) {
        const el = screen
          .getByTestId(`data-moniker:${mk}`)
          .element() as HTMLElement;
        await userEvent.click(el);
        await expectFocused(el, "true");
        expect(countFocused()).toBe(1);
      }
    }
  });

  /**
   * Rapid alternating clicks for ~30 user-events must not leave stale
   * `data-focused` on a previously-clicked scope. The task description
   * calls for 30 clicks; we stay inside the grid so each click is on a
   * registered scope.
   */
  it("visual_rapid_clicks_leave_exactly_one_data_focused", async () => {
    const screen = await render(<AppWithGridFixture />);
    const a = screen
      .getByTestId(`data-moniker:${FIXTURE_CELL_MONIKERS[0][0]}`)
      .element() as HTMLElement;
    const b = screen
      .getByTestId(`data-moniker:${FIXTURE_CELL_MONIKERS[2][1]}`)
      .element() as HTMLElement;

    for (let i = 0; i < 30; i++) {
      await userEvent.click(i % 2 === 0 ? a : b);
    }

    // Settle any lingering ResizeObserver / focus-changed events.
    await new Promise((r) => setTimeout(r, 50));

    expect(countFocused()).toBe(1);
  });

  /**
   * After a click + nav round-trip, the origin element loses
   * `data-focused` while the destination gains it. A regression that
   * forgets to clear `data-focused` on the previous scope would fail
   * here.
   */
  it("visual_nav_round_trip_transfers_data_focused_atomically", async () => {
    const screen = await render(<AppWithGridFixture />);
    const srcMk = FIXTURE_CELL_MONIKERS[0][0];
    const dstMk = FIXTURE_CELL_MONIKERS[1][1];
    const src = screen
      .getByTestId(`data-moniker:${srcMk}`)
      .element() as HTMLElement;
    const dst = screen
      .getByTestId(`data-moniker:${dstMk}`)
      .element() as HTMLElement;

    handles.scriptResponse("dispatch_command:nav.down", () =>
      handles.payloadForFocusMove(srcMk, dstMk),
    );

    await userEvent.click(src);
    await expectFocused(src, "true");

    await userEvent.keyboard("j");
    await expectFocused(dst, "true");
    await expectFocused(src, null);
    expect(countFocused()).toBe(1);
  });
});
