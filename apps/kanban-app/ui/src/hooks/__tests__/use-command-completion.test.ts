/**
 * Tests for {@link useCommandCompletionExtension} — the `/` slash-command
 * autocomplete the AI composer mounts.
 *
 * The hook is deliberately context-free (it reads no schema / entity-store /
 * board-data providers), so these tests need no Tauri or context mocks — they
 * mount the returned extension into a real CM6 EditorView and assert the menu.
 */

import { describe, it, expect } from "vitest";
import { renderHook } from "@testing-library/react";
import type { AvailableCommand } from "@agentclientprotocol/sdk";
import { useCommandCompletionExtension } from "../use-command-completion";

/** Two ACP slash commands the agent advertises. */
const COMMANDS: AvailableCommand[] = [
  { name: "plan", description: "Draft an execution plan" },
  { name: "review", description: "Review the diff" },
];

describe("useCommandCompletionExtension", () => {
  it("returns no extension when there are no available commands", () => {
    const { result } = renderHook(() => useCommandCompletionExtension([]));
    // No commands → no autocomplete source → typing `/` opens nothing and the
    // composer's Enter still submits.
    expect(result.current).toEqual([]);
  });

  it("opens a `/` completion menu listing the available commands", async () => {
    const { result } = renderHook(() =>
      useCommandCompletionExtension(COMMANDS),
    );

    const { EditorView } = await import("@codemirror/view");
    const { EditorState } = await import("@codemirror/state");
    const { startCompletion, currentCompletions } =
      await import("@codemirror/autocomplete");

    const parent = document.createElement("div");
    document.body.appendChild(parent);
    const view = new EditorView({
      state: EditorState.create({
        doc: "/",
        extensions: result.current,
      }),
      parent,
    });

    // Place the cursor after the `/` and trigger autocomplete. The command
    // search is synchronous, so the completions resolve on the next tick.
    view.dispatch({ selection: { anchor: 1 } });
    startCompletion(view);
    await new Promise((resolve) => setTimeout(resolve, 100));

    const labels = currentCompletions(view.state).map((c) => c.label);
    expect(labels).toContain("/plan");
    expect(labels).toContain("/review");

    view.destroy();
    parent.remove();
  });

  it("filters the menu by the typed command-name substring", async () => {
    const { result } = renderHook(() =>
      useCommandCompletionExtension(COMMANDS),
    );

    const { EditorView } = await import("@codemirror/view");
    const { EditorState } = await import("@codemirror/state");
    const { startCompletion, currentCompletions } =
      await import("@codemirror/autocomplete");

    const parent = document.createElement("div");
    document.body.appendChild(parent);
    const view = new EditorView({
      state: EditorState.create({
        doc: "/rev",
        extensions: result.current,
      }),
      parent,
    });

    view.dispatch({ selection: { anchor: 4 } });
    startCompletion(view);
    await new Promise((resolve) => setTimeout(resolve, 100));

    const labels = currentCompletions(view.state).map((c) => c.label);
    expect(labels).toContain("/review");
    expect(labels).not.toContain("/plan");

    view.destroy();
    parent.remove();
  });
});
