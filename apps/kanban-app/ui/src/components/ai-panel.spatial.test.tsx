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
import { act } from "@testing-library/react";
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
import { FocusLayer } from "./focus-layer";
import { FocusScope } from "./focus-scope";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import {
  setupSpatialHarness,
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

  it("registers a focus scope for the composer, model selector, and scrollback parented at the panel zone", async () => {
    const { unmount } = await renderPanel();
    await flushSetup();

    const zone = findRegisterRecord("ui:ai-panel");
    expect(zone).toBeTruthy();
    const zoneFq = zone!.fq as FullyQualifiedMoniker;

    for (const segment of [
      "ui:ai-panel.model-selector",
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
  // Intra-panel spatial nav: ArrowDown from the model selector moves toward
  // a lower control inside the panel (scrollback / composer), never out of
  // the panel zone.
  // -------------------------------------------------------------------------

  it("ArrowDown from the model selector moves to a control lower in the panel", async () => {
    const { unmount } = await renderPanel();
    await flushSetup();

    const selector = findRegisterRecord("ui:ai-panel.model-selector");
    const composer = findRegisterRecord("ui:ai-panel.composer");
    const scrollback = findRegisterRecord("ui:ai-panel.scrollback");
    expect(selector && composer && scrollback).toBeTruthy();
    const selectorFq = selector!.fq as FullyQualifiedMoniker;

    // Run the kernel's beam-nav port: Down from the selector.
    const result = harness.registry.get(selectorFq);
    expect(result, "selector must be in the shadow registry").toBeTruthy();

    await act(async () => {
      await mockInvoke("spatial_navigate", {
        focusedFq: selectorFq,
        direction: "down",
      });
    });
    // The shadow navigator emits focus-changed on the resulting FQM; assert
    // it landed on a panel control below the selector — the composer or the
    // scrollback, both inside the ui:ai-panel zone.
    await flushSetup();

    const panelControlFqs = new Set([
      composer!.fq as FullyQualifiedMoniker,
      scrollback!.fq as FullyQualifiedMoniker,
    ]);
    expect(
      panelControlFqs.has(harness.currentFocus.fq as FullyQualifiedMoniker),
      `ArrowDown from the model selector must land on a control inside the panel \
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
