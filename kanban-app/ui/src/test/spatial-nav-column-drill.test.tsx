/**
 * Enter on a focused board column header drills to the first card
 * in that column — task 01KPX6FSPY6V15JATXMY6AGRER.
 *
 * This is the Bug B reproducer from 01KPX6E0QPNRWZTQXGXX2MBEMV:
 * after the Space rebind moved `ui.inspect` off Enter, column
 * headers were left with no Enter binding at all. Pressing Enter
 * on a focused column header did nothing. This task fills that gap
 * by adding a per-header `column.drill.<id>` command bound to
 * Enter whose execute calls `setFocus(firstCardMoniker)`.
 *
 * ## Shape
 *
 * The test mounts the minimum tree to reproduce the drill contract:
 *
 *   <EntityFocusProvider>
 *     <FocusLayer name="window">
 *       <CommandScopeProvider commands=[]>
 *         <KeybindingHandler mode="cua" />
 *         <ColumnHeaderDrillFixture firstCardMoniker="task:first">
 *           ... header scope here ...
 *         </ColumnHeaderDrillFixture>
 *         <FocusScope moniker="task:first" ...>card</FocusScope>
 *       </CommandScopeProvider>
 *     </FocusLayer>
 *   </EntityFocusProvider>
 *
 * The column header's `FocusScope` is wired with the drill command —
 * same contract as the eventual production code. Enter is pressed
 * after clicking the header; we assert the focused moniker is now
 * the card's.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { userEvent } from "vitest/browser";
import { render } from "vitest-browser-react";
import { useContext, useEffect, useRef, useCallback, useMemo } from "react";

vi.mock("@tauri-apps/api/core", async () => {
  const { tauriCoreMock } = await import("./setup-tauri-stub");
  return tauriCoreMock();
});
vi.mock("@tauri-apps/api/event", async () => {
  const { tauriEventMock } = await import("./setup-tauri-stub");
  return tauriEventMock();
});
vi.mock("@tauri-apps/api/window", async () => {
  const { tauriWindowMock } = await import("./setup-tauri-stub");
  return tauriWindowMock();
});
vi.mock("@tauri-apps/api/webviewWindow", async () => {
  const { tauriWebviewWindowMock } = await import("./setup-tauri-stub");
  return tauriWebviewWindowMock();
});
vi.mock("@tauri-apps/plugin-log", async () => {
  const { tauriPluginLogMock } = await import("./setup-tauri-stub");
  return tauriPluginLogMock();
});

import { setupTauriStub, type TauriStubHandles } from "./setup-tauri-stub";
import {
  EntityFocusProvider,
  useEntityFocus,
  useFocusedScope,
} from "@/lib/entity-focus-context";
import { FocusLayer } from "@/components/focus-layer";
import { FocusScope } from "@/components/focus-scope";
import {
  CommandScopeContext,
  CommandScopeProvider,
  useDispatchCommand,
  type CommandDef,
} from "@/lib/command-scope";
import {
  createKeyHandler,
  extractScopeBindings,
  type KeymapMode,
} from "@/lib/keybindings";

/** Inline replica of `KeybindingHandler` — see notes in spatial-nav-space-scroll-bug.test.tsx */
function KeybindingHandler({ mode }: { mode: KeymapMode }) {
  const dispatch = useDispatchCommand();
  const focusedScope = useFocusedScope();
  const treeScope = useContext(CommandScopeContext);

  const dispatchRef = useRef(dispatch);
  dispatchRef.current = dispatch;
  const focusedScopeRef = useRef(focusedScope);
  focusedScopeRef.current = focusedScope;
  const treeScopeRef = useRef(treeScope);
  treeScopeRef.current = treeScope;

  const executeCommand = useCallback(async (id: string): Promise<boolean> => {
    await dispatchRef.current(id);
    return true;
  }, []);

  useEffect(() => {
    const handler = createKeyHandler(mode, executeCommand, () =>
      extractScopeBindings(
        focusedScopeRef.current ?? treeScopeRef.current,
        mode,
      ),
    );
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [mode, executeCommand]);

  return null;
}

const HEADER_MONIKER = "column:todo.name";
const FIRST_CARD_MONIKER = "task:first";
const SECOND_CARD_MONIKER = "task:second";

/**
 * Column-header fixture that mirrors the production header's drill
 * contract: when the header is focused and Enter is pressed, focus
 * moves to `firstCardMoniker`.
 *
 * Production will implement this inside `ColumnHeader` in
 * `column-view.tsx`. The fixture duplicates the exact CommandDef
 * shape so the test pins the contract independent of that file's
 * implementation.
 */
function ColumnHeaderDrillFixture({
  firstCardMoniker,
}: {
  firstCardMoniker?: string;
}) {
  const { setFocus } = useEntityFocus();

  const commands = useMemo<CommandDef[]>(() => {
    if (!firstCardMoniker) return [];
    return [
      {
        id: `column.drill.todo`,
        name: "Focus first card",
        keys: { vim: "Enter", cua: "Enter", emacs: "Enter" },
        execute: () => setFocus(firstCardMoniker),
        contextMenu: false,
      },
    ];
  }, [firstCardMoniker, setFocus]);

  return (
    <FocusScope moniker={HEADER_MONIKER} commands={commands}>
      <div data-testid="column-header">Todo</div>
    </FocusScope>
  );
}

function CardFixture({ moniker, label }: { moniker: string; label: string }) {
  return (
    <FocusScope moniker={moniker} commands={[]}>
      <div data-testid={`card-${moniker}`}>{label}</div>
    </FocusScope>
  );
}

describe("Column header Enter drills to first card (01KPX6FSPY6V15JATXMY6AGRER)", () => {
  let handles: TauriStubHandles;

  beforeEach(() => {
    handles = setupTauriStub();
  });

  it("pressing Enter on a focused column header moves focus to the first card", async () => {
    const screen = await render(
      <EntityFocusProvider>
        <FocusLayer name="window">
          <CommandScopeProvider commands={[]}>
            <KeybindingHandler mode="cua" />
            <ColumnHeaderDrillFixture firstCardMoniker={FIRST_CARD_MONIKER} />
            <CardFixture moniker={FIRST_CARD_MONIKER} label="first" />
            <CardFixture moniker={SECOND_CARD_MONIKER} label="second" />
          </CommandScopeProvider>
        </FocusLayer>
      </EntityFocusProvider>,
    );

    const header = screen.getByTestId("column-header");

    // Focus the header.
    await userEvent.click(header.element());
    await new Promise((r) => requestAnimationFrame(() => r(undefined)));

    // Drive Rust-side focused_key + emit focus-changed so subsequent
    // reads see the header as focused. The stub tracks focused_key
    // via spatial_focus, but `setFocus` in entity-focus-context also
    // calls setFocusedMoniker synchronously — the click path is
    // sufficient for extractScopeBindings to see the header's
    // commands.

    // Press Enter — drill command should fire.
    await userEvent.keyboard("{Enter}");

    // Assert: the card's moniker is now the focused moniker (stored
    // in the stub via spatial_focus).
    await expect
      .poll(
        () =>
          handles
            .invocations()
            .some(
              (i) =>
                i.cmd === "spatial_focus" &&
                (i.args as { key?: string })?.key !== undefined,
            ),
        { timeout: 500 },
      )
      .toBe(true);

    // Specifically: after the Enter, there must be a spatial_focus
    // call whose key maps back to FIRST_CARD_MONIKER via the
    // spatial_register history.
    const registeredKeys = new Map<string, string>(); // spatial key → moniker
    for (const inv of handles.invocations()) {
      if (inv.cmd === "spatial_register") {
        const args = (inv.args as { args: { key: string; moniker: string } })
          .args;
        registeredKeys.set(args.key, args.moniker);
      }
    }
    const focusMonikers = handles
      .invocations()
      .filter((i) => i.cmd === "spatial_focus")
      .map((i) => {
        const k = (i.args as { key?: string })?.key;
        return k ? (registeredKeys.get(k) ?? null) : null;
      })
      .filter((m): m is string => m !== null);
    // The last spatial_focus must be the first card (drill target).
    expect(focusMonikers[focusMonikers.length - 1]).toBe(FIRST_CARD_MONIKER);
  });

  it("pressing Enter on a column header with no cards is a no-op (no crash, no setFocus)", async () => {
    const screen = await render(
      <EntityFocusProvider>
        <FocusLayer name="window">
          <CommandScopeProvider commands={[]}>
            <KeybindingHandler mode="cua" />
            {/* firstCardMoniker intentionally omitted — empty column */}
            <ColumnHeaderDrillFixture firstCardMoniker={undefined} />
          </CommandScopeProvider>
        </FocusLayer>
      </EntityFocusProvider>,
    );

    const header = screen.getByTestId("column-header");
    await userEvent.click(header.element());
    await new Promise((r) => requestAnimationFrame(() => r(undefined)));

    const focusBefore = handles
      .invocations()
      .filter((i) => i.cmd === "spatial_focus").length;

    await userEvent.keyboard("{Enter}");
    await new Promise((r) => setTimeout(r, 50));

    const focusAfter = handles
      .invocations()
      .filter((i) => i.cmd === "spatial_focus").length;

    // No new spatial_focus call — the drill command was never
    // registered because firstCardMoniker is undefined.
    expect(focusAfter).toBe(focusBefore);
  });
});
