import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, act } from "@testing-library/react";
import type { MentionableType } from "@/lib/schema-context";
import type { Entity } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Mock data — tag entity with name "bug"
// ---------------------------------------------------------------------------

const mockTag: Entity = {
  id: "tag-bug",
  entity_type: "tag",
  moniker: "tag:tag-bug",
  fields: { tag_name: "bug", color: "ff0000" },
};

const MENTIONABLE_TYPES: MentionableType[] = [
  { entityType: "tag", prefix: "#", displayField: "tag_name" },
];

// ---------------------------------------------------------------------------
// Mocks — Tauri, schema, entity store, entity-commands
// ---------------------------------------------------------------------------

const mockGetEntities = vi.fn((_type: string) => [mockTag]);

// Preserve the real Tauri core exports (SERIALIZE_TO_IPC_FN, Resource, Channel,
// TauriEvent, etc.) so that transitively-imported submodules like `window.js`
// and `dpi.js` can resolve their re-exports. Only override `invoke`.
vi.mock("@tauri-apps/api/core", async () => {
  const actual = await vi.importActual<typeof import("@tauri-apps/api/core")>(
    "@tauri-apps/api/core",
  );
  return {
    ...actual,
    invoke: vi.fn(() => Promise.resolve("ok")),
  };
});
vi.mock("@tauri-apps/api/event", async () => {
  const actual = await vi.importActual<typeof import("@tauri-apps/api/event")>(
    "@tauri-apps/api/event",
  );
  return {
    ...actual,
    listen: vi.fn(() => Promise.resolve(() => {})),
  };
});
vi.mock("@tauri-apps/plugin-log", () => ({
  error: vi.fn(),
  warn: vi.fn(),
  info: vi.fn(),
  debug: vi.fn(),
  trace: vi.fn(),
  attachConsole: vi.fn(() => Promise.resolve()),
}));

vi.mock("@/lib/schema-context", () => ({
  useSchema: () => ({
    getSchema: () => undefined,
    getFieldDef: () => undefined,
    getEntityCommands: () => [],
    mentionableTypes: MENTIONABLE_TYPES,
    loading: false,
  }),
  useSchemaOptional: () => ({
    getSchema: () => undefined,
    getFieldDef: () => undefined,
    getEntityCommands: () => [],
  }),
}));

vi.mock("@/lib/entity-store-context", () => ({
  useEntityStore: () => ({ getEntities: mockGetEntities, getEntity: vi.fn() }),
}));

// `useMentionExtensions` reads virtual-tag metadata via `useBoardData`;
// tests that don't exercise virtual tags can return `null`.
vi.mock("@/components/window-container", () => ({
  useBoardData: () => null,
}));

// ---------------------------------------------------------------------------

import { MarkdownDisplay } from "@/components/fields/displays/markdown-display";
import { EntityFocusProvider } from "@/lib/entity-focus-context";

import { TooltipProvider } from "@/components/ui/tooltip";
import type { DisplayProps } from "@/components/fields/displays/text-display";

function makeProps(
  value: unknown,
  mode: "compact" | "full" = "full",
  onCommit?: (value: unknown) => void,
) {
  return {
    field: {
      id: "f1",
      name: "body",
      type: { kind: "text" },
    } as DisplayProps["field"],
    value,
    entity: { entity_type: "task", id: "t1", moniker: "task:t1", fields: {} },
    mode,
    onCommit,
  };
}

/** Render with all required context providers. */
function renderMarkdown(value: string) {
  return render(
    <TooltipProvider>
      <EntityFocusProvider>
        <MarkdownDisplay {...makeProps(value, "full")} />
      </EntityFocusProvider>
    </TooltipProvider>,
  );
}

/** Flush microtasks and CM6 mount effects. */
async function flush() {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 0));
  });
}

describe("multiline display with mention types", () => {
  beforeEach(() => {
    mockGetEntities.mockReturnValue([mockTag]);
  });

  it("renders tag pills in display mode with mentions loaded", async () => {
    const { container } = renderMarkdown("Fix the #bug in login");
    await flush();

    // MarkdownDisplay now mounts a CM6 read-only viewer; the mention
    // decoration extension replaces `#bug` with a widget carrying the
    // `cm-mention-pill` class whose textContent is `#bug`.
    const pill = container.querySelector(".cm-mention-pill");
    expect(pill).not.toBeNull();
    expect(pill?.textContent).toBe("#bug");
  });
});
