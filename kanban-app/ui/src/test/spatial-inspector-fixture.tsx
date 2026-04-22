/**
 * Deterministic inspector-nav fixture for vitest-browser spatial-nav tests.
 *
 * ## Purpose
 *
 * Pairs with `spatial-grid-fixture.tsx` to exercise inspector-layer
 * navigation without requiring a real Tauri backend, SchemaProvider, or
 * the production entity store. Mirrors the production stack shape as
 * closely as possible:
 *
 * - `FocusLayer name="window"` containing a single card `FocusScope`.
 * - Double-clicking the card dispatches `ui.inspect`, which toggles
 *   local state to mount the inspector.
 * - When open, the inspector renders its own `FocusLayer name="inspector"`
 *   with four vertically stacked field `FocusScope`s.
 * - `app.dismiss` registered inside the inspector scope closes it,
 *   causing `FocusLayer`'s cleanup to call `spatial_remove_layer`, which
 *   the shim resolves by restoring focus to the window layer's
 *   `lastFocused` (the card).
 *
 * ## Why another fixture?
 *
 * `spatial-grid-fixture.tsx` models one layer (the board). Inspector
 * navigation is explicitly a second-layer concern: layer stack length,
 * layer focus memory, and nav trapping inside the active layer all only
 * show up when two layers coexist. Rather than bolt inspector behaviour
 * onto the grid fixture, this file is a sibling with its own shape.
 *
 * ## What the fixture does NOT do
 *
 * - No real schema — fields are plain strings declared inline.
 * - No real editor — the inspector body is just the focus structure.
 *   Edit mode is out of scope for this fixture; a separate fixture can
 *   cover it when needed.
 * - No production `<EntityInspector>` — we want to isolate the
 *   layer/focus contract from the entity-inspector rendering quirks.
 *   When a production edit breaks inspector nav, tests against this
 *   fixture fail first, pointing the fix at the spatial plumbing.
 */

import { useEffect, useRef, useState } from "react";
import { CommandScopeProvider, type CommandDef } from "@/lib/command-scope";
import {
  EntityFocusProvider,
  useEntityFocus,
} from "@/lib/entity-focus-context";
import { FocusLayer } from "@/components/focus-layer";
import { FocusScope } from "@/components/focus-scope";
import { fieldMoniker, moniker } from "@/lib/moniker";
import { FixtureShell } from "./spatial-fixture-shell";

/** Canonical entity type + id for the card the inspector opens on. */
export const FIXTURE_CARD_TYPE = "task";
export const FIXTURE_CARD_ID = "card-1-1";
export const FIXTURE_CARD_MONIKER = moniker(FIXTURE_CARD_TYPE, FIXTURE_CARD_ID);

/**
 * The four field names rendered by the inspector, in display order.
 *
 * Four is the minimum that lets tests distinguish "j moves to the next
 * field", "j at last clamps", and "k at first clamps" without making
 * any of them land on the same assertion.
 */
export const FIXTURE_FIELD_NAMES = ["title", "status", "body", "tags"] as const;

/** Pre-computed field monikers so tests can reference them by index. */
export const FIXTURE_FIELD_MONIKERS: readonly string[] =
  FIXTURE_FIELD_NAMES.map((name) =>
    fieldMoniker(FIXTURE_CARD_TYPE, FIXTURE_CARD_ID, name),
  );

/**
 * One inspector field row. Wraps a plain label div in a `FocusScope` so
 * each field is its own spatial entry — matches production's
 * `EntityInspector` where every field row is wrapped in a `FocusScope`
 * keyed by `fieldMoniker(type, id, fieldName)`.
 *
 * Row height is hard-coded so `getBoundingClientRect()` produces
 * predictable rects in the headless browser (the beam test depends on
 * real geometry).
 */
function FixtureFieldRow({ fieldName }: { fieldName: string }) {
  const mk = fieldMoniker(FIXTURE_CARD_TYPE, FIXTURE_CARD_ID, fieldName);
  return (
    <FocusScope
      moniker={mk}
      commands={[]}
      data-testid={`field-row-${fieldName}`}
      style={{
        height: "40px",
        padding: "8px",
        borderBottom: "1px solid #ccc",
      }}
    >
      <span>{fieldName}</span>
    </FocusScope>
  );
}

/**
 * Focus the first field on mount and restore the previously focused
 * moniker on unmount.
 *
 * Mirrors `useFirstFieldFocus` in `entity-inspector.tsx` so the fixture
 * exercises the same "inspector opens → first field claims focus →
 * inspector closes → caller regains focus" lifecycle. Focus memory on
 * the shim's window layer handles the actual restore; this hook only
 * drives the initial setFocus into the inspector.
 */
function useFirstFieldFocusOnMount(firstFieldMoniker: string): void {
  const { setFocus } = useEntityFocus();
  const setFocusRef = useRef(setFocus);
  setFocusRef.current = setFocus;

  useEffect(() => {
    setFocusRef.current(firstFieldMoniker);
  }, [firstFieldMoniker]);
}

interface InspectorBodyProps {
  onClose: () => void;
}

/**
 * The inspector panel: a `FocusLayer` wrapping the field rows with
 * an `app.dismiss` binding that closes the panel.
 *
 * Registering `app.dismiss` with an `execute` handler in the inspector
 * scope shadows the global `app.dismiss` (keybindings.ts routes Escape
 * → `app.dismiss`). The shim's `removeLayer` pops the inspector layer
 * and restores focus to the window layer's `lastFocused` entry (the
 * card).
 */
function InspectorBody({ onClose }: InspectorBodyProps) {
  const firstFieldMoniker = FIXTURE_FIELD_MONIKERS[0];
  useFirstFieldFocusOnMount(firstFieldMoniker);

  const inspectorCommands: CommandDef[] = [
    {
      id: "app.dismiss",
      name: "Close Inspector",
      keys: { vim: "Escape", cua: "Escape" },
      execute: onClose,
    },
  ];

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
          {FIXTURE_FIELD_NAMES.map((name) => (
            <FixtureFieldRow key={name} fieldName={name} />
          ))}
        </div>
      </CommandScopeProvider>
    </FocusLayer>
  );
}

/**
 * Inspector-navigation fixture ready for rendering in vitest-browser tests.
 *
 * Usage:
 * ```tsx
 * setupTauriStub();
 * const screen = await render(<AppWithInspectorFixture />);
 * const card = screen.getByTestId("fixture-card");
 * await userEvent.dblClick(card); // opens inspector
 * ```
 *
 * All Tauri IPC goes through the boundary stub — no real backend
 * involvement.
 * The inspector mounts/unmounts purely via React state, driven by
 * `ui.inspect` (open) and `app.dismiss` (close).
 *
 * ## Layering
 *
 * The shared `FixtureShell` gives us the window-layer `FocusLayer`, the
 * keydown handler, and the standard `nav.*` command set. We extend it
 * with a local `ui.inspect` command (wired to `setInspectorOpen(true)`)
 * and the inspector's vim `g g` / `Shift+G` bindings for
 * `nav.first`/`nav.last`. The `<InspectorBody>` overlay is rendered as
 * a sibling of the fixture body so it sits inside the same
 * `CommandScopeProvider` — that's how production's `ui.inspect` routes
 * the open event to the inspector stack.
 */
export function AppWithInspectorFixture() {
  const [inspectorOpen, setInspectorOpen] = useState(false);
  const openInspector = () => setInspectorOpen(true);
  const closeInspector = () => setInspectorOpen(false);

  // Keep the open callback stable across renders so `extraCommands`
  // doesn't re-register a new `ui.inspect` entry every render.
  const openInspectorRef = useRef(openInspector);
  openInspectorRef.current = openInspector;

  const extraCommands: CommandDef[] = [
    {
      id: "ui.inspect",
      name: "Open Inspector",
      // execute signature `(opts?) => void | Promise<void>` — we ignore
      // the target moniker because the fixture only has one card.
      execute: () => {
        openInspectorRef.current();
      },
    },
  ];

  return (
    <EntityFocusProvider>
      <FixtureShell
        extraCommands={extraCommands}
        navOverrides={{ navFirstVim: "g g", navLastVim: "Shift+G" }}
      >
        <div
          data-testid="inspector-fixture-root"
          style={{
            width: "400px",
            padding: "16px",
          }}
        >
          <FocusScope
            moniker={FIXTURE_CARD_MONIKER}
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
        {inspectorOpen && <InspectorBody onClose={closeInspector} />}
      </FixtureShell>
    </EntityFocusProvider>
  );
}
