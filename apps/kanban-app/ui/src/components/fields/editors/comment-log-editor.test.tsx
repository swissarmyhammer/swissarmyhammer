import { describe, it, expect, vi } from "vitest";
import { screen, fireEvent, act } from "@testing-library/react";
import { renderInAct, rerenderInAct } from "@/test/act-render";

// ---------------------------------------------------------------------------
// Mocks — must be declared before importing the component under test
// ---------------------------------------------------------------------------

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const mockInvoke = vi.fn((..._args: any[]) => Promise.resolve("ok"));
// Spread the real module and override only the parts the test controls.
// @tauri-apps/api >=2.11 pulls submodules that import named exports from core
// (SERIALIZE_TO_IPC_FN, Resource, Channel, …); a hand-listed stub drops them
// and breaks module loading.
vi.mock("@tauri-apps/api/core", async (importActual) => ({
  ...(await importActual<typeof import("@tauri-apps/api/core")>()),
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  invoke: (...args: any[]) => mockInvoke(...args),
}));
vi.mock("@tauri-apps/api/event", async (importActual) => ({
  ...(await importActual<typeof import("@tauri-apps/api/event")>()),
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

import { CommentLogEditor } from "./comment-log-editor";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { FocusLayer } from "@/components/focus-layer";
import { TooltipProvider } from "@/components/ui/tooltip";
import { asSegment } from "@/types/spatial";
import type { Entity, FieldDef } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/** Mirrors `builtin/definitions/comments.yaml`. */
const COMMENTS_FIELD: FieldDef = {
  id: "f-comments",
  name: "comments",
  description: "Conversation log",
  type: { kind: "comment-log" },
  icon: "message-square",
  editor: "comment-log",
  display: "comment-log",
  section: "log",
};

const COMMENT_A = {
  id: "01aaaaaaaaaaaaaaaaaaaaaaaaaa",
  actor: "alice",
  text: "First comment",
  timestamp: "2026-01-01T00:00:00+00:00",
};

const COMMENT_B = {
  id: "01bbbbbbbbbbbbbbbbbbbbbbbbbb",
  actor: "bob",
  text: "Second comment",
  timestamp: "2026-01-02T00:00:00+00:00",
};

/** A concurrent agent append — the design's stated external-update case. */
const COMMENT_C = {
  id: "01cccccccccccccccccccccccccc",
  actor: "bob",
  text: "Agent appended",
  timestamp: "2026-01-03T00:00:00+00:00",
};

const ACTORS: Entity[] = [
  {
    entity_type: "actor",
    id: "alice",
    moniker: "actor:alice",
    fields: { name: "Alice Smith" },
  },
  {
    entity_type: "actor",
    id: "bob",
    moniker: "actor:bob",
    fields: { name: "Bob Jones" },
  },
];

interface EditorTreeProps {
  value?: unknown;
  onCommit?: (val: unknown) => void;
  onCancel?: () => void;
  onChange?: (val: unknown) => void;
}

/**
 * Build the full provider tree around the editor. Extracted from
 * `renderEditor` so resync tests can `rerenderInAct` the same tree with
 * a fresh `value` prop (simulating an external field-change round-trip).
 */
function editorTree(props: EditorTreeProps) {
  return (
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <TooltipProvider>
          <SchemaProvider>
            <EntityStoreProvider entities={{ actor: ACTORS }}>
              <EntityFocusProvider>
                <CommentLogEditor
                  field={COMMENTS_FIELD}
                  value={props.value ?? []}
                  onCommit={props.onCommit ?? vi.fn()}
                  onCancel={props.onCancel ?? vi.fn()}
                  onChange={props.onChange}
                  mode="full"
                />
              </EntityFocusProvider>
            </EntityStoreProvider>
          </SchemaProvider>
        </TooltipProvider>
      </FocusLayer>
    </SpatialFocusProvider>
  );
}

async function renderEditor(props: EditorTreeProps) {
  return await renderInAct(editorTree(props));
}

/** Type into the compose box and click the submit button. */
async function composeAndSubmit(text: string) {
  const compose = screen.getByPlaceholderText(/add a comment/i);
  await act(async () => {
    fireEvent.change(compose, { target: { value: text } });
  });
  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name: /^comment$/i }));
  });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("CommentLogEditor — rendering", () => {
  it("renders existing member texts and resolved author names", async () => {
    await renderEditor({ value: [COMMENT_A, COMMENT_B] });
    expect(screen.getByText("First comment")).toBeTruthy();
    expect(screen.getByText("Second comment")).toBeTruthy();
    expect(screen.getByText("Alice Smith")).toBeTruthy();
    expect(screen.getByText("Bob Jones")).toBeTruthy();
  });

  it("renders an empty compose box and no members for an empty log", async () => {
    const { container } = await renderEditor({ value: [] });
    expect(screen.getByPlaceholderText(/add a comment/i)).toBeTruthy();
    expect(container.querySelectorAll("[data-comment-id]").length).toBe(0);
  });
});

describe("CommentLogEditor — add", () => {
  it("submit emits [...current, {text}] — the new member carries only text", async () => {
    const onChange = vi.fn();
    await renderEditor({ value: [COMMENT_A], onChange });

    await composeAndSubmit("Hello there");

    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange).toHaveBeenCalledWith([COMMENT_A, { text: "Hello there" }]);
  });

  it("empty submit is a no-op", async () => {
    const onChange = vi.fn();
    await renderEditor({ value: [COMMENT_A], onChange });

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /^comment$/i }));
    });

    expect(onChange).not.toHaveBeenCalled();
  });

  it("whitespace-only submit is a no-op", async () => {
    const onChange = vi.fn();
    await renderEditor({ value: [], onChange });

    await composeAndSubmit("   ");

    expect(onChange).not.toHaveBeenCalled();
  });

  it("clears the compose box after submit", async () => {
    await renderEditor({ value: [], onChange: vi.fn() });

    await composeAndSubmit("posted");

    const compose = screen.getByPlaceholderText(
      /add a comment/i,
    ) as HTMLTextAreaElement;
    expect(compose.value).toBe("");
  });

  it("Enter in the compose box submits", async () => {
    const onChange = vi.fn();
    await renderEditor({ value: [], onChange });

    const compose = screen.getByPlaceholderText(/add a comment/i);
    await act(async () => {
      fireEvent.change(compose, { target: { value: "via enter" } });
    });
    await act(async () => {
      fireEvent.keyDown(compose, { key: "Enter" });
    });

    expect(onChange).toHaveBeenCalledWith([{ text: "via enter" }]);
  });

  it("Shift+Enter in the compose box does NOT submit", async () => {
    const onChange = vi.fn();
    await renderEditor({ value: [], onChange });

    const compose = screen.getByPlaceholderText(/add a comment/i);
    await act(async () => {
      fireEvent.change(compose, { target: { value: "multi\nline" } });
    });
    await act(async () => {
      fireEvent.keyDown(compose, { key: "Enter", shiftKey: true });
    });

    expect(onChange).not.toHaveBeenCalled();
  });

  it("Enter during an IME composition does NOT submit", async () => {
    const onChange = vi.fn();
    await renderEditor({ value: [], onChange });

    const compose = screen.getByPlaceholderText(
      /add a comment/i,
    ) as HTMLTextAreaElement;
    await act(async () => {
      fireEvent.change(compose, { target: { value: "変換中" } });
    });
    // Enter confirming an IME composition carries isComposing — it must
    // not post the comment mid-composition (precedent: prompt-input.tsx).
    await act(async () => {
      fireEvent.keyDown(compose, { key: "Enter", isComposing: true });
    });

    expect(onChange).not.toHaveBeenCalled();
    expect(compose.value).toBe("変換中");
  });

  it("trims surrounding whitespace from the submitted text", async () => {
    const onChange = vi.fn();
    await renderEditor({ value: [], onChange });

    await composeAndSubmit("  padded  ");

    expect(onChange).toHaveBeenCalledWith([{ text: "padded" }]);
  });
});

describe("CommentLogEditor — edit", () => {
  it("editing a member emits the array with updated text and retained id", async () => {
    const onChange = vi.fn();
    await renderEditor({ value: [COMMENT_A, COMMENT_B], onChange });

    const editButtons = screen.getAllByRole("button", {
      name: /edit comment/i,
    });
    await act(async () => {
      fireEvent.click(editButtons[0]);
    });

    const textarea = screen.getByDisplayValue("First comment");
    await act(async () => {
      fireEvent.change(textarea, { target: { value: "Edited comment" } });
    });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /^save$/i }));
    });

    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange).toHaveBeenCalledWith([
      { ...COMMENT_A, text: "Edited comment" },
      COMMENT_B,
    ]);
  });

  it("clearing a member's text to whitespace emits nothing — empty edit is a cancel", async () => {
    const onChange = vi.fn();
    await renderEditor({ value: [COMMENT_A], onChange });

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /edit comment/i }));
    });
    const textarea = screen.getByDisplayValue("First comment");
    await act(async () => {
      fireEvent.change(textarea, { target: { value: "   " } });
    });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /^save$/i }));
    });

    // Consistent with the add path: an empty body is never persisted.
    expect(onChange).not.toHaveBeenCalled();
    expect(screen.getByText("First comment")).toBeTruthy();
  });

  it("cancelling a member edit emits nothing", async () => {
    const onChange = vi.fn();
    await renderEditor({ value: [COMMENT_A], onChange });

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /edit comment/i }));
    });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /^cancel$/i }));
    });

    expect(onChange).not.toHaveBeenCalled();
    expect(screen.getByText("First comment")).toBeTruthy();
  });
});

describe("CommentLogEditor — delete (tombstone)", () => {
  it("deleting emits the array with the member replaced by {id, deleted: true}", async () => {
    const onChange = vi.fn();
    await renderEditor({ value: [COMMENT_A, COMMENT_B], onChange });

    const deleteButtons = screen.getAllByRole("button", {
      name: /delete comment/i,
    });
    await act(async () => {
      fireEvent.click(deleteButtons[0]);
    });

    expect(onChange).toHaveBeenCalledTimes(1);
    // The tombstone replaces the member IN PLACE — the member is never
    // deleted by omission (absence means "preserve" on the server).
    const emitted = onChange.mock.calls[0][0] as unknown[];
    expect(emitted).toEqual([{ id: COMMENT_A.id, deleted: true }, COMMENT_B]);
  });

  it("the deleted member disappears from the editor view but stays in the wire array", async () => {
    const onChange = vi.fn();
    const { container } = await renderEditor({
      value: [COMMENT_A, COMMENT_B],
      onChange,
    });

    const deleteButtons = screen.getAllByRole("button", {
      name: /delete comment/i,
    });
    await act(async () => {
      fireEvent.click(deleteButtons[0]);
    });

    expect(screen.queryByText("First comment")).toBeNull();
    expect(screen.getByText("Second comment")).toBeTruthy();
    expect(container.querySelectorAll("[data-comment-id]").length).toBe(1);
  });
});

describe("CommentLogEditor — consecutive operations compose", () => {
  it("two deletes in a row emit an array carrying BOTH tombstones", async () => {
    const onChange = vi.fn();
    await renderEditor({ value: [COMMENT_A, COMMENT_B], onChange });

    const first = screen.getAllByRole("button", { name: /delete comment/i });
    await act(async () => {
      fireEvent.click(first[0]);
    });
    const second = screen.getAllByRole("button", { name: /delete comment/i });
    await act(async () => {
      fireEvent.click(second[0]);
    });

    expect(onChange).toHaveBeenCalledTimes(2);
    expect(onChange.mock.calls[1][0]).toEqual([
      { id: COMMENT_A.id, deleted: true },
      { id: COMMENT_B.id, deleted: true },
    ]);
  });

  it("an add after a delete keeps the tombstone in the emitted array", async () => {
    const onChange = vi.fn();
    await renderEditor({ value: [COMMENT_A], onChange });

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /delete comment/i }));
    });
    await composeAndSubmit("replacement");

    expect(onChange.mock.calls[1][0]).toEqual([
      { id: COMMENT_A.id, deleted: true },
      { text: "replacement" },
    ]);
  });
});

describe("CommentLogEditor — external resync rebases un-flushed local ops", () => {
  // These tests simulate the composite race from the review: a local op
  // sits in the 1s autosave debounce when an external field-change (an
  // agent append) delivers a fresh `value`. The resync must NOT
  // wholesale-reset the draft — a follow-up local op inside the window
  // re-emits FROM the draft and would otherwise permanently drop the
  // earlier, still-pending op.

  it("keeps a pending tombstone across an external append; a follow-up op re-emits it", async () => {
    const onChange = vi.fn();
    const { rerender } = await renderInAct(
      editorTree({ value: [COMMENT_A, COMMENT_B], onChange }),
    );

    // Local delete of A — its save is still pending in the debounce.
    await act(async () => {
      fireEvent.click(
        screen.getAllByRole("button", { name: /delete comment/i })[0],
      );
    });
    expect(onChange.mock.calls[0][0]).toEqual([
      { id: COMMENT_A.id, deleted: true },
      COMMENT_B,
    ]);

    // External append lands before our save flushes: A is still present
    // in the fresh server value.
    await rerenderInAct(
      rerender,
      editorTree({ value: [COMMENT_A, COMMENT_B, COMMENT_C], onChange }),
    );

    // The locally deleted member must not reappear...
    expect(screen.queryByText("First comment")).toBeNull();
    expect(screen.getByText("Agent appended")).toBeTruthy();

    // ...and a follow-up op (delete B) must still carry A's tombstone.
    await act(async () => {
      fireEvent.click(
        screen.getAllByRole("button", { name: /delete comment/i })[0],
      );
    });
    expect(onChange.mock.calls[1][0]).toEqual([
      { id: COMMENT_A.id, deleted: true },
      { id: COMMENT_B.id, deleted: true },
      COMMENT_C,
    ]);
  });

  it("keeps a pending {text} add across an external append; a follow-up op re-emits it", async () => {
    const onChange = vi.fn();
    const { rerender } = await renderInAct(
      editorTree({ value: [COMMENT_A], onChange }),
    );

    await composeAndSubmit("typed but unflushed");
    expect(onChange.mock.calls[0][0]).toEqual([
      COMMENT_A,
      { text: "typed but unflushed" },
    ]);

    await rerenderInAct(
      rerender,
      editorTree({ value: [COMMENT_A, COMMENT_C], onChange }),
    );

    // The pending comment must stay visible...
    expect(screen.getByText("typed but unflushed")).toBeTruthy();

    // ...and a follow-up op (delete A) must still carry it.
    await act(async () => {
      fireEvent.click(
        screen.getAllByRole("button", { name: /delete comment/i })[0],
      );
    });
    expect(onChange.mock.calls[1][0]).toEqual([
      { id: COMMENT_A.id, deleted: true },
      COMMENT_C,
      { text: "typed but unflushed" },
    ]);
  });

  it("keeps an un-acknowledged text edit across an external append", async () => {
    const onChange = vi.fn();
    const { rerender } = await renderInAct(
      editorTree({ value: [COMMENT_A], onChange }),
    );

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /edit comment/i }));
    });
    const textarea = screen.getByDisplayValue("First comment");
    await act(async () => {
      fireEvent.change(textarea, { target: { value: "locally edited" } });
    });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /^save$/i }));
    });
    expect(onChange.mock.calls[0][0]).toEqual([
      { ...COMMENT_A, text: "locally edited" },
    ]);

    // External append; the server still returns A's original text.
    await rerenderInAct(
      rerender,
      editorTree({ value: [COMMENT_A, COMMENT_C], onChange }),
    );

    expect(screen.getByText("locally edited")).toBeTruthy();
    expect(screen.queryByText("First comment")).toBeNull();

    // Follow-up op (delete the agent comment) re-emits the edit.
    await act(async () => {
      fireEvent.click(
        screen.getAllByRole("button", { name: /delete comment/i })[1],
      );
    });
    expect(onChange.mock.calls[1][0]).toEqual([
      { ...COMMENT_A, text: "locally edited" },
      { id: COMMENT_C.id, deleted: true },
    ]);
  });

  it("drops a tombstone once the server acknowledges the delete", async () => {
    const onChange = vi.fn();
    const { rerender } = await renderInAct(
      editorTree({ value: [COMMENT_A, COMMENT_B], onChange }),
    );

    await act(async () => {
      fireEvent.click(
        screen.getAllByRole("button", { name: /delete comment/i })[0],
      );
    });

    // Server round-trip applied the delete: A is gone from the value.
    await rerenderInAct(rerender, editorTree({ value: [COMMENT_B], onChange }));

    await composeAndSubmit("after ack");
    expect(onChange.mock.calls[1][0]).toEqual([
      COMMENT_B,
      { text: "after ack" },
    ]);
  });

  it("drops a pending add once the server mints it", async () => {
    const onChange = vi.fn();
    const { rerender } = await renderInAct(
      editorTree({ value: [COMMENT_A], onChange }),
    );

    await composeAndSubmit("minted later");

    const minted = {
      id: "01dddddddddddddddddddddddddd",
      actor: "alice",
      text: "minted later",
      timestamp: "2026-01-04T00:00:00+00:00",
    };
    await rerenderInAct(
      rerender,
      editorTree({ value: [COMMENT_A, minted], onChange }),
    );

    // Exactly one copy — the pending placeholder is gone.
    expect(screen.getAllByText("minted later").length).toBe(1);

    // A follow-up op must not re-emit the wire-only {text} member.
    await act(async () => {
      fireEvent.click(
        screen.getAllByRole("button", { name: /delete comment/i })[0],
      );
    });
    expect(onChange.mock.calls[1][0]).toEqual([
      { id: COMMENT_A.id, deleted: true },
      minted,
    ]);
  });
});

describe("CommentLogEditor — pure UI", () => {
  it("never dispatches a command — persistence flows only through onChange/onCommit", async () => {
    const onChange = vi.fn();
    await renderEditor({ value: [COMMENT_A], onChange });

    mockInvoke.mockClear();

    // Exercise every mutation path: add, edit, delete.
    await composeAndSubmit("a new comment");
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /edit comment/i }));
    });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /^cancel$/i }));
    });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /delete comment/i }));
    });

    const dispatches = mockInvoke.mock.calls.filter(
      (c) => c[0] === "dispatch_command",
    );
    expect(dispatches).toEqual([]);
    expect(onChange).toHaveBeenCalled();
  });
});
