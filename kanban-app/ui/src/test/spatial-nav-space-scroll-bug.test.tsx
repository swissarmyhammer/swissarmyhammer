/**
 * Live-app reproducer for the "Space scrolls instead of inspecting" bug
 * reported by the user in task 01KPX6E0QPNRWZTQXGXX2MBEMV.
 *
 * ## Why this file exists
 *
 * Every previous Space-rebind test either:
 * - stubbed `trySingleKey` directly and never attached the real
 *   `createKeyHandler` to `document`, so it couldn't measure whether
 *   `preventDefault` actually beat the browser's default scroll action;
 * - or mocked `invoke` and asserted the call happened, without
 *   verifying that the scroll container's `scrollTop` didn't move
 *   (which is the symptom the user reports).
 *
 * The user pressed Space on a focused grid cell. The grid scrolled.
 * The inspector did not open. No unit test caught this.
 *
 * This file fixes that. It mounts a real scrollable container with a
 * scope (moniker'd like a grid cell) inside, attaches the real
 * `<AppShell>` `KeybindingHandler` (which wires
 * `document.addEventListener("keydown", ...)` per the production path),
 * dispatches a real Space keydown, and asserts BOTH:
 *
 *   1. `scrollTop` on the container is unchanged.
 *   2. `dispatch_command("ui.inspect", …)` fired through the real
 *      command-scope pipeline.
 *
 * If (1) fails, preventDefault didn't arrive in time (or at all) and
 * the fix lives in keybindings.ts / app-shell.tsx.
 * If (2) fails, the scope-chain resolution is broken and the fix
 * lives in command-scope.tsx / useInspectCommand.
 * If both fail, both layers are broken and we fix in order.
 *
 * ## Non-goals
 *
 * This file does NOT try to stand up the full board/grid fixture. It
 * mounts the minimum topology: EntityFocusProvider → FocusLayer →
 * CommandScopeProvider with global commands → KeybindingHandler → a
 * scrollable div → a FocusScope with an `entity.inspect.<m>` command.
 * That's enough to prove or disprove both failure modes.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { userEvent } from "vitest/browser";
import { render } from "vitest-browser-react";
import { useMemo } from "react";

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
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { FocusLayer } from "@/components/focus-layer";
import { FocusScope } from "@/components/focus-scope";
import {
  CommandScopeProvider,
  useDispatchCommand,
  type CommandDef,
} from "@/lib/command-scope";

// Minimal replica of `KeybindingHandler` from app-shell.tsx. We can't
// import `AppShell` directly because it pulls the full window/
// perspective/ui-state machinery — and this bug is specifically about
// the keybinding layer, which is isolatable.
//
// If this handler ever drifts from the real one in app-shell.tsx, the
// test stops representing production. Keep them in lockstep.
import { useEffect, useRef, useContext, useCallback } from "react";
import { CommandScopeContext } from "@/lib/command-scope";
import { useFocusedScope } from "@/lib/entity-focus-context";
import {
  createKeyHandler,
  extractScopeBindings,
  type KeymapMode,
} from "@/lib/keybindings";

/**
 * Inline replica of `KeybindingHandler` from `app-shell.tsx`.
 *
 * Attaches the real `createKeyHandler` to `document` via
 * `addEventListener("keydown")` — the exact production wiring. Any
 * divergence (capture phase, passive option, different target) would
 * hide the bug.
 */
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

/**
 * Mount the smallest topology that reproduces the bug:
 *
 *   <EntityFocusProvider>
 *     <FocusLayer name="window">
 *       <CommandScopeProvider commands={[]}>
 *         <KeybindingHandler mode="cua" />
 *         <div style="overflow:auto; height:200px; content is 800px tall">
 *           <FocusScope moniker="task:x" commands=[{id:entity.inspect.x, keys:{cua:"Space"}, execute:dispatch ui.inspect}]>
 *             <div>card</div>
 *           </FocusScope>
 *         </div>
 *       </CommandScopeProvider>
 *     </FocusLayer>
 *   </EntityFocusProvider>
 *
 * The scroll container has real overflow + tall content so Space's
 * default browser action IS to scroll. The FocusScope has a per-card
 * Space binding to `ui.inspect` (the exact pattern `entity-card.tsx`
 * uses via `useInspectCommand`).
 */

const CARD_MONIKER = "task:space-bug";

function ScrollableCardFixture() {
  const dispatchInspect = useDispatchCommand("ui.inspect");
  const commands = useMemo<CommandDef[]>(
    () => [
      {
        id: `entity.inspect.${CARD_MONIKER}`,
        name: "Inspect",
        keys: { vim: "Space", cua: "Space", emacs: "Space" },
        execute: () => {
          dispatchInspect({ target: CARD_MONIKER }).catch(console.error);
        },
        contextMenu: false,
      },
    ],
    [dispatchInspect],
  );

  return (
    <div
      data-testid="scroll-container"
      style={{
        overflow: "auto",
        height: "200px",
        width: "400px",
        border: "1px solid black",
      }}
    >
      <div style={{ height: "800px" }}>
        {/* Card sits near the top, inside tall content. */}
        <FocusScope moniker={CARD_MONIKER} commands={commands}>
          <div
            data-testid="card"
            style={{ height: "60px", padding: "8px", background: "#eee" }}
          >
            card
          </div>
        </FocusScope>
        <div style={{ height: "740px", paddingTop: "16px" }}>spacer</div>
      </div>
    </div>
  );
}

describe("Space on a focused card inside a scrollable container (01KPX6E0QPNRWZTQXGXX2MBEMV Bug A)", () => {
  let handles: TauriStubHandles;

  beforeEach(() => {
    handles = setupTauriStub();
  });

  it("pressing Space on the focused card dispatches ui.inspect and does NOT scroll the container", async () => {
    const screen = await render(
      <EntityFocusProvider>
        <FocusLayer name="window">
          <CommandScopeProvider commands={[]}>
            <KeybindingHandler mode="cua" />
            <ScrollableCardFixture />
          </CommandScopeProvider>
        </FocusLayer>
      </EntityFocusProvider>,
    );

    const card = screen.getByTestId("card");
    const container = screen.getByTestId("scroll-container");

    // Focus the card — clicking runs the FocusScope's onClick which
    // invokes setFocus(moniker). We don't poll data-focused here
    // because the optimistic path in setFocus() calls
    // setFocusedMoniker synchronously, and the subsequent
    // invoke("spatial_focus") round-trip's ack is orthogonal to the
    // scope chain that extractScopeBindings walks. What matters:
    // focusedScope is populated before the subsequent keydown.
    await userEvent.click(card.element());

    // Give React a paint frame so the focus store propagates to the
    // FocusedScopeContext consumer in KeybindingHandler.
    await new Promise((r) => requestAnimationFrame(() => r(undefined)));
    await new Promise((r) => requestAnimationFrame(() => r(undefined)));

    // Baseline scroll position — must be zero at mount and stay zero.
    const scrollEl = container.element() as HTMLElement;
    expect(scrollEl.scrollTop).toBe(0);
    const beforeDispatches = handles.dispatchedCommands().length;

    // Real browser keydown. userEvent.keyboard dispatches a genuine
    // KeyboardEvent with key=" ", so Space's default browser action
    // (page-scroll) is what we're racing against our preventDefault.
    await userEvent.keyboard(" ");

    // Three assertions — all must hold. Any one failing is a live bug:
    //   - scrollTop > 0 → preventDefault didn't beat the browser scroll
    //   - ui.inspect absent → scope-chain / command resolution broke
    //   - ui.inspect target undefined → runBackendDispatch isn't
    //     forwarding the resolved CommandDef's target, so the
    //     backend inspector call silently resolves against an empty
    //     target (the exact failure mode shipped in the previous
    //     Space-rebind commit)
    await expect
      .poll(
        () =>
          handles
            .dispatchedCommands()
            .slice(beforeDispatches)
            .some((d) => d.cmd === "ui.inspect"),
        { timeout: 500 },
      )
      .toBe(true);

    const inspectCalls = handles
      .dispatchedCommands()
      .slice(beforeDispatches)
      .filter((d) => d.cmd === "ui.inspect");
    expect(inspectCalls[0]?.target).toBe(CARD_MONIKER);

    expect(scrollEl.scrollTop).toBe(0);
  });

  // Second reproducer — targets the target-forwarding bug found during
  // the live-app diagnosis.
  //
  // The card from `ScrollableCardFixture` above has a per-card
  // `entity.inspect.<moniker>` CommandDef WITH an `execute` handler
  // that dispatches `ui.inspect` via its own call. That path works.
  //
  // Production uses `useEntityCommands` which registers the bare
  // `ui.inspect` CommandDef from YAML, stamps `target: entityMoniker`
  // on it, and has NO `execute`. When Space resolves to that bare
  // CommandDef, dispatch falls through to `runBackendDispatch`. Before
  // the fix, `runBackendDispatch` read `target` only from `opts.target`
  // (which is `undefined` when the keybinding handler calls
  // `executeCommand(id)` without options). Result: `dispatch_command`
  // fires with `cmd: "ui.inspect"` and `target: undefined`, backend
  // has no entity to inspect, inspector never opens — the exact live
  // symptom.
  //
  // This test mounts a card with a YAML-shaped `ui.inspect` CommandDef
  // (target set, keys set, no execute) and asserts the dispatch
  // carries the target. Before the fix: target is `undefined`. After:
  // target is the card's moniker.
  it("a target-carrying CommandDef with no execute handler still carries its target through keybinding dispatch", async () => {
    const TARGET_MONIKER = "task:target-fwd";

    function YamlShapedInspectFixture() {
      const commands = useMemo<CommandDef[]>(
        () => [
          {
            id: "ui.inspect",
            name: "Inspect",
            target: TARGET_MONIKER,
            keys: { vim: "Space", cua: "Space", emacs: "Space" },
            // NO execute — this is the shape useEntityCommands produces
            // from the schema-loaded YAML. Dispatch MUST fall through
            // to runBackendDispatch and forward resolved.target.
          },
        ],
        [],
      );

      return (
        <FocusScope moniker={TARGET_MONIKER} commands={commands}>
          <div data-testid="card-yaml-shape">card</div>
        </FocusScope>
      );
    }

    const screen = await render(
      <EntityFocusProvider>
        <FocusLayer name="window">
          <CommandScopeProvider commands={[]}>
            <KeybindingHandler mode="cua" />
            <YamlShapedInspectFixture />
          </CommandScopeProvider>
        </FocusLayer>
      </EntityFocusProvider>,
    );

    const card = screen.getByTestId("card-yaml-shape");
    await userEvent.click(card.element());
    await new Promise((r) => requestAnimationFrame(() => r(undefined)));
    await new Promise((r) => requestAnimationFrame(() => r(undefined)));

    const beforeDispatches = handles.dispatchedCommands().length;
    await userEvent.keyboard(" ");

    await expect
      .poll(
        () =>
          handles
            .dispatchedCommands()
            .slice(beforeDispatches)
            .some((d) => d.cmd === "ui.inspect"),
        { timeout: 500 },
      )
      .toBe(true);

    const inspectCall = handles
      .dispatchedCommands()
      .slice(beforeDispatches)
      .find((d) => d.cmd === "ui.inspect");
    expect(inspectCall?.target).toBe(TARGET_MONIKER);
  });
});
