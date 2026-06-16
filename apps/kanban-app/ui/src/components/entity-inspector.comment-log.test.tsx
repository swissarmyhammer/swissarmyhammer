/**
 * Integration: the `comments` field renders in the task inspector purely
 * through the field registry (`display: comment-log` / `editor:
 * comment-log` from the YAML schema — no comment-specific branching in
 * `EntityInspector`), and the full field-set round-trip works:
 *
 *   editor emits wire array via onChange
 *     → Field autosave → dispatch_command("entity.update_field")
 *     → server normalization (mirrored here, matching
 *       `comment/normalize.rs`: mint id/timestamp/author for new
 *       members, text-only edits, explicit tombstone deletes)
 *     → store update → UI shows the resolved author + timestamp.
 *
 * Modeled on `entity-inspector.test.tsx` (kernel simulator + mocked
 * Tauri IPC boundary, real providers/Field/registry in between).
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";

// Hoisted mocks: capture invoke and listen so the kernel simulator can drive
// `focus-changed` events through the production spatial-focus bridge.
type ListenCallback = (event: { payload: unknown }) => void;
const { mockInvoke, mockListen, listeners } = vi.hoisted(() => {
  const listeners = new Map<string, ListenCallback[]>();
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const mockInvoke = vi.fn(
    async (..._args: any[]): Promise<unknown> => undefined,
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

// ---------------------------------------------------------------------------
// Schema — task with a `comments` field mirroring the builtin YAML
// (`builtin/definitions/comments.yaml` + the `log` section in `task.yaml`)
// ---------------------------------------------------------------------------

const TASK_SCHEMA = {
  entity: {
    name: "task",
    body_field: "body",
    fields: ["title", "comments"],
    sections: [
      { id: "header", on_card: true },
      { id: "log", label: "Log" },
    ],
  },
  fields: [
    {
      id: "f1",
      name: "title",
      type: { kind: "markdown", single_line: true },
      editor: "markdown",
      display: "text",
      section: "header",
    },
    {
      id: "f-comments",
      name: "comments",
      description: "Conversation log",
      type: { kind: "comment-log" },
      icon: "message-square",
      editor: "comment-log",
      display: "comment-log",
      section: "log",
    },
  ],
};

const ACTOR_SCHEMA = {
  entity: {
    name: "actor",
    fields: ["name", "color"],
    mention_prefix: "@",
    mention_display_field: "name",
  },
  fields: [
    {
      id: "a1",
      name: "name",
      type: { kind: "text", single_line: true },
      editor: "markdown",
      display: "text",
      section: "header",
    },
  ],
};

const SCHEMAS: Record<string, unknown> = {
  task: TASK_SCHEMA,
  actor: ACTOR_SCHEMA,
};

// ---------------------------------------------------------------------------
// Server mirror — the comment-log normalization semantics of
// `comment/normalize.rs`, applied to a per-test stored log
// ---------------------------------------------------------------------------

interface StoredComment {
  id: string;
  actor: string;
  text: string;
  timestamp: string;
}

/** The stored log "on disk" — reset per test. */
let storedComments: StoredComment[] = [];
let mintCounter = 0;

/** Mint a deterministic 26-char id that sorts after the seeded ids. */
function mintId(): string {
  mintCounter += 1;
  return `01zzzzzzzzzzzzzzzzzzzzz${String(mintCounter).padStart(3, "0")}`;
}

const SERVER_TIMESTAMP = "2026-06-12T12:00:00+00:00";
const SERVER_AUTHOR = "alice";

/**
 * Apply the server merge to `storedComments`: tombstones delete, known
 * ids are text-only edits (actor/timestamp immutable), everything else
 * is new (minted id + server author/timestamp), absence preserves,
 * result sorted by id ascending.
 */
function applyServerNormalize(incoming: unknown[]): void {
  const byId = new Map(storedComments.map((m) => [m.id, m]));
  for (const raw of incoming) {
    const member = raw as Record<string, unknown>;
    const id = typeof member.id === "string" ? member.id : undefined;
    if (member.deleted === true) {
      if (id) byId.delete(id);
      continue;
    }
    const existing = id ? byId.get(id) : undefined;
    if (existing) {
      existing.text = String(member.text ?? "");
    } else {
      const newId = mintId();
      byId.set(newId, {
        id: newId,
        actor: SERVER_AUTHOR,
        text: String(member.text ?? ""),
        timestamp: SERVER_TIMESTAMP,
      });
    }
  }
  storedComments = [...byId.values()].sort((a, b) =>
    a.id < b.id ? -1 : a.id > b.id ? 1 : 0,
  );
}

// Fallback handler for non-spatial IPCs. The kernel simulator routes
// spatial_* commands through itself; everything else falls through to here.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const fallbackInvoke = async (cmd: string, args?: any): Promise<unknown> => {
  if (cmd === "list_entity_types") return ["task", "actor"];
  if (cmd === "get_entity_schema") {
    const entityType = args?.entityType as string;
    return SCHEMAS[entityType] ?? TASK_SCHEMA;
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
  if (
    cmd === "dispatch_command" &&
    args?.cmd === "entity.update_field" &&
    args?.args?.field_name === "comments"
  ) {
    applyServerNormalize(args.args.value as unknown[]);
    return { ok: true };
  }
  return "ok";
};

vi.mock("@tauri-apps/api/core", async () => {
  const actual = await vi.importActual<typeof import("@tauri-apps/api/core")>(
    "@tauri-apps/api/core",
  );
  return {
    ...actual,
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    invoke: (...args: any[]) => mockInvoke(...args),
  };
});
vi.mock("@tauri-apps/api/event", async () => {
  const actual = await vi.importActual<typeof import("@tauri-apps/api/event")>(
    "@tauri-apps/api/event",
  );
  return {
    ...actual,
    listen: (...a: Parameters<typeof mockListen>) => mockListen(...a),
  };
});
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

// The registry wiring IS the system under test: the inspector must pick
// up the comment-log display/editor purely from these registrations.
import "@/components/fields/registrations";
import { EntityInspector } from "./entity-inspector";
import { UIStateProvider } from "@/lib/ui-state-context";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { FieldUpdateProvider } from "@/lib/field-update-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import { CommandScopeProvider } from "@/lib/command-scope";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { FocusLayer } from "@/components/focus-layer";
import { asSegment } from "@/types/spatial";
import type { Entity } from "@/types/kanban";
import { installKernelSimulator } from "@/test-helpers/kernel-simulator";

beforeEach(() => {
  listeners.clear();
  mockInvoke.mockReset();
  mockListen.mockClear();
  installKernelSimulator(mockInvoke, listeners, fallbackInvoke);
  storedComments = [];
  mintCounter = 0;
});

const ALICE: Entity = {
  entity_type: "actor",
  id: "alice",
  moniker: "actor:alice",
  fields: { name: "Alice Smith" },
};

function makeTask(): Entity {
  return {
    entity_type: "task",
    id: "test-id",
    moniker: "task:test-id",
    fields: { title: "Test Task", comments: structuredClone(storedComments) },
  };
}

function buildTree(task: Entity) {
  return (
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <TooltipProvider>
          <SchemaProvider>
            <EntityStoreProvider entities={{ task: [task], actor: [ALICE] }}>
              <EntityFocusProvider>
                <FieldUpdateProvider>
                  <UIStateProvider>
                    <CommandScopeProvider commands={[]}>
                      <EntityInspector entity={task} />
                    </CommandScopeProvider>
                  </UIStateProvider>
                </FieldUpdateProvider>
              </EntityFocusProvider>
            </EntityStoreProvider>
          </SchemaProvider>
        </TooltipProvider>
      </FocusLayer>
    </SpatialFocusProvider>
  );
}

async function renderInspector() {
  const result = render(buildTree(makeTask()));
  // Wait for async schema load
  await act(async () => {
    await new Promise((r) => setTimeout(r, 50));
  });
  return result;
}

/** Re-render with a fresh task entity reflecting the stored log. */
async function refreshFromStore(result: ReturnType<typeof render>) {
  await act(async () => {
    result.rerender(buildTree(makeTask()));
  });
}

/** Click the comments field display to enter edit mode. */
async function openCommentsEditor(container: HTMLElement) {
  const row = container.querySelector('[data-testid="field-row-comments"]');
  expect(row).toBeTruthy();
  const clickTarget = row!.querySelector(".cursor-text");
  expect(clickTarget).toBeTruthy();
  await act(async () => {
    fireEvent.click(clickTarget!);
  });
}

/** Wait out the Field autosave debounce so the pending save dispatches. */
async function flushAutosave() {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 1150));
  });
}

/** All entity.update_field dispatches for the comments field. */
function commentUpdateCalls() {
  return mockInvoke.mock.calls.filter(
    (c) =>
      c[0] === "dispatch_command" &&
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (c[1] as any)?.cmd === "entity.update_field" &&
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (c[1] as any)?.args?.field_name === "comments",
  );
}

describe("EntityInspector — comment-log field via the registry", () => {
  it("renders the comments field in the Log section through the registry", async () => {
    const { container } = await renderInspector();

    expect(
      container.querySelector('[data-testid="inspector-section-label-log"]'),
    ).toBeTruthy();
    const row = container.querySelector('[data-testid="field-row-comments"]');
    expect(row).toBeTruthy();
    // Empty log renders the display's empty state inside the row.
    expect(row!.textContent).toContain("None");
  });

  it("add: composing a comment round-trips to a resolved author and timestamp", async () => {
    const result = await renderInspector();
    const { container } = result;

    await openCommentsEditor(container);
    const compose = screen.getByPlaceholderText(/add a comment/i);
    await act(async () => {
      fireEvent.change(compose, { target: { value: "Hello from the inspector" } });
    });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /^comment$/i }));
    });

    await flushAutosave();

    // The editor sent ONLY the text for the new member — no id, no
    // actor, no timestamp (the server owns those).
    const calls = commentUpdateCalls();
    expect(calls.length).toBe(1);
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const args = (calls[0][1] as any).args;
    expect(args.entity_type).toBe("task");
    expect(args.id).toBe("test-id");
    expect(args.value).toEqual([{ text: "Hello from the inspector" }]);

    // Field-change round-trip: the store now carries the normalized
    // member; the UI shows the server-resolved author + timestamp.
    expect(storedComments.length).toBe(1);
    await refreshFromStore(result);

    const item = container.querySelector(
      `[data-comment-id="${storedComments[0].id}"]`,
    );
    expect(item).toBeTruthy();
    expect(item!.textContent).toContain("Hello from the inspector");
    expect(item!.textContent).toContain("Alice Smith");
    expect(item!.querySelector(`[title="${SERVER_TIMESTAMP}"]`)).toBeTruthy();
  });

  it("edit: changing a member's text retains its id and immutable author/timestamp", async () => {
    storedComments = [
      {
        id: "01aaaaaaaaaaaaaaaaaaaaaaaaaa",
        actor: "alice",
        text: "Original text",
        timestamp: "2026-01-01T00:00:00+00:00",
      },
    ];
    const result = await renderInspector();
    const { container } = result;

    await openCommentsEditor(container);
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /edit comment/i }));
    });
    const textarea = screen.getByDisplayValue("Original text");
    await act(async () => {
      fireEvent.change(textarea, { target: { value: "Edited text" } });
    });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /^save$/i }));
    });

    await flushAutosave();

    const calls = commentUpdateCalls();
    expect(calls.length).toBe(1);
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const sent = (calls[0][1] as any).args.value as Array<
      Record<string, unknown>
    >;
    expect(sent.length).toBe(1);
    expect(sent[0].id).toBe("01aaaaaaaaaaaaaaaaaaaaaaaaaa");
    expect(sent[0].text).toBe("Edited text");

    await refreshFromStore(result);
    const item = container.querySelector(
      '[data-comment-id="01aaaaaaaaaaaaaaaaaaaaaaaaaa"]',
    );
    expect(item).toBeTruthy();
    expect(item!.textContent).toContain("Edited text");
    // Author and timestamp survived the round-trip unchanged.
    expect(item!.textContent).toContain("Alice Smith");
    expect(
      item!.querySelector('[title="2026-01-01T00:00:00+00:00"]'),
    ).toBeTruthy();
  });

  it("delete: the member is replaced by a tombstone — never deleted by omission", async () => {
    storedComments = [
      {
        id: "01aaaaaaaaaaaaaaaaaaaaaaaaaa",
        actor: "alice",
        text: "Delete me",
        timestamp: "2026-01-01T00:00:00+00:00",
      },
      {
        id: "01bbbbbbbbbbbbbbbbbbbbbbbbbb",
        actor: "alice",
        text: "Keep me",
        timestamp: "2026-01-02T00:00:00+00:00",
      },
    ];
    const result = await renderInspector();
    const { container } = result;

    await openCommentsEditor(container);
    const deleteButtons = screen.getAllByRole("button", {
      name: /delete comment/i,
    });
    await act(async () => {
      fireEvent.click(deleteButtons[0]);
    });

    await flushAutosave();

    const calls = commentUpdateCalls();
    expect(calls.length).toBe(1);
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const sent = (calls[0][1] as any).args.value as Array<
      Record<string, unknown>
    >;
    // The wire array still has BOTH entries: the tombstone in place of
    // the deleted member, and the surviving member untouched.
    expect(sent.length).toBe(2);
    expect(sent[0]).toEqual({
      id: "01aaaaaaaaaaaaaaaaaaaaaaaaaa",
      deleted: true,
    });
    expect(sent[1]).toMatchObject({ id: "01bbbbbbbbbbbbbbbbbbbbbbbbbb" });

    await refreshFromStore(result);
    expect(storedComments.length).toBe(1);
    expect(container.textContent).not.toContain("Delete me");
    expect(container.textContent).toContain("Keep me");
  });
});
