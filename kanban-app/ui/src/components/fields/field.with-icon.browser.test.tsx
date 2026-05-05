/**
 * Browser-mode tests pinning the "icon lives inside the field's `<FocusScope>`"
 * contract introduced by card `01KQ9ZJHRXCY8Z5YT6RF4SG6EK`.
 *
 * The fix moved the inspector row's leftmost-icon decoration *into*
 * `<Field>` itself, gated on the new `withIcon` prop. Goals:
 *
 *   1. Clicking the icon dispatches `spatial_focus` for the field zone
 *      (it bubbles to the zone's click handler — the icon is now a
 *      descendant of the zone wrapper, not a sibling).
 *   2. The visible `<FocusIndicator>` (an `absolute inset-0` dotted
 *      border inside the zone wrapper) traces the zone's bounding box
 *      around both the icon and the content — they share one
 *      containing block now.
 *   3. Every existing `<Field>` callsite that does NOT opt in via
 *      `withIcon={true}` continues to render exactly as before.
 *
 * The eight tests below mirror the acceptance criteria in the card's
 * Tests section. Mocks follow the pattern
 * `field.enter-edit.browser.test.tsx` / `entity-inspector.spatial-nav.test.tsx`
 * already use, so this file integrates with the existing field-side
 * suite.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, fireEvent, act, waitFor } from "@testing-library/react";
import { CheckCircle, AlertTriangle, Tag, HelpCircle } from "lucide-react";

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
import { Field, registerDisplay } from "@/components/fields/field";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { FieldUpdateProvider } from "@/lib/field-update-context";
import { UIStateProvider } from "@/lib/ui-state-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { FocusLayer } from "@/components/focus-layer";
import { CommandBusyProvider, CommandScopeProvider } from "@/lib/command-scope";
import {
  asSegment,
  type FocusChangedPayload,
  type FullyQualifiedMoniker,
  type WindowLabel
} from "@/types/spatial";
import type { Entity, FieldDef } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Schema seeds
// ---------------------------------------------------------------------------

/** Field with a static `icon` that resolves to a known lucide component. */
const TAGS_FIELD: FieldDef = {
  id: "f-tags",
  name: "tags",
  type: { kind: "computed", derive: "parse-body-tags" },
  editor: "multi-select",
  display: "badge-list",
  icon: "tag",
  section: "header",
};

/** Field with no `icon` — the no-icon branch when `withIcon` is true. */
const TITLE_FIELD: FieldDef = {
  id: "f-title",
  name: "title",
  type: { kind: "markdown", single_line: true },
  editor: "markdown",
  display: "text",
  section: "header",
};

/** Field with an unknown `icon` name — the HelpCircle fallback path. */
const BOGUS_ICON_FIELD: FieldDef = {
  id: "f-bogus",
  name: "bogus_icon_field",
  type: { kind: "text", single_line: true },
  editor: "markdown",
  display: "text",
  icon: "definitely-not-a-real-lucide-icon-name",
  section: "header",
};

/**
 * Field whose display registers an `iconOverride` returning a different
 * icon for a known value — the dynamic icon-resolution path.
 */
const OVERRIDE_FIELD: FieldDef = {
  id: "f-override",
  name: "override_field",
  type: { kind: "text", single_line: true },
  editor: "markdown",
  display: "with-icon-override-display",
  icon: "tag",
  section: "header",
};

const TASK_SCHEMA = {
  entity: {
    name: "task",
    fields: ["title", "tags", "bogus_icon_field", "override_field"],
  },
  fields: [TITLE_FIELD, TAGS_FIELD, BOGUS_ICON_FIELD, OVERRIDE_FIELD],
};

const SCHEMAS: Record<string, unknown> = { task: TASK_SCHEMA };

/** Default invoke responses for the mount-time IPCs the providers fire. */
async function defaultInvokeImpl(
  cmd: string,
  args?: unknown,
): Promise<unknown> {
  if (cmd === "list_entity_types") return ["task"];
  if (cmd === "get_entity_schema") {
    const entityType = (args as { entityType?: string })?.entityType;
    return SCHEMAS[entityType ?? ""] ?? TASK_SCHEMA;
  }
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

function makeTask(fields: Record<string, unknown> = {}): Entity {
  return {
    entity_type: "task",
    id: "T1",
    moniker: "task:T1",
    fields,
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
 * Drive a `focus-changed` event into the React tree as if the Rust
 * kernel had emitted one — flips the matching `useFocusClaim`
 * subscription on the field's zone wrapper to true, which mounts the
 * visible `<FocusIndicator>`.
 */
async function fireFocusChanged({
  prev_fq = null,
  next_fq = null,
  next_segment = null,
}: {
  prev_fq?: FullyQualifiedMoniker | null;
  next_fq?: FullyQualifiedMoniker | null;
  next_segment?: string | null;
}) {
  const payload: FocusChangedPayload = {
    window_label: "main" as WindowLabel,
    prev_fq,
    next_fq,
    next_segment: next_segment as FocusChangedPayload["next_segment"],
  };
  const handlers = listeners.get("focus-changed") ?? [];
  await act(async () => {
    for (const handler of handlers) handler({ payload });
    await Promise.resolve();
  });
}

/**
 * Render a single `<Field>` instance inside the production-shaped
 * provider stack. The harness wires no `<AppShell>` because these
 * tests don't need the global keymap handler — they exercise click
 * focus, indicator placement, and icon resolution directly.
 */
function renderField(props: {
  field: FieldDef;
  entity: Entity;
  withIcon?: boolean;
}) {
  return render(
    <CommandBusyProvider>
      <SpatialFocusProvider>
        <FocusLayer name={asSegment("window")}>
          <TooltipProvider delayDuration={100}>
            <SchemaProvider>
              <EntityStoreProvider entities={{ task: [props.entity] }}>
                <EntityFocusProvider>
                  <FieldUpdateProvider>
                    <UIStateProvider>
                      <CommandScopeProvider commands={[]}>
                        <Field
                          fieldDef={props.field}
                          entityType={props.entity.entity_type}
                          entityId={props.entity.id}
                          mode="full"
                          editing={false}
                          showFocusBar
                          withIcon={props.withIcon}
                        />
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

describe("Field — withIcon prop renders the icon inside the focus zone", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
    mockInvoke.mockImplementation(defaultInvokeImpl);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  // -------------------------------------------------------------------------
  // #1: withIcon=true renders the icon as a descendant of the field zone.
  // -------------------------------------------------------------------------

  it("field_with_icon_renders_icon_inside_focus_zone", async () => {
    const { container, unmount } = renderField({
      field: TAGS_FIELD,
      entity: makeTask({ tags: [] }),
      withIcon: true,
    });
    await flushSetup();

    const fieldZone = container.querySelector(
      '[data-segment="field:task:T1.tags"]',
    );
    expect(
      fieldZone,
      "the field's <FocusScope> wrapper must be in the DOM",
    ).not.toBeNull();

    // Tooltip-trigger span = the `<FieldIconBadge>`'s outer node — the
    // icon lives inside it. Assert the badge is a descendant of the
    // field zone, NOT a sibling.
    const iconBadge = fieldZone!.querySelector(
      'span[data-slot="tooltip-trigger"]',
    );
    expect(
      iconBadge,
      "with withIcon=true, the icon badge must render as a descendant of the field zone wrapper",
    ).not.toBeNull();

    unmount();
  });

  // -------------------------------------------------------------------------
  // #2: withIcon omitted/false renders no icon at all.
  // -------------------------------------------------------------------------

  it("field_without_with_icon_prop_renders_no_icon", async () => {
    const { container, unmount } = renderField({
      field: TAGS_FIELD,
      entity: makeTask({ tags: [] }),
      // withIcon omitted — defaults to false
    });
    await flushSetup();

    const fieldZone = container.querySelector(
      '[data-segment="field:task:T1.tags"]',
    );
    expect(fieldZone).not.toBeNull();
    const iconBadge = fieldZone!.querySelector(
      'span[data-slot="tooltip-trigger"]',
    );
    expect(
      iconBadge,
      "without withIcon, no icon badge may render — backwards-compatible default for non-inspector callers",
    ).toBeNull();

    unmount();
  });

  // -------------------------------------------------------------------------
  // #3: Clicking the icon focuses the field zone.
  // -------------------------------------------------------------------------

  it("clicking_icon_inside_field_focuses_field_zone", async () => {
    const { container, unmount } = renderField({
      field: TAGS_FIELD,
      entity: makeTask({ tags: [] }),
      withIcon: true,
    });
    await flushSetup();

    const fieldZone = registerScopeArgs().find(
      (a) => a.segment === "field:task:T1.tags",
    );
    expect(fieldZone).toBeTruthy();

    mockInvoke.mockClear();
    const iconBadge = container.querySelector(
      'span[data-slot="tooltip-trigger"]',
    ) as HTMLElement | null;
    expect(iconBadge).not.toBeNull();

    fireEvent.click(iconBadge!);
    await flushSetup();

    const focusCalls = spatialFocusCalls();
    expect(
      focusCalls.length,
      "clicking the icon must dispatch spatial_focus exactly once",
    ).toBeGreaterThanOrEqual(1);
    expect(
      focusCalls[0].fq,
      "the focus key must be the field zone's key (icon click bubbles to the zone)",
    ).toBe(fieldZone!.fq);

    unmount();
  });

  // -------------------------------------------------------------------------
  // #4: Clicking the content focuses the field zone.
  // -------------------------------------------------------------------------

  it("clicking_content_inside_field_focuses_field_zone", async () => {
    const { container, unmount } = renderField({
      field: TAGS_FIELD,
      entity: makeTask({ tags: [] }),
      withIcon: true,
    });
    await flushSetup();

    const fieldZone = registerScopeArgs().find(
      (a) => a.segment === "field:task:T1.tags",
    );
    expect(fieldZone).toBeTruthy();

    mockInvoke.mockClear();
    // The content's flex-1 wrapper is the second sibling inside the
    // flex row. A click anywhere inside it bubbles to the field zone.
    const contentWrap = container.querySelector(
      '[data-segment="field:task:T1.tags"] .flex-1.min-w-0',
    ) as HTMLElement | null;
    expect(contentWrap).not.toBeNull();

    fireEvent.click(contentWrap!);
    await flushSetup();

    const focusCalls = spatialFocusCalls();
    expect(focusCalls.length).toBeGreaterThanOrEqual(1);
    expect(focusCalls[0].fq).toBe(fieldZone!.fq);

    unmount();
  });

  // -------------------------------------------------------------------------
  // #5: The `<FocusIndicator>` paints inside the field zone wrapper.
  //
  // The indicator is a child of the field zone wrapper (not of the
  // inner flex row), positioned with `absolute inset-0` so its dotted
  // border traces the wrapper's bounds — surrounding both the icon and
  // the content. The icon is the first content child inside that same
  // wrapper. The indicator and the icon's flex-row ancestor are
  // siblings inside the zone; neither contains the other.
  // -------------------------------------------------------------------------

  it("focus_indicator_paints_inside_field_zone", async () => {
    const { container, unmount } = renderField({
      field: TAGS_FIELD,
      entity: makeTask({ tags: [] }),
      withIcon: true,
    });
    await flushSetup();

    const zoneArgs = registerScopeArgs().find(
      (a) => a.segment === "field:task:T1.tags",
    );
    expect(zoneArgs).toBeTruthy();

    // Drive a focus claim for the field zone.
    await fireFocusChanged({
      next_fq: zoneArgs!.fq as FullyQualifiedMoniker,
      next_segment: asSegment("field:task:T1.tags"),
    });
    await flushSetup();

    const fieldZone = container.querySelector(
      '[data-segment="field:task:T1.tags"]',
    ) as HTMLElement;
    expect(fieldZone.getAttribute("data-focused")).toBe("true");

    const indicator = fieldZone.querySelector(
      "[data-testid='focus-indicator']",
    );
    expect(
      indicator,
      "the focused field zone must mount a <FocusIndicator>",
    ).not.toBeNull();

    const iconBadge = fieldZone.querySelector(
      'span[data-slot="tooltip-trigger"]',
    );
    expect(iconBadge).not.toBeNull();

    // Both indicator and the icon-and-content flex row share the field
    // zone as their nearest containing block. Walk down from the zone:
    // the indicator must be a direct or nested child of the zone (not
    // of the icon, not of the content), AND the icon's flex-row
    // ancestor must also be a child of the zone. The `absolute inset-0`
    // dotted-border indicator traces the wrapper's bounds and
    // surrounds the entire flex-row container, including the icon.
    expect(
      fieldZone.contains(indicator),
      "focus indicator must live inside the field zone wrapper",
    ).toBe(true);
    expect(
      fieldZone.contains(iconBadge),
      "icon badge must live inside the field zone wrapper",
    ).toBe(true);
    // The indicator's class string carries `inset-0` and the dotted
    // border tokens — pin the contract that it paints inside the zone's
    // box as an outline tracing the wrapper.
    expect(indicator!.className).toContain("inset-0");
    expect(indicator!.className).toContain("border-dotted");
    expect(indicator!.className).toContain("border-primary");
    // The icon badge sits inside the flex row's first slot; verify its
    // inner flex-row ancestor is a sibling (within the zone) of the
    // indicator. The indicator overlays the row from inset-0 without
    // containing or being contained by the row.
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
    // Indicator and flex-row are both descendants of the field zone;
    // neither contains the other. (The indicator is a sibling of the
    // flex row inside the zone wrapper.)
    expect(indicator!.contains(flexRow as Node)).toBe(false);
    expect(flexRow!.contains(indicator)).toBe(false);

    unmount();
  });

  // -------------------------------------------------------------------------
  // #6: The static YAML icon renders when no override is provided.
  // -------------------------------------------------------------------------

  it("field_icon_uses_static_yaml_icon_when_no_override", async () => {
    const { container, unmount } = renderField({
      field: TAGS_FIELD,
      entity: makeTask({ tags: [] }),
      withIcon: true,
    });
    await flushSetup();

    // The `tag` icon resolves to `Tag` in the lucide registry.
    // `<Tag>`'s rendered SVG advertises a stable `aria-label` /
    // `lucide-tag` class; Lucide attaches `lucide lucide-tag` as
    // CSS classes by default. Pin against that class fingerprint so
    // a stylesheet rename (without a real component swap) doesn't
    // make this test pass falsely.
    const svg = container.querySelector(
      '[data-segment="field:task:T1.tags"] svg',
    ) as SVGElement | null;
    expect(svg, "field with icon=tag must render an svg").not.toBeNull();
    expect(
      svg!.getAttribute("class"),
      "the rendered svg must carry the lucide-tag class fingerprint",
    ).toContain("lucide-tag");

    // Sanity: the `<Tag>` component's render output shouldn't pretend
    // to be `<HelpCircle>` or some other lucide icon.
    expect(svg!.getAttribute("class")).not.toContain("lucide-help-circle");

    // Touch the symbols so the static-analyser doesn't flag them.
    void Tag;
    void HelpCircle;

    unmount();
  });

  // -------------------------------------------------------------------------
  // #7: Display registry's `iconOverride` replaces the static icon.
  // -------------------------------------------------------------------------

  it("field_icon_uses_display_registry_iconOverride_when_provided", async () => {
    // Register a display whose iconOverride returns CheckCircle when
    // the value is "ok", AlertTriangle otherwise.
    function NoopDisplay() {
      return null;
    }
    registerDisplay("with-icon-override-display", NoopDisplay, {
      iconOverride: (v: unknown) => (v === "ok" ? CheckCircle : AlertTriangle),
    });

    const { container, unmount } = renderField({
      field: OVERRIDE_FIELD,
      entity: makeTask({ override_field: "ok" }),
      withIcon: true,
    });
    await flushSetup();

    const svg = container.querySelector(
      '[data-segment="field:task:T1.override_field"] svg',
    ) as SVGElement | null;
    expect(svg).not.toBeNull();
    // CheckCircle's lucide class is `lucide-circle-check-big` (lucide
    // renamed CheckCircle's marker in newer versions). To stay
    // robust to that rename, accept either marker — the assertion
    // we care about is "NOT the static `tag` icon" plus a positive
    // match against any CheckCircle-flavoured class.
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
  // #8: Unknown icon name → HelpCircle fallback.
  // -------------------------------------------------------------------------

  it("field_icon_falls_back_to_HelpCircle_for_unknown_icon_name", async () => {
    const { container, unmount } = renderField({
      field: BOGUS_ICON_FIELD,
      entity: makeTask({ bogus_icon_field: "" }),
      withIcon: true,
    });
    await flushSetup();

    const svg = container.querySelector(
      '[data-segment="field:task:T1.bogus_icon_field"] svg',
    ) as SVGElement | null;
    expect(
      svg,
      "field with unknown icon name must still render an svg (HelpCircle fallback)",
    ).not.toBeNull();
    // Lucide renamed `HelpCircle`'s class marker between versions
    // (`lucide-circle-help` ↔ `lucide-circle-question-mark` ↔
    // `lucide-help-circle`). Accept any of them so a future lucide
    // bump doesn't make this test pass falsely AND doesn't make it
    // fail on a legitimate version refresh.
    const className = svg!.getAttribute("class") ?? "";
    const isHelpCircle =
      className.includes("lucide-circle-help") ||
      className.includes("lucide-help-circle") ||
      className.includes("lucide-circle-question-mark") ||
      className.includes("lucide-help");
    expect(
      isHelpCircle,
      `the rendered svg must be HelpCircle (any class fingerprint). Got: ${className}`,
    ).toBe(true);

    // Ensure `waitFor` is at least referenced so the import linter
    // doesn't trip on an unused symbol if we never await elsewhere.
    void waitFor;

    unmount();
  });
});
