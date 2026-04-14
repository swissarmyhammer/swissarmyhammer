/**
 * Tests for DateEditor submit/cancel wiring.
 *
 * Verifies that buildSubmitCancelExtensions refs are configured correctly:
 * - vim normal-mode Escape commits (if resolved) or cancels
 * - CUA Escape always cancels
 * - Enter commits resolved date or cancels if unparsable
 *
 * The underlying extension behavior (capture-phase listeners, vim mode
 * detection) is tested in cm-submit-cancel.test.ts. These tests verify
 * the DateEditor-specific ref wiring at the component level.
 *
 * A second `describe` block covers the field.description placeholder /
 * empty-state rendering contract — trigger text and CM6 placeholder both
 * fall back to field.description when no value is set.
 */

import { describe, it, expect, vi, afterEach } from "vitest";
import { render, act } from "@testing-library/react";
import { EditorView } from "@codemirror/view";
import { EditorState } from "@codemirror/state";
import { vim, getCM, Vim } from "@replit/codemirror-vim";
import { buildSubmitCancelExtensions } from "@/lib/cm-submit-cancel";
import { DateEditor } from "./date-editor";
import { UIStateProvider } from "@/lib/ui-state-context";
import type { Entity, FieldDef } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Tauri mocks — required by UIStateProvider transitively for the component
// tests further down. Declared up-front so all test imports see them.
// ---------------------------------------------------------------------------

vi.mock("@tauri-apps/api/core", () => ({
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  invoke: vi.fn((cmd: string) => {
    if (cmd === "get_ui_state") {
      return Promise.resolve({
        keymap_mode: "cua",
        scope_chain: [],
        open_boards: [],
        has_clipboard: false,
        clipboard_entity_type: null,
        windows: {},
        recent_boards: [],
      });
    }
    return Promise.resolve(null);
  }),
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

// Suppress console.log from debug logging
vi.spyOn(console, "log").mockImplementation(() => {});

/** Create a minimal CM6 EditorView with extensions and initial doc. */
function createEditor(
  extensions: import("@codemirror/state").Extension[],
  doc = "",
) {
  const parent = document.createElement("div");
  document.body.appendChild(parent);
  const view = new EditorView({
    state: EditorState.create({ doc, extensions }),
    parent,
  });
  return {
    view,
    parent,
    cleanup: () => {
      view.destroy();
      parent.remove();
    },
  };
}

/** Simulate a keydown event on a target element. */
function simulateKeydown(target: HTMLElement, key: string) {
  target.dispatchEvent(
    new KeyboardEvent("keydown", {
      key,
      bubbles: true,
      cancelable: true,
    }),
  );
}

describe("DateEditor submit/cancel ref wiring", () => {
  /**
   * Replicate the ref setup from DateEditor to test in isolation.
   *
   * This mirrors the exact logic from the component:
   * - submitRef: Enter → commit resolved or cancel
   * - escapeRef: vim → commit resolved or cancel; CUA → cancel
   */
  function makeDateEditorRefs(
    mode: string,
    resolved: string | null,
    onCommit: (iso: string) => void,
    onCancel: () => void,
  ) {
    const commitRef = { current: onCommit };
    const cancelRef = { current: onCancel };
    const resolvedRef = { current: resolved };

    const submitRef = { current: null as (() => void) | null };
    submitRef.current = () => {
      const r = resolvedRef.current;
      if (r) commitRef.current(r);
      else cancelRef.current();
    };

    const escapeRef = { current: null as (() => void) | null };
    escapeRef.current =
      mode === "vim"
        ? () => {
            const r = resolvedRef.current;
            if (r) commitRef.current(r);
            else cancelRef.current();
          }
        : () => cancelRef.current();

    return { submitRef, escapeRef, resolvedRef };
  }

  // --- Unit tests for ref logic (no CM6 needed) ---

  describe("ref callbacks", () => {
    it("submitRef commits resolved date when available", () => {
      const onCommit = vi.fn();
      const onCancel = vi.fn();
      const { submitRef } = makeDateEditorRefs(
        "cua",
        "2025-06-15",
        onCommit,
        onCancel,
      );

      submitRef.current!();

      expect(onCommit).toHaveBeenCalledWith("2025-06-15");
      expect(onCancel).not.toHaveBeenCalled();
    });

    it("submitRef cancels when no resolved date", () => {
      const onCommit = vi.fn();
      const onCancel = vi.fn();
      const { submitRef } = makeDateEditorRefs("cua", null, onCommit, onCancel);

      submitRef.current!();

      expect(onCommit).not.toHaveBeenCalled();
      expect(onCancel).toHaveBeenCalledOnce();
    });

    it("vim escapeRef commits resolved date", () => {
      const onCommit = vi.fn();
      const onCancel = vi.fn();
      const { escapeRef } = makeDateEditorRefs(
        "vim",
        "2025-06-15",
        onCommit,
        onCancel,
      );

      escapeRef.current!();

      expect(onCommit).toHaveBeenCalledWith("2025-06-15");
      expect(onCancel).not.toHaveBeenCalled();
    });

    it("vim escapeRef cancels when no resolved date", () => {
      const onCommit = vi.fn();
      const onCancel = vi.fn();
      const { escapeRef } = makeDateEditorRefs("vim", null, onCommit, onCancel);

      escapeRef.current!();

      expect(onCommit).not.toHaveBeenCalled();
      expect(onCancel).toHaveBeenCalledOnce();
    });

    it("CUA escapeRef always cancels regardless of resolved", () => {
      const onCommit = vi.fn();
      const onCancel = vi.fn();
      const { escapeRef } = makeDateEditorRefs(
        "cua",
        "2025-06-15",
        onCommit,
        onCancel,
      );

      escapeRef.current!();

      expect(onCommit).not.toHaveBeenCalled();
      expect(onCancel).toHaveBeenCalledOnce();
    });
  });

  // --- Integration tests with real CM6 + buildSubmitCancelExtensions ---

  describe("vim mode with real EditorView", () => {
    let cleanup: () => void;

    afterEach(() => {
      cleanup?.();
    });

    it("Escape in normal mode commits resolved date", () => {
      const onCommit = vi.fn();
      const onCancel = vi.fn();
      const { escapeRef, submitRef } = makeDateEditorRefs(
        "vim",
        "2025-06-15",
        onCommit,
        onCancel,
      );

      const extensions = [
        vim(),
        ...buildSubmitCancelExtensions({
          mode: "vim",
          onSubmitRef: submitRef,
          onCancelRef: escapeRef,
          singleLine: true,
        }),
      ];
      const { view, cleanup: c } = createEditor(extensions, "next friday");
      cleanup = c;

      // Verify normal mode
      const cm = getCM(view);
      expect(cm!.state.vim?.insertMode).toBeFalsy();

      // Escape in normal mode → escapeRef → commit
      simulateKeydown(view.contentDOM, "Escape");

      expect(onCommit).toHaveBeenCalledWith("2025-06-15");
      expect(onCancel).not.toHaveBeenCalled();
    });

    it("Escape in insert mode does NOT commit or cancel", () => {
      const onCommit = vi.fn();
      const onCancel = vi.fn();
      const { escapeRef, submitRef } = makeDateEditorRefs(
        "vim",
        "2025-06-15",
        onCommit,
        onCancel,
      );

      const extensions = [
        vim(),
        ...buildSubmitCancelExtensions({
          mode: "vim",
          onSubmitRef: submitRef,
          onCancelRef: escapeRef,
          singleLine: true,
        }),
      ];
      const { view, cleanup: c } = createEditor(extensions, "next friday");
      cleanup = c;

      // Enter insert mode
      const cm = getCM(view);
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      Vim.handleKey(cm as any, "i", "mapping");
      expect(cm!.state.vim?.insertMode).toBe(true);

      // Escape in insert mode → vim handles it (exits to normal), no commit/cancel
      simulateKeydown(view.contentDOM, "Escape");

      expect(onCommit).not.toHaveBeenCalled();
      expect(onCancel).not.toHaveBeenCalled();
    });

    it("Enter in normal mode commits resolved date", () => {
      const onCommit = vi.fn();
      const onCancel = vi.fn();
      const { escapeRef, submitRef } = makeDateEditorRefs(
        "vim",
        "2025-06-15",
        onCommit,
        onCancel,
      );

      const extensions = [
        vim(),
        ...buildSubmitCancelExtensions({
          mode: "vim",
          onSubmitRef: submitRef,
          onCancelRef: escapeRef,
          singleLine: true,
        }),
      ];
      const { view, cleanup: c } = createEditor(extensions, "next friday");
      cleanup = c;

      simulateKeydown(view.dom, "Enter");

      expect(onCommit).toHaveBeenCalledWith("2025-06-15");
    });
  });

  describe("CUA mode with real EditorView", () => {
    let cleanup: () => void;

    afterEach(() => {
      cleanup?.();
    });

    it("Escape cancels without committing", () => {
      const onCommit = vi.fn();
      const onCancel = vi.fn();
      const { escapeRef, submitRef } = makeDateEditorRefs(
        "cua",
        "2025-06-15",
        onCommit,
        onCancel,
      );

      const extensions = [
        ...buildSubmitCancelExtensions({
          mode: "cua",
          onSubmitRef: submitRef,
          onCancelRef: escapeRef,
          singleLine: true,
        }),
      ];
      const { view, cleanup: c } = createEditor(extensions, "next friday");
      cleanup = c;

      simulateKeydown(view.contentDOM, "Escape");

      expect(onCancel).toHaveBeenCalledOnce();
      expect(onCommit).not.toHaveBeenCalled();
    });

    it("Enter commits resolved date", () => {
      const onCommit = vi.fn();
      const onCancel = vi.fn();
      const { escapeRef, submitRef } = makeDateEditorRefs(
        "cua",
        "2025-06-15",
        onCommit,
        onCancel,
      );

      const extensions = [
        ...buildSubmitCancelExtensions({
          mode: "cua",
          onSubmitRef: submitRef,
          onCancelRef: escapeRef,
          singleLine: true,
        }),
      ];
      const { view, cleanup: c } = createEditor(extensions, "next friday");
      cleanup = c;

      simulateKeydown(view.contentDOM, "Enter");

      expect(onCommit).toHaveBeenCalledWith("2025-06-15");
    });
  });
});

// ---------------------------------------------------------------------------
// Component-level placeholder / empty-state tests
// ---------------------------------------------------------------------------

/** Date field fixture with a description (mirrors the `due` builtin). */
const DUE_FIELD: FieldDef = {
  id: "00000000000000000000000011",
  name: "due",
  description: "Hard deadline date",
  type: { kind: "date" },
  editor: "date",
  display: "date",
} as unknown as FieldDef;

/** Date field fixture with no description — exercises the `-` fallback. */
const BARE_DATE_FIELD: FieldDef = {
  id: "00000000000000000000000099",
  name: "bare_date",
  type: { kind: "date" },
  editor: "date",
  display: "date",
} as unknown as FieldDef;

/** Minimal host entity (the editor doesn't read from it today, but the
 * contract requires it, so fixtures supply it). */
const TASK_ENTITY: Entity = {
  id: "task-1",
  entity_type: "task",
  moniker: "task:task-1",
  fields: {},
};

/** Render DateEditor inside the UIStateProvider it requires. */
function renderDateEditor(props: { field: FieldDef; value: unknown }) {
  return render(
    <UIStateProvider>
      <DateEditor
        field={props.field}
        entity={TASK_ENTITY}
        value={props.value}
        mode="full"
        onCommit={vi.fn()}
        onCancel={vi.fn()}
        onChange={vi.fn()}
      />
    </UIStateProvider>,
  );
}

/** Flush microtasks and pending effects (Popover + CM6 mount asynchronously). */
async function flush() {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 0));
  });
}

describe("DateEditor empty-state and placeholder", () => {
  it("renders the field description (muted) in the trigger when value is empty", async () => {
    const { container } = renderDateEditor({ field: DUE_FIELD, value: "" });
    await flush();

    // PopoverTrigger renders inside the editor's container — the muted
    // fallback is the span with text-muted-foreground/50.
    const muted = container.querySelector("span.text-muted-foreground\\/50");
    expect(muted).toBeTruthy();
    expect(muted?.textContent).toBe("Hard deadline date");
  });

  it("renders the dash (muted) in the trigger when value is empty and no description is set", async () => {
    const { container } = renderDateEditor({
      field: BARE_DATE_FIELD,
      value: "",
    });
    await flush();

    const muted = container.querySelector("span.text-muted-foreground\\/50");
    expect(muted).toBeTruthy();
    expect(muted?.textContent).toBe("-");
  });

  it("sets the CodeMirror placeholder from field.description when present", async () => {
    renderDateEditor({ field: DUE_FIELD, value: "" });
    await flush();

    // PopoverContent mounts in a Radix portal attached to document.body.
    // CM6 renders the placeholder as a span with class `cm-placeholder`.
    const placeholder =
      document.body.querySelector<HTMLElement>(".cm-placeholder");
    expect(placeholder).toBeTruthy();
    expect(placeholder?.textContent).toBe("Hard deadline date");
  });

  it("falls back to the canned placeholder when field.description is absent", async () => {
    renderDateEditor({ field: BARE_DATE_FIELD, value: "" });
    await flush();

    const placeholder =
      document.body.querySelector<HTMLElement>(".cm-placeholder");
    expect(placeholder).toBeTruthy();
    expect(placeholder?.textContent).toBe(
      "Type a date... (e.g. tomorrow, next friday)",
    );
  });
});
