/**
 * Spatial-nav integration test for the AI panel's focus scopes.
 *
 * Source of truth for kanban task `01KRRN6N593QQAA4RXZ2RBC1PF` — "AI panel
 * focus scopes: jump-to and spatial navigation for the panel's controls".
 *
 * # What this pins
 *
 * The `AiPanel` View wires every interactive element into the app's
 * spatial-nav graph by reusing the shared `FocusScope` / `Pressable`
 * primitives:
 *
 *   - the panel body is a `<FocusScope moniker="ui:ai-panel">` zone — a
 *     CHILD of the window-root `<FocusLayer name="window">`, so its FQM is
 *     the path `/window/ui:ai-panel`, NOT a flat leaf string;
 *   - the conversation scrollback, the composer, and the model selector are
 *     each their own `<FocusScope>` leaf parented at the `ui:ai-panel` zone;
 *   - per-message copy / retry buttons register their own leaves too.
 *
 * Because the panel zone is a child of the SAME window layer the board /
 * nav-bar / perspective-bar live in, cardinal navigation crosses cleanly
 * between the view area and the panel WITHOUT a cross-layer jump — the
 * kernel's layer-boundary guard only fires across distinct layer FQMs, and
 * `/window/ui:ai-panel` shares the `/window` layer with everything else.
 *
 * # Harness
 *
 * Runs through the shared spatial-nav harness
 * (`@/test/spatial-shadow-registry`): the global registry hook in
 * `src/test/setup.ts` mirrors every `<FocusScope>` mount into a
 * `spatial_register_scope` mock-invoke call, the harness captures those
 * into a JS shadow registry, and `spatial_navigate` is routed through the
 * in-test port of `BeamNavStrategy::next`. A regression in either the
 * React-side registration shape or the FQM-path composition surfaces here.
 *
 * Runs under `kanban-app/ui/vite.config.ts`'s browser project (real
 * Chromium via Playwright).
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { act, fireEvent } from "@testing-library/react";
import { renderInAct } from "@/test/act-render";

// ---------------------------------------------------------------------------
// Tauri API mocks — file-scoped, forwarding to spies owned by the shared
// spatial-nav harness module.
// ---------------------------------------------------------------------------

const { mockInvoke, mockListen } = await vi.hoisted(async () => {
  const helper = await import("@/test/spatial-shadow-registry");
  return {
    mockInvoke: helper.mockInvoke,
    mockListen: helper.mockListen,
  };
});

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...a: unknown[]) => mockInvoke(...(a as [string, unknown?])),
}));

vi.mock("@tauri-apps/api/event", () => ({
  emit: vi.fn(() => Promise.resolve()),
  listen: (...a: Parameters<typeof mockListen>) => mockListen(...a),
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
// Imports — after mocks
// ---------------------------------------------------------------------------

import type {
  ContentBlock,
  PromptResponse,
  SessionNotification,
} from "@agentclientprotocol/sdk";
import type {
  AcpSession,
  KanbanAcpClient,
  SessionUpdateHandler,
} from "@/ai/acp-client";
import type { ConversationConnect } from "@/ai/conversation";
import { AiPanel, type AiModel, type AiPanelConnectFactory } from "./ai-panel";
import { AppShell } from "./app-shell";
import { FocusLayer } from "./focus-layer";
import { FocusScope } from "./focus-scope";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { UIStateProvider } from "@/lib/ui-state-context";
import { AppModeProvider } from "@/lib/app-mode-context";
import { UndoProvider } from "@/lib/undo-context";
import {
  setupSpatialHarness,
  type DefaultInvokeImpl,
  type SpatialHarness,
} from "@/test/spatial-shadow-registry";
import {
  asSegment,
  composeFq,
  fqRoot,
  type FullyQualifiedMoniker,
} from "@/types/spatial";

// ---------------------------------------------------------------------------
// Mock ACP transport — a fake session that replays scripted updates.
// ---------------------------------------------------------------------------

/** The script a {@link FakeSession} replays when `prompt` is called. */
interface SessionScript {
  /** `session/update` notifications streamed before `prompt` resolves. */
  updates?: SessionNotification["update"][];
}

/** A controllable fake ACP session — streams scripted updates, no WebSocket. */
class FakeSession implements AcpSession {
  readonly sessionId = "fake-session";

  constructor(
    private readonly onUpdate: SessionUpdateHandler,
    private readonly script: SessionScript,
  ) {}

  async prompt(_prompt: ContentBlock[]): Promise<PromptResponse> {
    for (const update of this.script.updates ?? []) {
      await this.onUpdate({ sessionId: this.sessionId, update });
    }
    return { stopReason: "end_turn" };
  }

  async cancel(): Promise<void> {}
  async setMode(): Promise<void> {}
}

/** Build an {@link AiPanelConnectFactory} backed by {@link FakeSession}s. */
function mockConnect(script: SessionScript = {}): AiPanelConnectFactory {
  return () => {
    const connect: ConversationConnect = async (handlers) => {
      const client: KanbanAcpClient = {
        protocolVersion: 1,
        initializeResponse: { protocolVersion: 1, agentCapabilities: {} },
        async startSession(): Promise<AcpSession> {
          return new FakeSession(handlers.onSessionUpdate, script);
        },
      };
      return client;
    };
    return connect;
  };
}

/** The Claude Code model fixture used across the panel tests. */
const MODELS: AiModel[] = [
  {
    id: "claude-code",
    label: "Claude Code",
    kind: "claude-code",
    available: true,
    hint: null,
  },
];

// ---------------------------------------------------------------------------
// Layout substitute — the browser test bundle does not load Tailwind, so the
// panel's `flex flex-col` chrome collapses without these rules. The panel and
// its sibling view-area placeholder must lay out side by side for cardinal
// beam search to have horizontal geometry to score against.
// ---------------------------------------------------------------------------

const TEST_LAYOUT_CSS = `
  .flex { display: flex; }
  .flex-col { flex-direction: column; }
  .flex-1 { flex: 1 1 0%; min-width: 0; min-height: 0; }
  .min-h-0 { min-height: 0; }
  .min-w-0 { min-width: 0; }
  .h-full { height: 100%; }
  .relative { position: relative; }
  .border-t { border-top: 1px solid #ccc; }
  .border-b { border-bottom: 1px solid #ccc; }
  [data-slot='ai-panel'] { width: 420px; }
`;

/** Inject the layout substitute stylesheet exactly once per document. */
function ensureTestLayoutCss(): void {
  if (document.querySelector("style[data-test-ai-panel-spatial]")) return;
  const style = document.createElement("style");
  style.setAttribute("data-test-ai-panel-spatial", "");
  style.textContent = TEST_LAYOUT_CSS;
  document.head.appendChild(style);
}

/** Wait for register effects scheduled in `useEffect` to flush. */
async function flushSetup() {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 80));
  });
}

/**
 * Render `<AiPanel>` inside the production-shaped spatial-nav stack.
 *
 * A sibling `view-area` `<FocusScope>` leaf stands in for the board so the
 * cross-zone navigation case has a target on the other side of the panel.
 * The view-area leaf and the `ui:ai-panel` zone are both children of the
 * single `<FocusLayer name="window">` — exactly the production topology.
 */
async function renderPanel(script: SessionScript = {}) {
  ensureTestLayoutCss();
  return await renderInAct(
    <div
      style={{
        width: "1200px",
        height: "700px",
        display: "flex",
        flexDirection: "row",
      }}
    >
      <SpatialFocusProvider>
        <FocusLayer name={asSegment("window")}>
          <EntityFocusProvider>
            {/* Sibling stand-in for the view area — a peer of the panel
                zone under the same window layer. */}
            <FocusScope
              moniker={asSegment("ui:view-area")}
              showFocus={false}
              style={{ flex: "1 1 0%", height: "100%" }}
            >
              <div style={{ height: "100%" }}>view area</div>
            </FocusScope>
            <AiPanel
              boardDir="/tmp/board"
              models={MODELS}
              modelId="claude-code"
              onSelectModel={() => {}}
              onCollapse={() => {}}
              createConnect={mockConnect(script)}
            />
          </EntityFocusProvider>
        </FocusLayer>
      </SpatialFocusProvider>
    </div>,
  );
}

/** Pull the most recent `spatial_register_scope` record for a segment. */
function findRegisterRecord(segment: string): Record<string, unknown> | null {
  for (let i = mockInvoke.mock.calls.length - 1; i >= 0; i--) {
    const c = mockInvoke.mock.calls[i];
    if (c[0] === "spatial_register_scope") {
      const r = c[1] as Record<string, unknown>;
      if (r && r.segment === segment) return r;
    }
  }
  return null;
}

/**
 * Fallback for the non-spatial Tauri commands the `AppShell` provider
 * stack hits on mount — `get_ui_state` (drives the keymap mode the
 * `KeybindingHandler` resolves against) and `get_undo_state`. Every
 * spatial command is handled by the shadow navigator before this runs.
 */
const appShellInvokeImpl: DefaultInvokeImpl = (cmd) => {
  if (cmd === "get_ui_state") {
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
  if (cmd === "get_undo_state") return { can_undo: false, can_redo: false };
  return undefined;
};

/**
 * Render `<AiPanel>` inside the production spatial-nav stack wrapped by
 * `<AppShell>` so the global keybinding pipeline is live.
 *
 * The shadow harness alone routes `spatial_*` IPCs but never turns a
 * keystroke into a command dispatch — that is `<AppShell>`'s
 * `<KeybindingHandler>`, which attaches a `keydown` listener on
 * `document` and resolves the focused scope's `commands` via
 * `extractScopeBindings`. Mounting it here lets the test drive a real
 * Enter keystroke and observe the composer scope's `drillIn` command
 * run — the path the user reported as broken.
 */
async function renderPanelWithShell(script: SessionScript = {}) {
  ensureTestLayoutCss();
  return await renderInAct(
    <div
      style={{
        width: "1200px",
        height: "700px",
        display: "flex",
        flexDirection: "row",
      }}
    >
      <SpatialFocusProvider>
        <FocusLayer name={asSegment("window")}>
          <EntityFocusProvider>
            <UIStateProvider>
              <AppModeProvider>
                <UndoProvider>
                  <AppShell>
                    <AiPanel
                      boardDir="/tmp/board"
                      models={MODELS}
                      modelId="claude-code"
                      onSelectModel={() => {}}
                      onCollapse={() => {}}
                      createConnect={mockConnect(script)}
                    />
                  </AppShell>
                </UndoProvider>
              </AppModeProvider>
            </UIStateProvider>
          </EntityFocusProvider>
        </FocusLayer>
      </SpatialFocusProvider>
    </div>,
  );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("AiPanel — spatial-nav focus scopes", () => {
  let harness: SpatialHarness;

  beforeEach(() => {
    harness = setupSpatialHarness();
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  // -------------------------------------------------------------------------
  // The panel is a zone with a path-correct moniker under the window layer.
  // -------------------------------------------------------------------------

  it("registers the panel as a ui:ai-panel zone whose FQM is the path /window/ui:ai-panel", async () => {
    const { unmount } = await renderPanel();
    await flushSetup();

    const zone = findRegisterRecord("ui:ai-panel");
    expect(
      zone,
      "the panel body must register a ui:ai-panel FocusScope zone",
    ).toBeTruthy();

    // The zone's FQM MUST be the PATH through the window layer, not a flat
    // leaf string — a flat moniker causes duplicate-registration ambiguity.
    const windowLayerFq = fqRoot(asSegment("window"));
    const expectedZoneFq = composeFq(windowLayerFq, asSegment("ui:ai-panel"));
    expect(
      zone!.fq,
      "the panel zone FQM must be /window/ui:ai-panel — the path through the window layer",
    ).toBe(expectedZoneFq);

    // The zone is parented directly under the window layer root.
    expect(
      zone!.parentZone,
      "the panel zone is a peer top-level scope under the window layer (parentZone null)",
    ).toBeNull();
    expect(
      zone!.layerFq,
      "the panel zone shares the window layer — NOT its own layer",
    ).toBe(windowLayerFq);

    unmount();
  });

  // -------------------------------------------------------------------------
  // Each interactive element registers its own focus scope, parented at the
  // panel zone — so each is path-addressable and a first-class beam target.
  // -------------------------------------------------------------------------

  it("registers a focus scope for the composer and scrollback parented at the panel zone", async () => {
    const { unmount } = await renderPanel();
    await flushSetup();

    const zone = findRegisterRecord("ui:ai-panel");
    expect(zone).toBeTruthy();
    const zoneFq = zone!.fq as FullyQualifiedMoniker;

    // The scrollback and the composer are each a `<FocusScope>` leaf parented
    // directly at the panel zone.
    for (const segment of [
      "ui:ai-panel.scrollback",
      "ui:ai-panel.composer",
    ] as const) {
      const leaf = findRegisterRecord(segment);
      expect(leaf, `${segment} must register a FocusScope leaf`).toBeTruthy();
      // Each leaf's FQM is the path /window/ui:ai-panel/<segment>.
      expect(
        leaf!.fq,
        `${segment} FQM must be composed under the panel zone`,
      ).toBe(composeFq(zoneFq, asSegment(segment)));
      expect(
        leaf!.parentZone,
        `${segment} must be parented at the ui:ai-panel zone`,
      ).toBe(zoneFq);
      expect(leaf!.layerFq, `${segment} must live in the window layer`).toBe(
        zone!.layerFq,
      );
    }

    unmount();
  });

  // -------------------------------------------------------------------------
  // The composer prompt (CM6) and the footer model selector are two
  // INDEPENDENT controls — each its own focus leaf, SIBLINGS under the
  // `ui:ai-panel` zone. Neither is nested inside the other: the surrounding
  // bordered composer container carries no focus scope, so the
  // `ui:ai-panel.composer` (CM6) and `ui:ai-panel.model-selector` leaves both
  // compose their FQM directly under `/window/ui:ai-panel`.
  // -------------------------------------------------------------------------

  it("registers the model selector as a sibling of the composer leaf under the panel zone", async () => {
    const { unmount } = await renderPanel();
    await flushSetup();

    const zone = findRegisterRecord("ui:ai-panel");
    const composer = findRegisterRecord("ui:ai-panel.composer");
    expect(zone && composer).toBeTruthy();
    const zoneFq = zone!.fq as FullyQualifiedMoniker;

    const selector = findRegisterRecord("ui:ai-panel.model-selector");
    expect(
      selector,
      "the model selector must still register a FocusScope leaf",
    ).toBeTruthy();
    // The selector is its own leaf composed directly under the panel zone —
    // a SIBLING of the composer CM6 leaf, not nested under it.
    expect(
      selector!.fq,
      "the model selector FQM must be composed directly under the panel zone",
    ).toBe(composeFq(zoneFq, asSegment("ui:ai-panel.model-selector")));
    expect(
      selector!.parentZone,
      "the model selector must be parented at the ui:ai-panel zone, not the composer",
    ).toBe(zoneFq);
    // The composer CM6 leaf is likewise parented at the panel zone — the two
    // are peers, neither one a path-descendant of the other.
    expect(
      composer!.parentZone,
      "the composer CM6 leaf must be parented at the ui:ai-panel zone",
    ).toBe(zoneFq);
    expect(
      String(selector!.fq).startsWith(`${String(composer!.fq)}/`),
      "the model selector must NOT be nested inside the composer scope",
    ).toBe(false);
    expect(
      selector!.layerFq,
      "the model selector must live in the window layer",
    ).toBe(zone!.layerFq);

    unmount();
  });

  // -------------------------------------------------------------------------
  // Drilling into the composer leaf lands DOM focus on the CM6 prompt — NOT
  // the model picker. `ui:ai-panel.composer` wraps only the CM6 editor body,
  // so focusing it puts the caret in the prompt, exactly like the filter
  // formula bar's scope; the model picker is a separate leaf and is not the
  // current spatial-nav target.
  // -------------------------------------------------------------------------

  it("focusing the composer leaf lands DOM focus on the CM6 prompt, not the model picker", async () => {
    const { container, unmount } = await renderPanel();
    await flushSetup();

    const composer = findRegisterRecord("ui:ai-panel.composer");
    const selector = findRegisterRecord("ui:ai-panel.model-selector");
    expect(composer && selector).toBeTruthy();
    const composerFq = composer!.fq as FullyQualifiedMoniker;

    // Focus the composer leaf, then drive DOM focus into the CM6 prompt —
    // the same "land on the scope, drill into the CM6 editor" flow the
    // filter formula bar uses.
    await act(async () => {
      await mockInvoke("spatial_focus", { fq: composerFq });
    });
    await flushSetup();

    const composerNode = container.querySelector(
      "[data-segment='ui:ai-panel.composer']",
    ) as HTMLElement | null;
    expect(composerNode, "composer leaf must be in the DOM").not.toBeNull();

    // The CM6 prompt — the `role="textbox"` content DOM with the panel's
    // accessible label — lives INSIDE the composer leaf.
    const prompt = composerNode!.querySelector(
      "[role='textbox'][aria-label='Message the AI agent']",
    ) as HTMLElement | null;
    expect(
      prompt,
      "the CM6 prompt must be a descendant of the ui:ai-panel.composer leaf",
    ).not.toBeNull();
    await act(async () => {
      prompt!.focus();
    });
    expect(
      document.activeElement,
      "drilling into the composer must land DOM focus on the CM6 prompt",
    ).toBe(prompt);

    // The model picker is NOT inside the composer leaf — it is a sibling
    // leaf, so the picker's trigger is not the drilled-into element.
    const pickerInComposer = composerNode!.querySelector(
      "[data-segment='ui:ai-panel.model-selector']",
    );
    expect(
      pickerInComposer,
      "the model picker leaf must NOT be nested inside the composer leaf",
    ).toBeNull();

    unmount();
  });

  // -------------------------------------------------------------------------
  // Drill-in actually drives the cursor in: with the composer leaf the
  // spatial focus, pressing Enter must move DOM focus into the CM6 prompt.
  //
  // A bare `<FocusScope>` only *registers* the composer as a nav target —
  // landing on it and pressing Enter does NOT focus the editor. The fix
  // gives the composer scope a per-scope `ui.ai-panel.composer.drillIn`
  // `CommandDef` (keyed to Enter) whose `execute` calls the shared
  // `TextEditorHandle.focus()`. This test drives a real Enter keystroke
  // through `<AppShell>`'s `<KeybindingHandler>` and asserts the CM6
  // prompt — NOT the model picker — receives DOM focus, mirroring the
  // filter formula bar's `filter_editor.drillIn` contract.
  // -------------------------------------------------------------------------

  it("Enter on the focused composer leaf drives DOM focus into the CM6 prompt", async () => {
    harness = setupSpatialHarness({ defaultInvokeImpl: appShellInvokeImpl });
    const { container, unmount } = await renderPanelWithShell();
    await flushSetup();

    const composer = findRegisterRecord("ui:ai-panel.composer");
    expect(composer, "the composer leaf must register").toBeTruthy();
    const composerFq = composer!.fq as FullyQualifiedMoniker;

    // The CM6 prompt is the `role="textbox"` content DOM with the panel's
    // accessible label, a descendant of the composer leaf.
    const composerNode = container.querySelector(
      "[data-segment='ui:ai-panel.composer']",
    ) as HTMLElement | null;
    expect(composerNode, "composer leaf must be in the DOM").not.toBeNull();
    const prompt = composerNode!.querySelector(
      "[role='textbox'][aria-label='Message the AI agent']",
    ) as HTMLElement | null;
    expect(prompt, "the CM6 prompt must be inside the composer leaf").not.toBeNull();

    // Move the cursor OFF the CM6 prompt first so the drill-in has a
    // visible effect to assert — focus the document body.
    await act(async () => {
      (document.body as HTMLElement).focus();
    });
    expect(
      document.activeElement,
      "precondition: DOM focus must not already be on the CM6 prompt",
    ).not.toBe(prompt);

    // Seed the spatial focus onto the composer leaf. The shadow
    // navigator echoes a `focus-changed` event whose `next_segment` the
    // entity-focus bridge mirrors into the store — that is the chain
    // `extractScopeBindings` walks on the next keydown.
    await act(async () => {
      await mockInvoke("spatial_focus", { fq: composerFq });
    });
    await flushSetup();

    // Press Enter. `<KeybindingHandler>` resolves it against the focused
    // composer scope's `commands` — the `ui.ai-panel.composer.drillIn`
    // `CommandDef` shadows the global `nav.drillIn` and calls
    // `editorRef.current?.focus()`.
    await act(async () => {
      fireEvent.keyDown(document, { key: "Enter", code: "Enter" });
      await Promise.resolve();
    });
    await flushSetup();

    expect(
      document.activeElement,
      "Enter on the focused composer leaf must land DOM focus on the CM6 prompt",
    ).toBe(prompt);

    unmount();
  });

  // -------------------------------------------------------------------------
  // The surrounding bordered composer container is NOT a focus scope. Only
  // the CM6 editor body is `ui:ai-panel.composer`; the `ai-prompt-composer`
  // bordered shell that also holds the footer toolbar carries no scope, so
  // it has no `data-segment` focus-scope marker.
  // -------------------------------------------------------------------------

  it("the ai-prompt-composer bordered container is not a focus scope", async () => {
    const { container, unmount } = await renderPanel();
    await flushSetup();

    const composerShell = container.querySelector(
      "[data-slot='ai-prompt-composer']",
    ) as HTMLElement | null;
    expect(
      composerShell,
      "the bordered composer container must be present",
    ).not.toBeNull();
    // A `<FocusScope>` always stamps `data-segment` (and `data-moniker`) on
    // its wrapper div. The bordered shell carrying either marker would mean
    // it is a focus scope — it must not be one.
    expect(
      composerShell!.hasAttribute("data-segment"),
      "the bordered composer container must not carry a focus-scope data-segment marker",
    ).toBe(false);
    expect(
      composerShell!.hasAttribute("data-moniker"),
      "the bordered composer container must not carry a focus-scope data-moniker marker",
    ).toBe(false);

    // The CM6 editor body — a descendant of the shell — IS the
    // `ui:ai-panel.composer` focus scope.
    const composerScope = composerShell!.querySelector(
      "[data-segment='ui:ai-panel.composer']",
    );
    expect(
      composerScope,
      "the CM6 editor body inside the shell must be the ui:ai-panel.composer scope",
    ).not.toBeNull();

    unmount();
  });

  // -------------------------------------------------------------------------
  // Jump-to: jumping into the panel lands directly on the composer.
  //
  // The jump-to overlay enumerates every focusable scope and dispatches
  // `spatial_focus(fq)` for the chosen one. This case proves the composer
  // leaf is a jump target: dispatching `spatial_focus` against its FQM
  // moves focus there and the React tree flips `data-focused`.
  // -------------------------------------------------------------------------

  it("jump-to lands directly on the composer leaf", async () => {
    const { container, unmount } = await renderPanel();
    await flushSetup();

    const composer = findRegisterRecord("ui:ai-panel.composer");
    expect(composer).toBeTruthy();
    const composerFq = composer!.fq as FullyQualifiedMoniker;

    // The jump-to overlay's terminal action is `spatial_focus(targetFq)`.
    await act(async () => {
      await mockInvoke("spatial_focus", { fq: composerFq });
    });
    await flushSetup();

    const composerNode = container.querySelector(
      "[data-segment='ui:ai-panel.composer']",
    ) as HTMLElement | null;
    expect(composerNode, "composer leaf must be in the DOM").not.toBeNull();
    expect(
      composerNode!.getAttribute("data-focused"),
      "jumping to the composer FQM must flip data-focused on the composer leaf",
    ).not.toBeNull();

    unmount();
  });

  // -------------------------------------------------------------------------
  // Jump-to: jumping to the model selector lands on the selector leaf.
  // -------------------------------------------------------------------------

  it("jump-to lands directly on the model selector leaf", async () => {
    const { container, unmount } = await renderPanel();
    await flushSetup();

    const selector = findRegisterRecord("ui:ai-panel.model-selector");
    expect(selector).toBeTruthy();
    const selectorFq = selector!.fq as FullyQualifiedMoniker;

    await act(async () => {
      await mockInvoke("spatial_focus", { fq: selectorFq });
    });
    await flushSetup();

    const selectorNode = container.querySelector(
      "[data-segment='ui:ai-panel.model-selector']",
    ) as HTMLElement | null;
    expect(selectorNode).not.toBeNull();
    expect(
      selectorNode!.getAttribute("data-focused"),
      "jumping to the model-selector FQM must flip data-focused on its leaf",
    ).not.toBeNull();

    unmount();
  });

  // -------------------------------------------------------------------------
  // Activating the model-picker leaf hands the keyboard to the Radix Select.
  //
  // The `ui:ai-panel.model-selector` leaf is a `<Pressable>`: pressing Enter
  // on the focused leaf runs the `pressable.activate` CommandDef, which calls
  // `onPress`. A bare `onPress` no-op leaves DOM focus on `document.body`, so
  // a Radix `Select` — which needs DOM focus on its trigger `<button>` for
  // Space/Enter/↑↓ to open and navigate the listbox — is unreachable by
  // keyboard. The fix gives `ComposerModelSelect` a ref to the
  // `PromptInputSelectTrigger` and has `onPress` call `triggerRef.focus()`.
  // This test seeds spatial focus on the model-selector leaf, fires a real
  // Enter keystroke through `<AppShell>`'s `<KeybindingHandler>`, and asserts
  // DOM focus landed on the select trigger.
  // -------------------------------------------------------------------------

  it("Enter on the focused model-selector leaf lands DOM focus on the select trigger", async () => {
    harness = setupSpatialHarness({ defaultInvokeImpl: appShellInvokeImpl });
    const { container, unmount } = await renderPanelWithShell();
    await flushSetup();

    const selector = findRegisterRecord("ui:ai-panel.model-selector");
    expect(selector, "the model-selector leaf must register").toBeTruthy();
    const selectorFq = selector!.fq as FullyQualifiedMoniker;

    // The model-select trigger is the `role="combobox"` button inside the
    // model-selector leaf — the host element of the `<Pressable asChild>`.
    const selectorNode = container.querySelector(
      "[data-segment='ui:ai-panel.model-selector']",
    ) as HTMLElement | null;
    expect(selectorNode, "model-selector leaf must be in the DOM").not.toBeNull();
    const trigger = selectorNode!.querySelector(
      "button[role='combobox']",
    ) as HTMLElement | null;
    expect(
      trigger,
      "the Radix select trigger button must be inside the model-selector leaf",
    ).not.toBeNull();

    // Move DOM focus OFF the trigger first so the activation has a visible
    // effect to assert.
    await act(async () => {
      (document.body as HTMLElement).focus();
    });
    expect(
      document.activeElement,
      "precondition: DOM focus must not already be on the select trigger",
    ).not.toBe(trigger);

    // Seed spatial focus onto the model-selector leaf.
    await act(async () => {
      await mockInvoke("spatial_focus", { fq: selectorFq });
    });
    await flushSetup();

    // Press Enter — `<KeybindingHandler>` resolves it against the focused
    // model-selector scope's `pressable.activate` CommandDef, which calls
    // `onPress` → `triggerRef.current?.focus()`.
    await act(async () => {
      fireEvent.keyDown(document, { key: "Enter", code: "Enter" });
      await Promise.resolve();
    });
    await flushSetup();

    expect(
      document.activeElement,
      "Enter on the focused model-selector leaf must land DOM focus on the select trigger",
    ).toBe(trigger);

    unmount();
  });

  // -------------------------------------------------------------------------
  // Escape inside the CM6 prompt drills out of the composer.
  //
  // Once the CM6 prompt has DOM focus, Escape must release it: blur the
  // contenteditable and return kernel spatial focus to the
  // `ui:ai-panel.composer` scope so the user can press `s` to re-open the
  // jump overlay. Without this the focus is trapped in the editor. The
  // composer routes Escape through the SHARED `buildSubmitCancelExtensions`
  // helper (`@/lib/cm-submit-cancel.ts`) — the same mechanism the filter
  // formula bar, the markdown field, the command palette, and the field
  // editors all use. Its `onCancelRef` callback is composed inside the
  // composer scope by `ComposerEditorDrillOutWiring` and blurs the active
  // element + dispatches `nav.focus` against the composer scope FQM.
  // -------------------------------------------------------------------------

  it("Escape inside the CM6 prompt blurs the editor and returns kernel focus to the composer scope", async () => {
    harness = setupSpatialHarness({ defaultInvokeImpl: appShellInvokeImpl });
    const { container, unmount } = await renderPanelWithShell();
    await flushSetup();

    const composer = findRegisterRecord("ui:ai-panel.composer");
    expect(composer, "the composer leaf must register").toBeTruthy();
    const composerFq = composer!.fq as FullyQualifiedMoniker;

    const composerNode = container.querySelector(
      "[data-segment='ui:ai-panel.composer']",
    ) as HTMLElement | null;
    expect(composerNode, "composer leaf must be in the DOM").not.toBeNull();
    const prompt = composerNode!.querySelector(
      "[role='textbox'][aria-label='Message the AI agent']",
    ) as HTMLElement | null;
    expect(prompt, "the CM6 prompt must be inside the composer leaf").not.toBeNull();

    // Drill DOM focus into the CM6 prompt — the trapped state Escape escapes.
    await act(async () => {
      prompt!.focus();
    });
    expect(
      document.activeElement,
      "precondition: DOM focus must be on the CM6 prompt",
    ).toBe(prompt);

    // Press Escape inside the CM6 editor. `buildSubmitCancelExtensions`'s
    // CUA Escape binding fires the composer's drill-out callback, which
    // blurs the contenteditable and dispatches `nav.focus` against the
    // composer scope FQM.
    await act(async () => {
      fireEvent.keyDown(prompt!, { key: "Escape", code: "Escape" });
      await Promise.resolve();
    });
    await flushSetup();

    // The editor lost DOM focus — the caret is no longer trapped.
    expect(
      document.activeElement,
      "Escape inside the CM6 prompt must blur the editor",
    ).not.toBe(prompt);

    // Kernel spatial focus returned to the `ui:ai-panel.composer` scope.
    expect(
      harness.currentFocus.fq,
      "Escape must return kernel spatial focus to the ui:ai-panel.composer scope",
    ).toBe(composerFq);

    unmount();
  });

  // -------------------------------------------------------------------------
  // Intra-panel spatial nav: the model selector now lives in the composer
  // footer (the AI Elements `PromptInput` layout), so it sits LOW in the
  // panel. ArrowUp from it moves toward a control higher in the panel (the
  // composer body or the scrollback), never out of the panel zone.
  // -------------------------------------------------------------------------

  it("ArrowUp from the model selector moves to a control higher in the panel", async () => {
    const { unmount } = await renderPanel();
    await flushSetup();

    const selector = findRegisterRecord("ui:ai-panel.model-selector");
    const composer = findRegisterRecord("ui:ai-panel.composer");
    const scrollback = findRegisterRecord("ui:ai-panel.scrollback");
    expect(selector && composer && scrollback).toBeTruthy();
    const selectorFq = selector!.fq as FullyQualifiedMoniker;

    // Run the kernel's beam-nav port: Up from the selector.
    const result = harness.registry.get(selectorFq);
    expect(result, "selector must be in the shadow registry").toBeTruthy();

    await act(async () => {
      await mockInvoke("spatial_navigate", {
        focusedFq: selectorFq,
        direction: "up",
      });
    });
    // The shadow navigator emits focus-changed on the resulting FQM; assert
    // it landed on a panel control above the footer selector — the composer
    // body or the scrollback, both inside the ui:ai-panel zone.
    await flushSetup();

    const panelControlFqs = new Set([
      composer!.fq as FullyQualifiedMoniker,
      scrollback!.fq as FullyQualifiedMoniker,
    ]);
    expect(
      panelControlFqs.has(harness.currentFocus.fq as FullyQualifiedMoniker),
      `ArrowUp from the model selector must land on a control inside the panel \
       (composer or scrollback); landed on ${String(harness.currentFocus.fq)}`,
    ).toBe(true);

    unmount();
  });

  // -------------------------------------------------------------------------
  // Cross-zone nav WITHOUT a cross-layer jump: ArrowLeft from a panel control
  // crosses into the view area. Because both the panel zone and the view
  // area share the /window layer, the kernel's layer-boundary guard never
  // fires — the move is a normal in-layer beam search.
  // -------------------------------------------------------------------------

  it("ArrowLeft from the composer crosses into the view area within the same window layer", async () => {
    const { unmount } = await renderPanel();
    await flushSetup();

    const composer = findRegisterRecord("ui:ai-panel.composer");
    const viewArea = findRegisterRecord("ui:view-area");
    expect(composer && viewArea).toBeTruthy();

    const composerFq = composer!.fq as FullyQualifiedMoniker;
    const viewAreaFq = viewArea!.fq as FullyQualifiedMoniker;

    // Both must be in the SAME layer — that is what makes the crossing a
    // plain in-layer beam search, not a blocked cross-layer jump.
    expect(
      composer!.layerFq,
      "the panel control and the view area must share one layer FQM",
    ).toBe(viewArea!.layerFq);

    await act(async () => {
      await mockInvoke("spatial_navigate", {
        focusedFq: composerFq,
        direction: "left",
      });
    });
    await flushSetup();

    // Beam search resolved to the view area — focus crossed cleanly.
    expect(
      harness.currentFocus.fq,
      "ArrowLeft from the composer must cross into the view area (same layer, no cross-layer jump)",
    ).toBe(viewAreaFq);

    unmount();
  });

  // -------------------------------------------------------------------------
  // And back: ArrowRight from the view area re-enters the panel zone.
  // -------------------------------------------------------------------------

  it("ArrowRight from the view area re-enters the panel", async () => {
    const { unmount } = await renderPanel();
    await flushSetup();

    const viewArea = findRegisterRecord("ui:view-area");
    const zone = findRegisterRecord("ui:ai-panel");
    expect(viewArea && zone).toBeTruthy();

    const viewAreaFq = viewArea!.fq as FullyQualifiedMoniker;
    const zoneFqStr = String(zone!.fq);

    await act(async () => {
      await mockInvoke("spatial_navigate", {
        focusedFq: viewAreaFq,
        direction: "right",
      });
    });
    await flushSetup();

    // The resolved FQM is the panel zone itself or a path-descendant of it
    // — either way, focus re-entered the panel.
    const landedFq = String(harness.currentFocus.fq);
    expect(
      landedFq === zoneFqStr || landedFq.startsWith(`${zoneFqStr}/`),
      `ArrowRight from the view area must re-enter the panel; landed on ${landedFq}`,
    ).toBe(true);

    unmount();
  });

  // -------------------------------------------------------------------------
  // Per-message action buttons: a copy button on a rendered assistant
  // message registers its own focus scope and is reachable by jump-to.
  // -------------------------------------------------------------------------

  it("a per-message copy action registers a focus scope and is jump-addressable", async () => {
    const { container, unmount } = await renderPanel({
      updates: [
        {
          sessionUpdate: "agent_message_chunk",
          content: { type: "text", text: "Here is the answer." },
        },
      ],
    });
    await flushSetup();

    // Drive a turn so an assistant message renders with its action buttons.
    // The composer is a CM6 editor — type into its `role="textbox"` content
    // DOM, not a plain `<textarea>`.
    const composer = container.querySelector(
      "[role='textbox'][aria-label='Message the AI agent']",
    ) as HTMLElement | null;
    expect(composer, "composer CM6 content DOM must be present").not.toBeNull();
    const { userEvent } = await import("vitest/browser");
    await act(async () => {
      await userEvent.type(composer!, "what is the answer?");
    });
    await act(async () => {
      const submit = container.querySelector(
        "button[aria-label='Submit']",
      ) as HTMLButtonElement;
      await userEvent.click(submit);
    });
    await flushSetup();

    // The copy action on the (single) message registers a leaf scope. Its
    // moniker is per-message — assert at least one message-action copy leaf
    // registered. The message lives inside the conversation scrollback, so
    // the leaf is parented at the `ui:ai-panel.scrollback` zone, and its
    // FQM is a path-descendant of the `ui:ai-panel` panel zone.
    const zone = findRegisterRecord("ui:ai-panel");
    const scrollback = findRegisterRecord("ui:ai-panel.scrollback");
    expect(zone && scrollback).toBeTruthy();
    const copyRecord = mockInvoke.mock.calls
      .filter((c) => c[0] === "spatial_register_scope")
      .map((c) => c[1] as Record<string, unknown>)
      .find(
        (r) =>
          typeof r.segment === "string" &&
          r.segment.startsWith("ui:ai-panel.message-action:") &&
          r.segment.endsWith(":copy"),
      );
    expect(
      copyRecord,
      "a per-message copy action must register a ui:ai-panel.message-action:{id}:copy leaf",
    ).toBeTruthy();
    expect(
      copyRecord!.parentZone,
      "the message-action copy leaf must be parented at the scrollback zone (the message lives inside it)",
    ).toBe(scrollback!.fq);
    // The leaf's FQM is still a path-descendant of the panel zone — it
    // belongs to the panel's spatial subtree.
    expect(
      String(copyRecord!.fq).startsWith(`${String(zone!.fq)}/`),
      "the message-action copy leaf FQM must be a path-descendant of the ui:ai-panel zone",
    ).toBe(true);

    // Jump-to lands on it.
    await act(async () => {
      await mockInvoke("spatial_focus", {
        fq: copyRecord!.fq as FullyQualifiedMoniker,
      });
    });
    await flushSetup();

    const copyNode = container.querySelector(
      `[data-segment='${String(copyRecord!.segment)}']`,
    ) as HTMLElement | null;
    expect(copyNode, "the copy action leaf must be in the DOM").not.toBeNull();
    expect(
      copyNode!.getAttribute("data-focused"),
      "jumping to the copy action FQM must flip data-focused on its leaf",
    ).not.toBeNull();

    unmount();
  });
});
