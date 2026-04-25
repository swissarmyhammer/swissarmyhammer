import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve("ok")),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock("@tauri-apps/plugin-log", () => ({
  error: vi.fn(),
  warn: vi.fn(),
  info: vi.fn(),
  debug: vi.fn(),
  trace: vi.fn(),
  attachConsole: vi.fn(() => Promise.resolve()),
}));

import { AvatarDisplay } from "./avatar-display";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";

import { TooltipProvider } from "@/components/ui/tooltip";
import type { Entity, FieldDef } from "@/types/kanban";

/** Default field def used when a test does not supply its own. */
const DEFAULT_FIELD: FieldDef = {
  id: "f-assignees",
  name: "assignees",
  type: { kind: "reference", entity: "actor", multiple: true },
  display: "avatar",
} as unknown as FieldDef;

/** Default entity used when a test does not supply its own. */
const DEFAULT_ENTITY: Entity = {
  entity_type: "task",
  id: "task-1",
  moniker: "task:task-1",
  fields: {},
};

/**
 * Wrap AvatarDisplay in required providers.
 *
 * Accepts an optional `field` and `mode` so placeholder/styling behavior
 * can be exercised. Defaults match the plain "render this list of actor
 * IDs in compact mode" call-site that the original tests covered.
 */
function renderDisplay(
  value: unknown,
  actors: Entity[] = [],
  options: {
    field?: FieldDef;
    mode?: "compact" | "full";
    entity?: Entity;
  } = {},
) {
  const field = options.field ?? DEFAULT_FIELD;
  const mode = options.mode ?? "compact";
  const entity = options.entity ?? DEFAULT_ENTITY;
  return render(
    <TooltipProvider>
      <SchemaProvider>
        <EntityStoreProvider entities={{ actor: actors }}>
          <EntityFocusProvider>
            <AvatarDisplay
              field={field}
              value={value}
              entity={entity}
              mode={mode}
            />
          </EntityFocusProvider>
        </EntityStoreProvider>
      </SchemaProvider>
    </TooltipProvider>,
  );
}

function makeActor(
  id: string,
  name: string,
  overrides: Record<string, unknown> = {},
): Entity {
  return {
    entity_type: "actor",
    id,
    moniker: `actor:${id}`,
    fields: { name, ...overrides },
  };
}

const DATA_URI = "data:image/svg+xml;base64,PHN2Zz48L3N2Zz4=";

describe("AvatarDisplay", () => {
  it("renders dash for empty array when no placeholder is set", () => {
    renderDisplay([]);
    expect(screen.getByText("-")).toBeTruthy();
  });

  it("renders dash for null/undefined when no placeholder is set", () => {
    renderDisplay(null);
    expect(screen.getByText("-")).toBeTruthy();
  });

  it("renders Avatar components for array of actor IDs", () => {
    const actors = [makeActor("alice", "Alice Smith")];
    renderDisplay(["alice"], actors);
    // Should render initials for Alice
    expect(screen.getByText("AS")).toBeTruthy();
  });

  it("renders an image directly for a string URL value", () => {
    const { container } = renderDisplay(DATA_URI);
    const img = container.querySelector("img");
    expect(img).not.toBeNull();
    expect(img!.src).toBe(DATA_URI);
  });

  it("renders an image for an https URL string", () => {
    const url = "https://example.com/avatar.png";
    const { container } = renderDisplay(url);
    const img = container.querySelector("img");
    expect(img).not.toBeNull();
    expect(img!.src).toBe(url);
  });

  it("renders dash for empty string when no placeholder is set", () => {
    renderDisplay("");
    expect(screen.getByText("-")).toBeTruthy();
  });

  it("image is rendered as a circle (rounded-full)", () => {
    const { container } = renderDisplay(DATA_URI);
    const img = container.querySelector("img")!;
    expect(img.className).toContain("rounded-full");
  });

  // ---------------------------------------------------------------------
  // Placeholder behavior — mirrors BadgeListDisplay/BadgeDisplay convention.
  // ---------------------------------------------------------------------

  it("renders the configured placeholder in compact mode when value is empty", () => {
    const fieldWithPlaceholder: FieldDef = {
      ...DEFAULT_FIELD,
      placeholder: "Assign",
    } as unknown as FieldDef;
    const { container } = renderDisplay([], [], {
      field: fieldWithPlaceholder,
      mode: "compact",
    });
    const hint = container.querySelector("span.text-muted-foreground\\/50");
    expect(hint).toBeTruthy();
    // Compact mode keeps the muted styling, but swaps the "-" fallback
    // for the YAML-configured placeholder string.
    expect(hint?.textContent).toBe("Assign");
  });

  it("renders the configured placeholder in full mode with italic styling when value is empty", () => {
    const fieldWithPlaceholder: FieldDef = {
      ...DEFAULT_FIELD,
      placeholder: "Assign",
    } as unknown as FieldDef;
    const { container } = renderDisplay([], [], {
      field: fieldWithPlaceholder,
      mode: "full",
    });
    const hint = container.querySelector("span.italic");
    expect(hint).toBeTruthy();
    expect(hint?.textContent).toBe("Assign");
  });

  it("renders italic 'None' fallback in full mode when no placeholder is set", () => {
    const { container } = renderDisplay([], [], { mode: "full" });
    const hint = container.querySelector("span.italic");
    expect(hint).toBeTruthy();
    expect(hint?.textContent).toBe("None");
  });

  it("falls back to dash in compact mode when no placeholder is set (regression guard)", () => {
    const { container } = renderDisplay([], [], { mode: "compact" });
    const hint = container.querySelector("span.text-muted-foreground\\/50");
    expect(hint).toBeTruthy();
    expect(hint?.textContent).toBe("-");
  });

  // ---------------------------------------------------------------------
  // Compact-mode height normalization — populated and empty variants
  // must share the same outer wrapper so the row virtualizer's fixed
  // ROW_HEIGHT estimate (`data-table.tsx::ROW_HEIGHT`) holds.
  //
  // Tailwind utilities are not bundled into the vitest browser project
  // (see `data-table.virtualized.test.tsx` header), so we cannot assert
  // pixel heights — instead we assert the structural invariant: both
  // branches emit a `data-compact-cell="true"` wrapper with identical
  // class names. The class encodes the `h-6 flex items-center` height
  // contract; the data-attribute is the public hook other tests (and the
  // height-equality test in `data-table.virtualized.test.tsx`) use.
  // ---------------------------------------------------------------------

  it("compact-mode populated and empty variants share the CompactCellWrapper contract", () => {
    const actors = [makeActor("alice", "Alice Smith")];

    const populated = renderDisplay(["alice"], actors, { mode: "compact" });
    const populatedWrapper = populated.container.querySelector(
      "[data-compact-cell='true']",
    );
    expect(populatedWrapper).toBeTruthy();
    const populatedClassName = populatedWrapper!.className;
    populated.unmount();

    const empty = renderDisplay([], [], { mode: "compact" });
    const emptyWrapper = empty.container.querySelector(
      "[data-compact-cell='true']",
    );
    expect(emptyWrapper).toBeTruthy();
    expect(emptyWrapper!.className).toBe(populatedClassName);
    empty.unmount();
  });

  it("full-mode does not wrap output in the compact-cell wrapper", () => {
    const actors = [makeActor("alice", "Alice Smith")];
    const { container } = renderDisplay(["alice"], actors, { mode: "full" });
    expect(container.querySelector("[data-compact-cell='true']")).toBeNull();
  });

  it("renders Avatar at sm size in compact mode so it fits in the normalized row", () => {
    const actors = [makeActor("alice", "Alice Smith")];
    const { container } = renderDisplay(["alice"], actors, { mode: "compact" });
    // Avatar size="sm" is "w-5 h-5 ..." per components/avatar.tsx; size="md"
    // is "w-7 h-7 ...". Asserting on the className proves the compact
    // path picked the smaller variant.
    const initialsSpan = container.querySelector(
      "span[aria-label='Alice Smith']",
    );
    expect(initialsSpan).toBeTruthy();
    expect(initialsSpan!.className).toContain("w-5");
    expect(initialsSpan!.className).toContain("h-5");
  });
});
