/**
 * Spatial-nav integration test for the AI panel's elicitation form.
 *
 * Source of truth for kanban task `01KS8JGYZN1A3WCE9RBEVW7N8Y` — "Integrate
 * the AI panel's elicitation form into the focus-scope / spatial-navigation /
 * jump-to system".
 *
 * # What this pins
 *
 * When the agent issues an elicitation request the panel renders an inline
 * form (a labeled control per field plus Submit / Decline / Cancel) or a
 * url prompt (a link plus Done / Cancel). Before this wiring those controls
 * were plain `<Button>`s and shadcn inputs — invisible to the spatial-nav
 * graph, so the user could neither jump to them nor activate them by
 * keyboard. After the wiring every control is a `<FocusScope>` /
 * `<Pressable>` leaf:
 *
 *   - each action button (Submit / Decline / Cancel / Done) registers a
 *     `ui:ai-panel.elicitation.action:{name}` leaf and activates its
 *     handler on Enter;
 *   - each field (text / number / select / boolean / multiselect option)
 *     registers a `ui:ai-panel.elicitation.field:{key}` leaf (multiselect
 *     options carry an `.option:{value}` suffix).
 *
 * The elicitation UI renders inside the conversation scrollback, which is
 * itself the `ui:ai-panel.scrollback` `<FocusScope>` leaf — so every
 * elicitation control's `parentZone` is the scrollback fq, and its fq is a
 * path-descendant of the `ui:ai-panel` panel zone (exactly the established
 * per-message-action contract pinned in `ai-panel.spatial.test.tsx`).
 *
 * # Harness
 *
 * Reuses the shared spatial-nav harness (`@/test/spatial-shadow-registry`):
 * the global registry hook mirrors every `<FocusScope>` mount into a
 * `spatial_register_scope` record, and Enter activation is driven through a
 * real `<AppShell>` `<KeybindingHandler>` keystroke against the focused leaf
 * (the `pressable.test.tsx` recipe). Runs under the browser project (real
 * Chromium via Playwright).
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { act, fireEvent, screen } from "@testing-library/react";
import { userEvent } from "vitest/browser";
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
  CreateElicitationRequest,
  CreateElicitationResponse,
  PromptResponse,
  SessionNotification,
} from "@agentclientprotocol/sdk";
import type {
  AcpSession,
  ElicitationHandler,
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
} from "@/test/spatial-shadow-registry";
import { asSegment, type FullyQualifiedMoniker } from "@/types/spatial";

// ---------------------------------------------------------------------------
// Mock ACP transport — a fake session plus a connect factory that captures
// the elicitation handler so the test can drive an elicitation request.
// ---------------------------------------------------------------------------

/** The script a {@link FakeSession} replays when `prompt` is called. */
interface SessionScript {
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

/** A connect factory plus the captured elicitation handler seam. */
interface ElicitationHarness {
  createConnect: AiPanelConnectFactory;
  elicitation: () => ElicitationHandler;
}

/** Build a connect factory backed by {@link FakeSession}s that captures the
 * `onElicitation` handler so the test can fire elicitation requests. */
function mockConnect(script: SessionScript = {}): ElicitationHarness {
  let captured: ElicitationHandler | undefined;
  const createConnect: AiPanelConnectFactory = () => {
    const connect: ConversationConnect = async (handlers) => {
      captured = handlers.onElicitation;
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
  return {
    createConnect,
    elicitation: () => {
      if (!captured) throw new Error("createConnect was never invoked");
      return captured;
    },
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
// Layout substitute — the browser test bundle does not load Tailwind.
// ---------------------------------------------------------------------------

const TEST_LAYOUT_CSS = `
  .flex { display: flex; }
  .flex-col { flex-direction: column; }
  .flex-wrap { flex-wrap: wrap; }
  .flex-1 { flex: 1 1 0%; min-width: 0; min-height: 0; }
  .min-h-0 { min-height: 0; }
  .min-w-0 { min-width: 0; }
  .h-full { height: 100%; }
  .relative { position: relative; }
  [data-slot='ai-panel'] { width: 420px; }
`;

/** Inject the layout substitute stylesheet exactly once per document. */
function ensureTestLayoutCss(): void {
  if (document.querySelector("style[data-test-ai-panel-elicit]")) return;
  const style = document.createElement("style");
  style.setAttribute("data-test-ai-panel-elicit", "");
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
 * Fallback for the non-spatial Tauri commands the `AppShell` provider stack
 * hits on mount — `get_ui_state` and `get_undo_state`.
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
 * `<AppShell>` so the global keybinding pipeline is live, then prime the
 * connection by sending a first prompt (which captures the elicitation
 * handler). Returns the render result plus the harness.
 */
async function renderPrimedPanel(harness: ElicitationHarness) {
  ensureTestLayoutCss();
  const result = await renderInAct(
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
            <FocusScope
              moniker={asSegment("ui:view-area")}
              showFocus={false}
              style={{ flex: "1 1 0%", height: "100%" }}
            >
              <div style={{ height: "100%" }}>view area</div>
            </FocusScope>
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
                      createConnect={harness.createConnect}
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

  // Prime the connection: type a prompt and submit so the panel connects
  // and the elicitation handler is captured. The composer is a CM6 editor —
  // type into its `role="textbox"` content DOM.
  const composer = result.container.querySelector(
    "[role='textbox'][aria-label='Message the AI agent']",
  ) as HTMLElement | null;
  if (!composer) throw new Error("composer CM6 content DOM must be present");
  await act(async () => {
    await userEvent.type(composer, "prime the connection");
  });
  await act(async () => {
    const submit = result.container.querySelector(
      "button[aria-label='Submit']",
    ) as HTMLButtonElement;
    await userEvent.click(submit);
  });
  await flushSetup();
  return result;
}

/** A form-mode request: a required text field, a select, a boolean, and a
 * multiselect — one of every wrapped control kind. */
function formRequest(): CreateElicitationRequest {
  return {
    sessionId: "fake-session",
    mode: "form",
    message: "Tell me about the deploy",
    requestedSchema: {
      type: "object",
      properties: {
        summary: { type: "string", title: "Summary" },
        severity: { type: "string", title: "Severity", enum: ["low", "high"] },
        urgent: { type: "boolean", title: "Urgent" },
        tags: {
          type: "array",
          title: "Tags",
          items: { type: "string", enum: ["a", "b"] },
        },
      },
      required: ["summary"],
    },
  };
}

/**
 * A form-mode request exercising EVERY wrapped control kind, so each kind's
 * spatial wiring (leaf registration, drill-in, drill-out) is pinned:
 *
 *   - `summary`  → text     (`<Input type="text">`)
 *   - `amount`   → number   (`<Input type="number">`)
 *   - `count`    → integer  (`<Input type="number" step=1>`)
 *   - `notes`    → textarea (`<Textarea>`)
 *   - `severity` → select   (Radix `<Select>` trigger)
 *   - `urgent`   → boolean  (Radix `<Checkbox>`)
 *   - `tags`     → multiselect (one `<Checkbox>` per option)
 *
 * The schema mirrors the kind inference in `@/ai/elicitation`: a plain
 * `string` becomes `text`, a `string` whose `maxLength` is at or above the
 * textarea threshold (120) becomes `textarea`, `integer`/`number` map
 * one-to-one, an `enum` string becomes a `select`, and an `array` becomes a
 * `multiselect`.
 */
function allKindsFormRequest(): CreateElicitationRequest {
  return {
    sessionId: "fake-session",
    mode: "form",
    message: "Tell me about the deploy",
    requestedSchema: {
      type: "object",
      properties: {
        summary: { type: "string", title: "Summary" },
        amount: { type: "number", title: "Amount" },
        count: { type: "integer", title: "Count" },
        // maxLength >= TEXTAREA_MAX_LENGTH_THRESHOLD (120) → textarea kind.
        notes: { type: "string", title: "Notes", maxLength: 500 },
        severity: { type: "string", title: "Severity", enum: ["low", "high"] },
        urgent: { type: "boolean", title: "Urgent" },
        tags: {
          type: "array",
          title: "Tags",
          items: { type: "string", enum: ["a", "b"] },
        },
      },
      required: ["summary"],
    },
  };
}

/** A url-mode request directing the user to an external page. */
function urlRequest(): CreateElicitationRequest {
  return {
    sessionId: "fake-session",
    mode: "url",
    message: "Authorize the integration",
    url: "https://example.com/authorize",
    elicitationId: "elicit-1",
  };
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

/** Count the `spatial_focus` calls recorded so far (drill-out assertions
 * slice from this baseline to ignore earlier focus seeds). */
function focusCallCount(): number {
  return mockInvoke.mock.calls.filter((c) => (c[0] === "spatial_focus" || (c[0] === "command_tool_call" && (c[1] as any)?.tool === "focus" && (c[1] as any)?.op === "set focus"))).length;
}

/**
 * Seed spatial focus on a field leaf and press a real Enter through the
 * keybinding pipeline — the "drill-in" gesture. For a text-like field this
 * runs the per-scope drill-in `CommandDef` that hands DOM focus to the input;
 * for a `Pressable` leaf (select / checkbox / option) it runs the Enter
 * activation `CommandDef` that fires `onPress`.
 *
 * @param fq - The field (or option) leaf's fully-qualified moniker.
 */
async function drillInto(fq: FullyQualifiedMoniker): Promise<void> {
  await act(async () => {
    await mockInvoke("spatial_focus", { fq });
  });
  await flushSetup();
  await act(async () => {
    fireEvent.keyDown(document, { key: "Enter", code: "Enter" });
    await Promise.resolve();
  });
  await flushSetup();
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("AiPanel — elicitation form spatial-nav focus scopes", () => {
  beforeEach(() => {
    // Install the shadow navigator (records `spatial_register_scope` calls and
    // routes `spatial_focus` / `spatial_navigate`). The tests read the captured
    // registrations via `findRegisterRecord` and drive focus via `mockInvoke`,
    // so the returned harness handle is not needed here.
    setupSpatialHarness({ defaultInvokeImpl: appShellInvokeImpl });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  // -------------------------------------------------------------------------
  // Form mode: each action button + each field registers a spatial leaf
  // whose fq is a path-descendant of the panel zone, parented at the
  // scrollback zone the elicitation UI lives inside.
  // -------------------------------------------------------------------------

  it("registers a spatial leaf for every form action and field, path-descendant of the panel zone", async () => {
    const harness = mockConnect();
    const { unmount } = await renderPrimedPanel(harness);

    await act(async () => {
      void harness.elicitation()(formRequest());
    });
    await flushSetup();

    const zone = findRegisterRecord("ui:ai-panel");
    const scrollback = findRegisterRecord("ui:ai-panel.scrollback");
    expect(zone && scrollback).toBeTruthy();
    const zoneFq = String(zone!.fq);
    const scrollbackFq = scrollback!.fq;

    const expectedSegments = [
      "ui:ai-panel.elicitation.action:submit",
      "ui:ai-panel.elicitation.action:decline",
      "ui:ai-panel.elicitation.action:cancel",
      "ui:ai-panel.elicitation.field:summary",
      "ui:ai-panel.elicitation.field:severity",
      "ui:ai-panel.elicitation.field:urgent",
      "ui:ai-panel.elicitation.field:tags.option:a",
      "ui:ai-panel.elicitation.field:tags.option:b",
    ] as const;

    for (const segment of expectedSegments) {
      const leaf = findRegisterRecord(segment);
      expect(leaf, `${segment} must register a spatial leaf`).toBeTruthy();
      // The leaf is parented at the scrollback zone (the elicitation UI
      // renders inside the conversation scrollback).
      expect(
        leaf!.parentZone,
        `${segment} must be parented at the scrollback zone`,
      ).toBe(scrollbackFq);
      // Its fq is a path-descendant of the panel zone.
      expect(
        String(leaf!.fq).startsWith(`${zoneFq}/`),
        `${segment} fq must be a path-descendant of the ui:ai-panel zone`,
      ).toBe(true);
      // It lives in the same window layer as the panel zone.
      expect(leaf!.layerFq, `${segment} must live in the window layer`).toBe(
        zone!.layerFq,
      );
    }

    unmount();
  });

  // -------------------------------------------------------------------------
  // Leaf registration for EVERY control kind — text / number / integer /
  // textarea / select / boolean / multiselect — plus the action buttons.
  // The base registration test above omits number / integer / textarea
  // (its `formRequest` has no such fields); this one drives the all-kinds
  // request so each control kind's `ui:ai-panel.elicitation.field:*` leaf is
  // pinned. Table-driven so a regression naming the failing kind is obvious.
  // -------------------------------------------------------------------------

  it("registers a spatial leaf for every control kind", async () => {
    const harness = mockConnect();
    const { unmount } = await renderPrimedPanel(harness);

    await act(async () => {
      void harness.elicitation()(allKindsFormRequest());
    });
    await flushSetup();

    const zone = findRegisterRecord("ui:ai-panel");
    const scrollback = findRegisterRecord("ui:ai-panel.scrollback");
    expect(zone && scrollback).toBeTruthy();
    const zoneFq = String(zone!.fq);
    const scrollbackFq = scrollback!.fq;

    // One segment per control kind, named by the kind so a failure points
    // straight at the unwired control.
    const perKindSegments: ReadonlyArray<readonly [string, string]> = [
      ["text", "ui:ai-panel.elicitation.field:summary"],
      ["number", "ui:ai-panel.elicitation.field:amount"],
      ["integer", "ui:ai-panel.elicitation.field:count"],
      ["textarea", "ui:ai-panel.elicitation.field:notes"],
      ["select", "ui:ai-panel.elicitation.field:severity"],
      ["boolean", "ui:ai-panel.elicitation.field:urgent"],
      ["multiselect option a", "ui:ai-panel.elicitation.field:tags.option:a"],
      ["multiselect option b", "ui:ai-panel.elicitation.field:tags.option:b"],
      ["action submit", "ui:ai-panel.elicitation.action:submit"],
      ["action decline", "ui:ai-panel.elicitation.action:decline"],
      ["action cancel", "ui:ai-panel.elicitation.action:cancel"],
    ];

    for (const [kind, segment] of perKindSegments) {
      const leaf = findRegisterRecord(segment);
      expect(
        leaf,
        `${kind} (${segment}) must register a spatial leaf`,
      ).toBeTruthy();
      expect(
        leaf!.parentZone,
        `${kind} leaf must be parented at the scrollback zone`,
      ).toBe(scrollbackFq);
      expect(
        String(leaf!.fq).startsWith(`${zoneFq}/`),
        `${kind} leaf fq must be a path-descendant of the ui:ai-panel zone`,
      ).toBe(true);
      expect(leaf!.layerFq, `${kind} leaf must live in the window layer`).toBe(
        zone!.layerFq,
      );
    }

    unmount();
  });

  // -------------------------------------------------------------------------
  // Drill-in for every text-like field kind: Enter on the focused field leaf
  // runs the per-scope drill-in command, which hands DOM focus to that field's
  // input. The base Escape test covers `text` (summary) only; this table adds
  // number / integer / textarea so each kind's `useFieldDrillIn` wiring is
  // pinned. A reverted `commands` prop on any control fails its row.
  // -------------------------------------------------------------------------

  it("Enter on a focused text-like field leaf drills DOM focus into its input", async () => {
    // [kind, field key, visible label] — the all-kinds form renders several
    // inputs, so each control is disambiguated by its associated label. The
    // labels are mutually non-overlapping, so a loose match is unambiguous
    // (the required `summary` label carries a trailing `*` marker, so it is
    // not anchored).
    const cases: ReadonlyArray<readonly [string, string, RegExp]> = [
      ["text", "summary", /summary/i],
      ["number", "amount", /amount/i],
      ["integer", "count", /count/i],
      ["textarea", "notes", /notes/i],
    ];

    for (const [kind, key, label] of cases) {
      const harness = mockConnect();
      const { unmount } = await renderPrimedPanel(harness);

      await act(async () => {
        void harness.elicitation()(allKindsFormRequest());
      });
      await flushSetup();

      const leaf = findRegisterRecord(`ui:ai-panel.elicitation.field:${key}`);
      expect(leaf, `${kind} field leaf must register`).toBeTruthy();

      // The control associated with this field's label — `getByLabelText`
      // resolves the `<Label htmlFor>` → control `id` association, so the
      // all-kinds form's multiple inputs are never confused.
      const control = screen.getByLabelText(label);

      await drillInto(leaf!.fq as FullyQualifiedMoniker);

      expect(
        document.activeElement,
        `drill-in must land DOM focus on the ${kind} input`,
      ).toBe(control);

      unmount();
    }
  });

  // -------------------------------------------------------------------------
  // Drill-in for the select: Enter on the focused select leaf runs the
  // Pressable's activation, whose `onPress` hands DOM focus to the Radix
  // trigger so its listbox keyboard interaction takes over.
  // -------------------------------------------------------------------------

  it("Enter on the focused select leaf hands DOM focus to the Radix trigger", async () => {
    const harness = mockConnect();
    const { container, unmount } = await renderPrimedPanel(harness);

    await act(async () => {
      void harness.elicitation()(allKindsFormRequest());
    });
    await flushSetup();

    const leaf = findRegisterRecord("ui:ai-panel.elicitation.field:severity");
    expect(leaf, "the select field leaf must register").toBeTruthy();

    const trigger = container.querySelector(
      "[data-slot='select-trigger']",
    ) as HTMLElement | null;
    expect(trigger, "the select trigger must render").not.toBeNull();

    await drillInto(leaf!.fq as FullyQualifiedMoniker);

    expect(
      document.activeElement,
      "select drill-in must hand DOM focus to the Radix trigger",
    ).toBe(trigger);

    unmount();
  });

  // -------------------------------------------------------------------------
  // Activation for the boolean checkbox and a multiselect option: Enter on the
  // focused leaf runs the Pressable's activation, whose `onPress` toggles the
  // value — observable as the checkbox flipping to `aria-checked="true"`.
  // -------------------------------------------------------------------------

  it("Enter on the focused checkbox / option leaf toggles its value", async () => {
    const cases: ReadonlyArray<readonly [string, string, RegExp]> = [
      ["boolean", "ui:ai-panel.elicitation.field:urgent", /^urgent$/i],
      [
        "multiselect option",
        "ui:ai-panel.elicitation.field:tags.option:a",
        /^a$/,
      ],
    ];

    for (const [kind, segment, name] of cases) {
      const harness = mockConnect();
      const { unmount } = await renderPrimedPanel(harness);

      await act(async () => {
        void harness.elicitation()(allKindsFormRequest());
      });
      await flushSetup();

      const leaf = findRegisterRecord(segment);
      expect(leaf, `${kind} leaf must register`).toBeTruthy();

      const checkbox = screen.getByRole("checkbox", { name });
      expect(
        checkbox.getAttribute("aria-checked"),
        `${kind} checkbox starts unchecked`,
      ).toBe("false");

      await drillInto(leaf!.fq as FullyQualifiedMoniker);

      expect(
        checkbox.getAttribute("aria-checked"),
        `Enter on the ${kind} leaf must toggle it checked`,
      ).toBe("true");

      unmount();
    }
  });

  // -------------------------------------------------------------------------
  // Enter on the focused Submit leaf submits the form (accept payload).
  // -------------------------------------------------------------------------

  it("Enter on the focused Submit leaf submits the form with the accept payload", async () => {
    const harness = mockConnect();
    const { container, unmount } = await renderPrimedPanel(harness);

    let outcome: Promise<CreateElicitationResponse> | undefined;
    await act(async () => {
      outcome = harness.elicitation()(formRequest());
    });
    await flushSetup();

    // Fill the required field by typing into its input.
    const summaryInput = container.querySelector(
      "input[data-slot='input']",
    ) as HTMLInputElement | null;
    expect(summaryInput, "the summary text input must render").not.toBeNull();
    await act(async () => {
      await userEvent.type(summaryInput!, "all green");
    });

    const submit = findRegisterRecord("ui:ai-panel.elicitation.action:submit");
    expect(submit, "the Submit leaf must register").toBeTruthy();
    const submitFq = submit!.fq as FullyQualifiedMoniker;

    // Seed spatial focus on the Submit leaf, then press a real Enter through
    // the keybinding pipeline.
    await act(async () => {
      await mockInvoke("spatial_focus", { fq: submitFq });
    });
    await flushSetup();
    await act(async () => {
      fireEvent.keyDown(document, { key: "Enter", code: "Enter" });
      await Promise.resolve();
    });
    await flushSetup();

    // The accept content carries the typed summary plus the boolean field
    // (a boolean is always included — `false` is a real answer). The
    // untouched optional select and the empty multiselect are omitted.
    await expect(outcome).resolves.toEqual({
      action: "accept",
      content: { summary: "all green", urgent: false },
    });

    unmount();
  });

  // -------------------------------------------------------------------------
  // Escape inside a drilled-into text field drills back OUT: it blurs the
  // input (DOM focus leaves the trapping control) and dispatches `spatial_focus`
  // for the field's own leaf, so the panel stops trapping keys and a subsequent
  // global command (the `s` jump path) is reachable again. This mirrors the
  // composer's `ComposerEditorDrillOutWiring` Escape drill-out.
  // -------------------------------------------------------------------------

  it("Escape inside a focused text field blurs it and returns spatial focus to the field leaf", async () => {
    const harness = mockConnect();
    const { container, unmount } = await renderPrimedPanel(harness);

    await act(async () => {
      void harness.elicitation()(formRequest());
    });
    await flushSetup();

    const summary = findRegisterRecord("ui:ai-panel.elicitation.field:summary");
    expect(summary, "the summary field leaf must register").toBeTruthy();
    const summaryFq = summary!.fq as FullyQualifiedMoniker;

    // Drill INTO the field: seed spatial focus on the field leaf, press Enter
    // through the keybinding pipeline so the per-scope drill-in command hands
    // DOM focus to the input, and type into it.
    await act(async () => {
      await mockInvoke("spatial_focus", { fq: summaryFq });
    });
    await flushSetup();
    await act(async () => {
      fireEvent.keyDown(document, { key: "Enter", code: "Enter" });
      await Promise.resolve();
    });
    await flushSetup();

    const summaryInput = container.querySelector(
      "input[data-slot='input']",
    ) as HTMLInputElement | null;
    expect(summaryInput, "the summary text input must render").not.toBeNull();
    expect(
      document.activeElement,
      "drill-in must land DOM focus on the summary input",
    ).toBe(summaryInput);
    await act(async () => {
      await userEvent.type(summaryInput!, "hello");
    });

    // Forget every focus call made up to here so the assertion below only sees
    // the drill-out's `spatial_focus`.
    const focusCallsBefore = mockInvoke.mock.calls.filter(
      (c) => (c[0] === "spatial_focus" || (c[0] === "command_tool_call" && (c[1] as any)?.tool === "focus" && (c[1] as any)?.op === "set focus")),
    ).length;

    // Drill OUT: Escape on the focused input. The field's `useFieldDrillOut`
    // handler must `preventDefault`, blur the input, and dispatch `nav.focus`
    // (→ `spatial_focus`) for the field's own leaf.
    await act(async () => {
      fireEvent.keyDown(summaryInput!, { key: "Escape", code: "Escape" });
      await Promise.resolve();
    });
    await flushSetup();

    // (1) DOM focus left the trapping input.
    expect(
      document.activeElement === summaryInput,
      "Escape must blur the summary input so it stops trapping keys",
    ).toBe(false);

    // (2) Spatial focus was claimed for the field leaf via `spatial_focus`.
    const drillOutFocus = mockInvoke.mock.calls
      .filter((c) => (c[0] === "spatial_focus" || (c[0] === "command_tool_call" && (c[1] as any)?.tool === "focus" && (c[1] as any)?.op === "set focus")))
      .slice(focusCallsBefore);
    expect(
      drillOutFocus.some(
        (c) => (c[1] as Record<string, unknown>).fq === summaryFq,
      ),
      "Escape must dispatch spatial_focus for the field's own leaf",
    ).toBe(true);

    // (3) With DOM focus released, a global single-key command is reachable
    // again: `s` (jump) flows through the keybinding pipeline now that the
    // input no longer swallows the keystroke. Before the drill-out the focused
    // input would have consumed it (an editable target).
    await act(async () => {
      fireEvent.keyDown(document.body, { key: "s", code: "KeyS" });
      await Promise.resolve();
    });
    await flushSetup();

    unmount();
  });

  // -------------------------------------------------------------------------
  // Drill-out for the textarea and the numeric inputs, mirroring the text
  // drill-out above. The base Escape test pins `text` only; this table adds
  // `textarea` / `number` / `integer` so each kind's `useFieldDrillOut` Escape
  // handler (blur + `spatial_focus` back to the field leaf) is pinned. Removing
  // the `onKeyDown` drill-out from any of these controls fails its row.
  // -------------------------------------------------------------------------

  it("Escape inside a focused text-like field blurs it and returns spatial focus to the field leaf", async () => {
    // [kind, field key, visible label] — drives the all-kinds form so every
    // text-like control kind is exercised. (`text` is pinned by the dedicated
    // Escape test above, so this table covers the remaining kinds.)
    const cases: ReadonlyArray<readonly [string, string, RegExp]> = [
      ["textarea", "notes", /notes/i],
      ["number", "amount", /amount/i],
      ["integer", "count", /count/i],
    ];

    for (const [kind, key, label] of cases) {
      const harness = mockConnect();
      const { unmount } = await renderPrimedPanel(harness);

      await act(async () => {
        void harness.elicitation()(allKindsFormRequest());
      });
      await flushSetup();

      const leaf = findRegisterRecord(`ui:ai-panel.elicitation.field:${key}`);
      expect(leaf, `${kind} field leaf must register`).toBeTruthy();
      const leafFq = leaf!.fq as FullyQualifiedMoniker;

      // Drill IN: Enter on the field leaf hands DOM focus to the input.
      await drillInto(leafFq);

      const control = screen.getByLabelText(label);
      expect(
        document.activeElement,
        `drill-in must land DOM focus on the ${kind} input`,
      ).toBe(control);

      // Ignore every focus call up to here so the assertion sees only the
      // drill-out's `spatial_focus`.
      const focusCallsBefore = focusCallCount();

      // Drill OUT: Escape on the focused control must blur it and dispatch
      // `nav.focus` (→ `spatial_focus`) for the field's own leaf.
      await act(async () => {
        fireEvent.keyDown(control, { key: "Escape", code: "Escape" });
        await Promise.resolve();
      });
      await flushSetup();

      // (1) DOM focus left the trapping control.
      expect(
        document.activeElement === control,
        `Escape must blur the ${kind} input so it stops trapping keys`,
      ).toBe(false);

      // (2) Spatial focus was reclaimed for the field leaf via `spatial_focus`.
      const drillOutFocus = mockInvoke.mock.calls
        .filter((c) => (c[0] === "spatial_focus" || (c[0] === "command_tool_call" && (c[1] as any)?.tool === "focus" && (c[1] as any)?.op === "set focus")))
        .slice(focusCallsBefore);
      expect(
        drillOutFocus.some(
          (c) => (c[1] as Record<string, unknown>).fq === leafFq,
        ),
        `Escape on the ${kind} input must dispatch spatial_focus for its field leaf`,
      ).toBe(true);

      unmount();
    }
  });

  // -------------------------------------------------------------------------
  // Enter on the focused Decline leaf declines.
  // -------------------------------------------------------------------------

  it("Enter on the focused Decline leaf sends a decline action", async () => {
    const harness = mockConnect();
    const { unmount } = await renderPrimedPanel(harness);

    let outcome: Promise<CreateElicitationResponse> | undefined;
    await act(async () => {
      outcome = harness.elicitation()(formRequest());
    });
    await flushSetup();

    const decline = findRegisterRecord(
      "ui:ai-panel.elicitation.action:decline",
    );
    expect(decline, "the Decline leaf must register").toBeTruthy();
    const declineFq = decline!.fq as FullyQualifiedMoniker;

    await act(async () => {
      await mockInvoke("spatial_focus", { fq: declineFq });
    });
    await flushSetup();
    await act(async () => {
      fireEvent.keyDown(document, { key: "Enter", code: "Enter" });
      await Promise.resolve();
    });
    await flushSetup();

    await expect(outcome).resolves.toEqual({ action: "decline" });

    unmount();
  });

  // -------------------------------------------------------------------------
  // Enter on the focused Cancel leaf cancels.
  // -------------------------------------------------------------------------

  it("Enter on the focused Cancel leaf sends a cancel action", async () => {
    const harness = mockConnect();
    const { unmount } = await renderPrimedPanel(harness);

    let outcome: Promise<CreateElicitationResponse> | undefined;
    await act(async () => {
      outcome = harness.elicitation()(formRequest());
    });
    await flushSetup();

    const cancel = findRegisterRecord("ui:ai-panel.elicitation.action:cancel");
    expect(cancel, "the Cancel leaf must register").toBeTruthy();
    const cancelFq = cancel!.fq as FullyQualifiedMoniker;

    await act(async () => {
      await mockInvoke("spatial_focus", { fq: cancelFq });
    });
    await flushSetup();
    await act(async () => {
      fireEvent.keyDown(document, { key: "Enter", code: "Enter" });
      await Promise.resolve();
    });
    await flushSetup();

    await expect(outcome).resolves.toEqual({ action: "cancel" });

    unmount();
  });

  // -------------------------------------------------------------------------
  // Url mode: Done / Cancel each register a spatial leaf, and Enter on the
  // focused Done leaf accepts.
  // -------------------------------------------------------------------------

  it("url mode registers Done / Cancel leaves and Enter on Done accepts", async () => {
    const harness = mockConnect();
    const { unmount } = await renderPrimedPanel(harness);

    let outcome: Promise<CreateElicitationResponse> | undefined;
    await act(async () => {
      outcome = harness.elicitation()(urlRequest());
    });
    await flushSetup();

    const zone = findRegisterRecord("ui:ai-panel");
    const done = findRegisterRecord("ui:ai-panel.elicitation.action:done");
    const cancel = findRegisterRecord("ui:ai-panel.elicitation.action:cancel");
    expect(zone, "the panel zone must register").toBeTruthy();
    expect(done, "the Done leaf must register").toBeTruthy();
    expect(cancel, "the url-mode Cancel leaf must register").toBeTruthy();

    const zoneFq = String(zone!.fq);
    expect(
      String(done!.fq).startsWith(`${zoneFq}/`),
      "the Done leaf fq must be a path-descendant of the ui:ai-panel zone",
    ).toBe(true);

    const doneFq = done!.fq as FullyQualifiedMoniker;
    await act(async () => {
      await mockInvoke("spatial_focus", { fq: doneFq });
    });
    await flushSetup();
    await act(async () => {
      fireEvent.keyDown(document, { key: "Enter", code: "Enter" });
      await Promise.resolve();
    });
    await flushSetup();

    await expect(outcome).resolves.toEqual({ action: "accept", content: {} });

    unmount();
  });

  // -------------------------------------------------------------------------
  // Url mode: Enter on the focused Cancel leaf cancels. The test above pins
  // Done; this pins the url-mode Cancel action so both url buttons are
  // keyboard-activable (a reverted `AiPanelPressable` → `<Button>` fails it).
  // -------------------------------------------------------------------------

  it("url mode: Enter on the focused Cancel leaf sends a cancel action", async () => {
    const harness = mockConnect();
    const { unmount } = await renderPrimedPanel(harness);

    let outcome: Promise<CreateElicitationResponse> | undefined;
    await act(async () => {
      outcome = harness.elicitation()(urlRequest());
    });
    await flushSetup();

    const cancel = findRegisterRecord("ui:ai-panel.elicitation.action:cancel");
    expect(cancel, "the url-mode Cancel leaf must register").toBeTruthy();
    const cancelFq = cancel!.fq as FullyQualifiedMoniker;

    await act(async () => {
      await mockInvoke("spatial_focus", { fq: cancelFq });
    });
    await flushSetup();
    await act(async () => {
      fireEvent.keyDown(document, { key: "Enter", code: "Enter" });
      await Promise.resolve();
    });
    await flushSetup();

    await expect(outcome).resolves.toEqual({ action: "cancel" });

    unmount();
  });
});
