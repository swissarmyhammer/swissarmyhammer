/**
 * Browser-mode tests pinning the "card field icons live inside the field's
 * `<FocusScope>`" contract. Mirrors the inspector path pinned by
 * `field.with-icon.browser.test.tsx`.
 *
 * Before this migration, `<CardField>` rendered a `<CardFieldIcon>` as a
 * sibling of `<Field>` inside an outer flex wrapper — the icon lived OUTSIDE
 * the field zone, so a click on the icon did not focus the zone, the
 * `<FocusIndicator>` painted between the icon and the content, and the
 * debug-border encompassed only the edit area. After this migration,
 * `<CardField>` renders a single `<Field withIcon />`, which puts the icon
 * inside the field zone exactly the way the inspector's `FieldRow` does.
 *
 * The tests below assert each visible-and-architectural property the
 * migration is supposed to fix.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, fireEvent, act } from "@testing-library/react";
import { CheckCircle, AlertTriangle } from "lucide-react";

// ---------------------------------------------------------------------------
// Tauri API mocks — must come before component imports.
// ---------------------------------------------------------------------------

type ListenCallback = (event: { payload: unknown }) => void;

const { mockInvoke, mockListen, listeners } = vi.hoisted(() => {
  const listeners = new Map<string, ListenCallback[]>();
  const mockInvoke = vi.fn(
    async (_cmd: string, _args?: unknown): Promise<unknown> => undefined,
  );
  const mockListen = vi.fn(
    (eventName: string, cb: ListenCallback): Promise<() => void> => {
      const cbs = listeners.get(eventName) ?? [];
      cbs.push(cb);
      listeners.set(eventName, cbs);
      return Promise.resolve(() => {
        const arr = listeners.get(eventName);
        if (arr) {
          const idx = arr.indexOf(cb);
          if (idx >= 0) arr.splice(idx, 1);
        }
      });
    },
  );
  return { mockInvoke, mockListen, listeners };
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

vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({
    onDragDropEvent: vi.fn(() => Promise.resolve(() => {})),
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

import "@/components/fields/registrations";
import { registerDisplay } from "@/components/fields/field";
import { EntityCard } from "@/components/entity-card";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { FieldUpdateProvider } from "@/lib/field-update-context";
import { UIStateProvider } from "@/lib/ui-state-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { FocusLayer } from "@/components/focus-layer";
import { CommandBusyProvider, CommandScopeProvider } from "@/lib/command-scope";
import { asSegment, type FullyQualifiedMoniker } from "@/types/spatial";
import type { Entity } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Schema seeds — task with a "tags" header field carrying icon=tag, a
// "title" header field with no icon, and an "override_field" header field
// whose display registers an iconOverride/tooltipOverride.
// ---------------------------------------------------------------------------

const TASK_SCHEMA = {
  entity: {
    name: "task",
    body_field: "body",
    fields: ["title", "tags", "override_field", "body"],
  },
  fields: [
    {
      id: "f-title",
      name: "title",
      type: { kind: "markdown", single_line: true },
      editor: "markdown",
      display: "text",
      section: "header",
    },
    {
      id: "f-tags",
      name: "tags",
      type: { kind: "computed", derive: "parse-body-tags" },
      editor: "multi-select",
      display: "badge-list",
      icon: "tag",
      description: "Task tags",
      section: "header",
    },
    {
      id: "f-override",
      name: "override_field",
      type: { kind: "text", single_line: true },
      editor: "markdown",
      display: "card-icon-override-display",
      icon: "tag",
      description: "Static description",
      section: "header",
    },
    {
      id: "f-body",
      name: "body",
      type: { kind: "markdown", single_line: false },
      editor: "markdown",
      display: "markdown",
      section: "body",
    },
  ],
};

/** Schema variant with NO icon on any header field — used to assert
 *  that `<Field withIcon />` does not render an icon slot when the
 *  field has no icon and no override. */
const ICONLESS_TASK_SCHEMA = {
  entity: {
    name: "task",
    body_field: "body",
    fields: ["title", "body"],
  },
  fields: [
    {
      id: "f-title",
      name: "title",
      type: { kind: "markdown", single_line: true },
      editor: "markdown",
      display: "text",
      section: "header",
    },
    {
      id: "f-body",
      name: "body",
      type: { kind: "markdown", single_line: false },
      editor: "markdown",
      display: "markdown",
      section: "body",
    },
  ],
};

let activeSchema: typeof TASK_SCHEMA = TASK_SCHEMA;

/** Default invoke responses for the mount-time IPCs the providers fire. */
async function defaultInvokeImpl(
  cmd: string,
  _args?: unknown,
): Promise<unknown> {
  if (cmd === "list_entity_types") return ["task"];
  if (cmd === "get_entity_schema") return activeSchema;
  if (cmd === "get_ui_state")
    return {
      palette_open: false,
      palette_mode: "command",
      keymap_mode: "cua",
      scope_chain: [],
      open_boards: [],
      windows: {},
      recent_boards: [],
    };
  if (cmd === "get_undo_state") return { can_undo: false, can_redo: false };
  if (cmd === "list_commands_for_scope") return [];
  if (cmd === "dispatch_command") return undefined;
  if (cmd === "spatial_drill_in") return null;
  return undefined;
}

/** Build a task entity with optional field overrides. */
function makeTask(fields: Record<string, unknown> = {}): Entity {
  return {
    entity_type: "task",
    id: "T1",
    moniker: "task:T1",
    fields: {
      title: "Hello",
      body: "",
      tags: [],
      override_field: "",
      ...fields,
    },
  };
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Wait for register effects scheduled in `useEffect` to flush. */
async function flushSetup() {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 50));
  });
}

/** Collect every `spatial_register_scope` invocation argument bag. */
function registerScopeArgs(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_register_scope")
    .map((c) => c[1] as Record<string, unknown>);
}

/** Collect every `spatial_focus` call's args, in order. */
function spatialFocusCalls(): Array<{ fq: FullyQualifiedMoniker }> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_focus")
    .map((c) => c[1] as { fq: FullyQualifiedMoniker });
}

/**
 * Render a single `<EntityCard>` inside the production-shaped provider
 * stack, including the spatial-nav stack (so `<FocusScope>` registers and
 * click-to-focus dispatches `spatial_focus`).
 */
function renderCard(entity: Entity) {
  return render(
    <CommandBusyProvider>
      <SpatialFocusProvider>
        <FocusLayer name={asSegment("window")}>
          <TooltipProvider delayDuration={0}>
            <SchemaProvider>
              <EntityStoreProvider entities={{ task: [entity], tag: [] }}>
                <EntityFocusProvider>
                  <FieldUpdateProvider>
                    <UIStateProvider>
                      <CommandScopeProvider commands={[]}>
                        <EntityCard entity={entity} />
                      </CommandScopeProvider>
                    </UIStateProvider>
                  </FieldUpdateProvider>
                </EntityFocusProvider>
              </EntityStoreProvider>
            </SchemaProvider>
          </TooltipProvider>
        </FocusLayer>
      </SpatialFocusProvider>
    </CommandBusyProvider>,
  );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("EntityCard — field icon lives inside the field's <FocusScope>", () => {
  beforeEach(() => {
    activeSchema = TASK_SCHEMA;
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
    mockInvoke.mockImplementation(defaultInvokeImpl);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  // -------------------------------------------------------------------------
  // #1: The icon for a card field must be a descendant of the field's
  //     `<FocusScope>` wrapper — not a sibling. Pre-migration this fails
  //     because `<CardFieldIcon>` rendered as a sibling of `<Field>`.
  // -------------------------------------------------------------------------

  it("card_field_icon_is_descendant_of_field_zone", async () => {
    const { container, unmount } = renderCard(makeTask({ tags: ["bug"] }));
    await flushSetup();

    const fieldZone = container.querySelector(
      '[data-segment="field:task:T1.tags"]',
    );
    expect(
      fieldZone,
      "the card's field zone wrapper for `tags` must be in the DOM",
    ).not.toBeNull();

    // Tooltip-trigger span = the `<FieldIconBadge>` outer node — the lucide
    // icon lives inside it. Assert the badge is a descendant of the field
    // zone, NOT a sibling.
    const iconBadge = fieldZone!.querySelector(
      'span[data-slot="tooltip-trigger"]',
    );
    expect(
      iconBadge,
      "the field icon badge must render as a descendant of the field zone wrapper (not a sibling)",
    ).not.toBeNull();

    // The lucide SVG itself must also be a descendant of the field zone.
    const svg = fieldZone!.querySelector("svg");
    expect(
      svg,
      "the lucide SVG must live inside the field zone wrapper",
    ).not.toBeNull();

    unmount();
  });

  // -------------------------------------------------------------------------
  // #2: Clicking the icon focuses the field zone — the icon click bubbles
  //     to the zone's onClick (because the icon is now a descendant of the
  //     zone). Pre-migration this dispatched no `spatial_focus` because
  //     the icon lived outside the zone.
  // -------------------------------------------------------------------------

  it("clicking_card_field_icon_focuses_field_zone", async () => {
    const { container, unmount } = renderCard(makeTask({ tags: ["bug"] }));
    await flushSetup();

    const zoneArgs = registerScopeArgs().find(
      (a) => a.segment === "field:task:T1.tags",
    );
    expect(
      zoneArgs,
      "the field zone for `tags` must have registered with the kernel",
    ).toBeTruthy();

    mockInvoke.mockClear();
    const fieldZone = container.querySelector(
      '[data-segment="field:task:T1.tags"]',
    );
    expect(fieldZone).not.toBeNull();
    const iconBadge = fieldZone!.querySelector(
      'span[data-slot="tooltip-trigger"]',
    ) as HTMLElement | null;
    expect(iconBadge).not.toBeNull();

    fireEvent.click(iconBadge!);
    await flushSetup();

    const focusCalls = spatialFocusCalls();
    expect(
      focusCalls.length,
      "clicking the icon must dispatch spatial_focus at least once",
    ).toBeGreaterThanOrEqual(1);
    expect(
      focusCalls[0].fq,
      "the focus key must be the field zone's key (icon click bubbles to the zone)",
    ).toBe(zoneArgs!.fq);

    unmount();
  });

  // -------------------------------------------------------------------------
  // #3: Clicking the content (anything inside the zone but not the icon)
  //     also focuses the field zone. Regression guard — pre-migration the
  //     content lived inside an inner flex-1 wrapper that already received
  //     the click; this test confirms the unified zone shape preserves
  //     content-click focus.
  // -------------------------------------------------------------------------

  it("clicking_card_field_content_focuses_field_zone", async () => {
    const { container, unmount } = renderCard(makeTask({ tags: ["bug"] }));
    await flushSetup();

    const zoneArgs = registerScopeArgs().find(
      (a) => a.segment === "field:task:T1.tags",
    );
    expect(zoneArgs).toBeTruthy();

    mockInvoke.mockClear();
    // Inside the field zone wrapper, `<Field withIcon />` puts the
    // content under a `flex-1 min-w-0` div sibling to the icon. Click
    // anywhere inside it.
    const contentWrap = container.querySelector(
      '[data-segment="field:task:T1.tags"] .flex-1.min-w-0',
    ) as HTMLElement | null;
    expect(
      contentWrap,
      "the content wrapper inside the field zone must exist",
    ).not.toBeNull();

    fireEvent.click(contentWrap!);
    await flushSetup();

    const focusCalls = spatialFocusCalls();
    expect(focusCalls.length).toBeGreaterThanOrEqual(1);
    expect(focusCalls[0].fq).toBe(zoneArgs!.fq);

    unmount();
  });

  // -------------------------------------------------------------------------
  // #4: When the field zone is focused, the `<FocusIndicator>` is a
  //     descendant of the field zone wrapper, the icon badge is also a
  //     descendant, and the indicator paints inside the zone's box as a
  //     dotted border tracing the wrapper's bounds — surrounding both
  //     the icon and the content rather than living outside the box.
  // -------------------------------------------------------------------------

  it("focus_indicator_paints_inside_field_zone_in_card", async () => {
    const { container, unmount } = renderCard(makeTask({ tags: ["bug"] }));
    await flushSetup();

    const zoneArgs = registerScopeArgs().find(
      (a) => a.segment === "field:task:T1.tags",
    );
    expect(zoneArgs).toBeTruthy();

    // Drive a focus claim for the field zone.
    const handlers = listeners.get("focus-changed") ?? [];
    await act(async () => {
      for (const handler of handlers)
        handler({
          payload: {
            window_label: "main",
            prev_fq: null,
            next_fq: zoneArgs!.fq,
            next_segment: asSegment("field:task:T1.tags"),
          },
        });
      await Promise.resolve();
    });
    await flushSetup();

    const fieldZone = container.querySelector(
      '[data-segment="field:task:T1.tags"]',
    ) as HTMLElement | null;
    expect(fieldZone).not.toBeNull();
    expect(fieldZone!.getAttribute("data-focused")).toBe("true");

    const indicator = fieldZone!.querySelector(
      "[data-testid='focus-indicator']",
    );
    expect(
      indicator,
      "the focused field zone must mount a <FocusIndicator>",
    ).not.toBeNull();

    const iconBadge = fieldZone!.querySelector(
      'span[data-slot="tooltip-trigger"]',
    );
    expect(iconBadge).not.toBeNull();

    // Indicator and icon are both descendants of the same field zone.
    expect(
      fieldZone!.contains(indicator),
      "focus indicator must live inside the field zone wrapper",
    ).toBe(true);
    expect(
      fieldZone!.contains(iconBadge),
      "icon badge must live inside the field zone wrapper",
    ).toBe(true);
    // The indicator's class string carries `inset-0` and the dotted
    // border tokens — pin the contract that it paints inside the zone's
    // box as an outline tracing the wrapper, not outside it.
    expect(indicator!.className).toContain("inset-0");
    expect(indicator!.className).toContain("border-dotted");
    expect(indicator!.className).toContain("border-primary");
    // Indicator and the icon's flex-row ancestor are siblings inside the
    // zone — neither contains the other.
    let flexRow: HTMLElement | null = iconBadge!.parentElement;
    while (
      flexRow &&
      flexRow !== fieldZone &&
      !flexRow.className.split(/\s+/).includes("flex")
    ) {
      flexRow = flexRow.parentElement;
    }
    expect(
      flexRow,
      "icon badge has no flex-row ancestor between it and the field zone",
    ).toBeTruthy();
    expect(indicator!.contains(flexRow as Node)).toBe(false);
    expect(flexRow!.contains(indicator)).toBe(false);

    unmount();
  });

  // -------------------------------------------------------------------------
  // #5: A field with no icon and no iconOverride renders content only —
  //     no empty icon slot, no orphan flex wrapper around the field zone.
  // -------------------------------------------------------------------------

  it("card_field_without_icon_renders_content_only", async () => {
    activeSchema = ICONLESS_TASK_SCHEMA;
    const { container, unmount } = renderCard(makeTask({ title: "Hello" }));
    await flushSetup();

    const fieldZone = container.querySelector(
      '[data-segment="field:task:T1.title"]',
    );
    expect(
      fieldZone,
      "the title field zone must still mount even without an icon",
    ).not.toBeNull();

    const iconBadge = fieldZone!.querySelector(
      'span[data-slot="tooltip-trigger"]',
    );
    expect(
      iconBadge,
      "icon-less field must not render an empty icon badge slot",
    ).toBeNull();

    // Sanity: there should be no orphaned svg inside the field zone for
    // the title field (text fields have no icons by default).
    const svg = fieldZone!.querySelector("svg");
    expect(
      svg,
      "icon-less field must not render any svg inside the field zone",
    ).toBeNull();

    unmount();
  });

  // -------------------------------------------------------------------------
  // #6: A display registered with `iconOverride(value)` → returns a
  //     non-default lucide icon for a known value. The card must render
  //     that override, not the static YAML icon.
  // -------------------------------------------------------------------------

  it("card_field_icon_uses_value_dependent_iconOverride", async () => {
    function NoopDisplay() {
      return null;
    }
    registerDisplay("card-icon-override-display", NoopDisplay, {
      iconOverride: (v: unknown) => (v === "ok" ? CheckCircle : AlertTriangle),
    });

    const { container, unmount } = renderCard(
      makeTask({ override_field: "ok" }),
    );
    await flushSetup();

    const svg = container.querySelector(
      '[data-segment="field:task:T1.override_field"] svg',
    ) as SVGElement | null;
    expect(svg).not.toBeNull();
    // CheckCircle's lucide class fingerprint differs across lucide
    // versions; accept any of the known markers AND assert the static
    // `tag` icon is NOT present.
    const className = svg!.getAttribute("class") ?? "";
    expect(
      className.includes("lucide-circle-check-big") ||
        className.includes("lucide-check-circle") ||
        className.includes("lucide-check"),
      `iconOverride must replace the static tag icon. svg classes: ${className}`,
    ).toBe(true);
    expect(className).not.toContain("lucide-tag");

    unmount();
  });

  // -------------------------------------------------------------------------
  // #7: A display registered with `tooltipOverride(value)` → returns a
  //     non-default tooltip string. The card's icon badge must carry that
  //     override text in its rendered tooltip surface.
  // -------------------------------------------------------------------------

  it("card_field_icon_uses_value_dependent_tooltipOverride", async () => {
    function NoopDisplay() {
      return null;
    }
    registerDisplay("card-icon-override-display", NoopDisplay, {
      iconOverride: (v: unknown) => (v === "ok" ? CheckCircle : AlertTriangle),
      tooltipOverride: (v: unknown) =>
        v === "ok" ? "All good" : "Something is wrong",
    });

    const { container, unmount } = renderCard(
      makeTask({ override_field: "ok" }),
    );
    await flushSetup();

    // Open the tooltip by hovering the trigger. Radix mounts the
    // content asynchronously; flush a tick.
    const trigger = container.querySelector(
      '[data-segment="field:task:T1.override_field"] span[data-slot="tooltip-trigger"]',
    ) as HTMLElement | null;
    expect(trigger).not.toBeNull();

    await act(async () => {
      fireEvent.pointerEnter(trigger!);
      fireEvent.focus(trigger!);
      await new Promise((r) => setTimeout(r, 50));
    });

    // Radix renders tooltip content into the document body. Look for
    // any element whose text matches our override string.
    const allText = document.body.textContent ?? "";
    expect(
      allText.includes("All good"),
      `tooltipOverride text must appear in the rendered tooltip. document text: ${allText}`,
    ).toBe(true);
    expect(allText).not.toContain("Static description");

    unmount();
  });
});
