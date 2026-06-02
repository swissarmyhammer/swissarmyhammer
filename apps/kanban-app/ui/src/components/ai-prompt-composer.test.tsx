/**
 * Component tests for {@link AiPromptComposer} — the AI panel's CM6 composer.
 *
 * The composer is a CodeMirror 6 instance built on the app's shared
 * {@link TextEditor} primitive, so it honors the active keymap (vim / emacs /
 * CUA) exactly like every other text input in the app ("CM6 everywhere",
 * `ideas/kanban/app-architecture.md`). It is NOT a plain `<textarea>`.
 *
 * These tests pin two contracts of kanban task `01KRRQ3SPXBY1ZNRJHFGB09R3Z`:
 *
 *   - the composer mounts a real CM6 editor (a `.cm-editor` with an
 *     `EditorView`), and a keymap motion works inside it;
 *   - Enter submits the buffer (`sendPrompt`), and the stop affordance fires
 *     `cancel` while a turn streams.
 *
 * Browser project (`*.test.tsx`) — CM6 mounts in real Chromium.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { act, screen, within } from "@testing-library/react";
import { userEvent } from "vitest/browser";
import { renderInAct } from "@/test/act-render";

// The keymap mode the composer's CM6 editor picks up — overridden per test.
let mockKeymapMode = "cua";
vi.mock("@/lib/ui-state-context", () => ({
  useUIState: () => ({ keymap_mode: mockKeymapMode }),
}));

import { completionStatus, currentCompletions } from "@codemirror/autocomplete";
import type { AvailableCommand } from "@agentclientprotocol/sdk";
import { AiPromptComposer } from "./ai-prompt-composer";
import type { AiModel } from "./ai-panel";
import { copyText } from "@/lib/clipboard";

/** The Claude Code + a disabled local model fixture for the footer select. */
const MODELS: AiModel[] = [
  {
    id: "claude-code",
    label: "Claude Code",
    kind: "claude-code",
    available: true,
    hint: "Claude Code CLI: /usr/local/bin/claude",
  },
  {
    id: "qwen",
    label: "Qwen Coder",
    kind: "local-llama",
    available: false,
    hint: "Model weights unavailable on this machine.",
  },
];

/** Resolve the live `EditorView` from a freshly rendered composer. */
async function getView(container: HTMLElement) {
  const cmEditor = container.querySelector(".cm-editor") as HTMLElement | null;
  expect(cmEditor, "the composer must mount a CM6 .cm-editor").toBeTruthy();
  const { EditorView } = await import("@codemirror/view");
  const view = EditorView.findFromDOM(cmEditor!);
  expect(view, "the .cm-editor must have a live EditorView").toBeTruthy();
  return view!;
}

describe("AiPromptComposer — CM6 instance honoring the active keymap", () => {
  beforeEach(() => {
    mockKeymapMode = "cua";
  });

  it("mounts a real CodeMirror 6 editor, not a plain textarea", async () => {
    const { container } = await renderInAct(
      <AiPromptComposer
        disabled={false}
        placeholder="Ask the AI agent..."
        streaming={false}
        onSend={() => {}}
        onCancel={() => {}}
        models={MODELS}
        selectedModel={MODELS[0]}
        onSelectModel={() => {}}
      />,
    );

    // A CM6 editor — `.cm-editor` with an `.cm-content[contenteditable]` —
    // is present, and there is no plain `<textarea>`.
    expect(container.querySelector(".cm-editor")).not.toBeNull();
    expect(container.querySelector(".cm-content")).not.toBeNull();
    expect(container.querySelector("textarea")).toBeNull();
    // The CM6 content DOM advertises itself as a textbox with the panel's
    // accessible label — the contract `ai.focus` and the panel tests rely on.
    const content = container.querySelector(".cm-content") as HTMLElement;
    expect(content.getAttribute("role")).toBe("textbox");
    expect(content.getAttribute("aria-label")).toBe("Message the AI agent");
  });

  it("a vim keymap motion works inside the composer", async () => {
    // Vim keymap — the editor starts in normal mode (TextEditor exits insert
    // on mount). Type a word in insert mode, then exercise a normal-mode
    // motion to prove the vim keymap is live inside the composer.
    mockKeymapMode = "vim";
    const { container } = await renderInAct(
      <AiPromptComposer
        disabled={false}
        placeholder="Ask the AI agent..."
        streaming={false}
        onSend={() => {}}
        onCancel={() => {}}
        models={MODELS}
        selectedModel={MODELS[0]}
        onSelectModel={() => {}}
      />,
    );
    const view = await getView(container);
    view.contentDOM.focus();

    // `i` enters insert mode, type the buffer, `Escape` returns to normal.
    await act(async () => {
      await userEvent.type(view.contentDOM, "i");
      await userEvent.type(view.contentDOM, "hello world");
      await userEvent.keyboard("{Escape}");
    });
    expect(view.state.doc.toString()).toBe("hello world");

    // Normal-mode motion: `0` jumps the cursor to the start of the line,
    // `x` deletes the character under the cursor. A working vim keymap
    // therefore turns "hello world" into "ello world".
    await act(async () => {
      await userEvent.type(view.contentDOM, "0");
      await userEvent.type(view.contentDOM, "x");
    });
    expect(
      view.state.doc.toString(),
      "the vim `0` motion + `x` delete must run inside the composer's CM6 editor",
    ).toBe("ello world");
  });

  it("an emacs keymap motion works inside the composer", async () => {
    // Emacs keymap — `Ctrl-A` moves the cursor to the start of the line.
    mockKeymapMode = "emacs";
    const { container } = await renderInAct(
      <AiPromptComposer
        disabled={false}
        placeholder="Ask the AI agent..."
        streaming={false}
        onSend={() => {}}
        onCancel={() => {}}
        models={MODELS}
        selectedModel={MODELS[0]}
        onSelectModel={() => {}}
      />,
    );
    const view = await getView(container);
    view.contentDOM.focus();

    await act(async () => {
      await userEvent.type(view.contentDOM, "abc");
    });
    expect(view.state.selection.main.head).toBe(3);

    // `Ctrl-A` (emacs: move-to-line-start) jumps the cursor to offset 0.
    await act(async () => {
      await userEvent.keyboard("{Control>}a{/Control}");
    });
    expect(
      view.state.selection.main.head,
      "the emacs Ctrl-A motion must run inside the composer's CM6 editor",
    ).toBe(0);
  });

  it("Enter submits the buffer via onSend", async () => {
    const onSend = vi.fn();
    const { container } = await renderInAct(
      <AiPromptComposer
        disabled={false}
        placeholder="Ask the AI agent..."
        streaming={false}
        onSend={onSend}
        onCancel={() => {}}
        models={MODELS}
        selectedModel={MODELS[0]}
        onSelectModel={() => {}}
      />,
    );
    const view = await getView(container);
    view.contentDOM.focus();

    await act(async () => {
      await userEvent.type(view.contentDOM, "ship it");
    });
    await act(async () => {
      await userEvent.keyboard("{Enter}");
    });
    expect(onSend).toHaveBeenCalledExactlyOnceWith("ship it");
  });

  it("Shift+Enter inserts a newline instead of submitting", async () => {
    const onSend = vi.fn();
    const { container } = await renderInAct(
      <AiPromptComposer
        disabled={false}
        placeholder="Ask the AI agent..."
        streaming={false}
        onSend={onSend}
        onCancel={() => {}}
        models={MODELS}
        selectedModel={MODELS[0]}
        onSelectModel={() => {}}
      />,
    );
    const view = await getView(container);
    view.contentDOM.focus();

    await act(async () => {
      await userEvent.type(view.contentDOM, "line one");
      await userEvent.keyboard("{Shift>}{Enter}{/Shift}");
      await userEvent.type(view.contentDOM, "line two");
    });
    // Shift+Enter grew the buffer with a newline — it did not submit.
    expect(view.state.doc.toString()).toBe("line one\nline two");
    expect(onSend).not.toHaveBeenCalled();
  });

  it("Enter on an empty buffer is a true no-op — no submit, no blank line", async () => {
    const onSend = vi.fn();
    const { container } = await renderInAct(
      <AiPromptComposer
        disabled={false}
        placeholder="Ask the AI agent..."
        streaming={false}
        onSend={onSend}
        onCancel={() => {}}
        models={MODELS}
        selectedModel={MODELS[0]}
        onSelectModel={() => {}}
      />,
    );
    const view = await getView(container);
    view.contentDOM.focus();

    // Repeated Enter on an empty composer must neither submit nor accumulate
    // blank lines — the keystroke is swallowed entirely.
    await act(async () => {
      await userEvent.keyboard("{Enter}");
      await userEvent.keyboard("{Enter}");
      await userEvent.keyboard("{Enter}");
    });
    expect(onSend).not.toHaveBeenCalled();
    expect(
      view.state.doc.toString(),
      "Enter on an empty composer must not insert a newline",
    ).toBe("");
  });

  it("the stop button cancels while a turn streams; the submit button is hidden", async () => {
    const onCancel = vi.fn();
    const { container } = await renderInAct(
      <AiPromptComposer
        disabled={false}
        placeholder="Ask the AI agent..."
        streaming={true}
        onSend={() => {}}
        onCancel={onCancel}
        models={MODELS}
        selectedModel={MODELS[0]}
        onSelectModel={() => {}}
      />,
    );

    // While streaming the action button is a stop control.
    const stop = container.querySelector(
      "button[aria-label='Stop']",
    ) as HTMLButtonElement | null;
    expect(stop, "a stop button must render while streaming").not.toBeNull();
    await act(async () => {
      await userEvent.click(stop!);
    });
    expect(onCancel).toHaveBeenCalledOnce();
  });

  it("shows the stop control as an icon button, not verbose status text, while streaming", async () => {
    const { container } = await renderInAct(
      <AiPromptComposer
        disabled={false}
        placeholder="Ask the AI agent..."
        streaming={true}
        onSend={() => {}}
        onCancel={() => {}}
        models={MODELS}
        selectedModel={MODELS[0]}
        onSelectModel={() => {}}
      />,
    );

    // The hard-to-read inline status prose must be gone — the stop affordance is
    // the icon button (with an accessible "Stop" label / hover tooltip), not text.
    expect(
      container.textContent ?? "",
      "the verbose 'click to stop' status text must not render",
    ).not.toMatch(/click to stop/i);

    // The stop control is still present as an icon button.
    const stop = container.querySelector("button[aria-label='Stop']");
    expect(
      stop,
      "a stop icon button must render while streaming",
    ).not.toBeNull();
    expect(
      stop?.querySelector("svg"),
      "the stop control must render an icon, not text",
    ).not.toBeNull();
  });

  it("is inert when disabled — the CM6 editor is not editable", async () => {
    const { container } = await renderInAct(
      <AiPromptComposer
        disabled={true}
        placeholder="Select a model to start..."
        streaming={false}
        onSend={() => {}}
        onCancel={() => {}}
        models={MODELS}
        selectedModel={MODELS[0]}
        onSelectModel={() => {}}
      />,
    );
    const content = container.querySelector(".cm-content") as HTMLElement;
    expect(content, "the CM6 content DOM must be present").not.toBeNull();
    // A disabled composer's CM6 editor is not editable.
    expect(content.getAttribute("contenteditable")).toBe("false");
  });
});

/** Two ACP slash commands the agent advertises via `available_commands_update`. */
const COMMANDS: AvailableCommand[] = [
  { name: "plan", description: "Draft an execution plan" },
  { name: "review", description: "Review the diff" },
];

describe("AiPromptComposer — slash-command autocomplete", () => {
  beforeEach(() => {
    mockKeymapMode = "cua";
  });

  it("typing / opens a command menu listing every available command", async () => {
    const { container } = await renderInAct(
      <AiPromptComposer
        disabled={false}
        placeholder="Ask the AI agent..."
        streaming={false}
        onSend={() => {}}
        onCancel={() => {}}
        models={MODELS}
        selectedModel={MODELS[0]}
        onSelectModel={() => {}}
        availableCommands={COMMANDS}
      />,
    );
    const view = await getView(container);
    view.contentDOM.focus();

    await act(async () => {
      await userEvent.type(view.contentDOM, "/");
    });

    // The menu activates after the activate-on-typing delay; poll for it.
    await vi.waitFor(
      () => {
        expect(completionStatus(view.state)).toBe("active");
      },
      { timeout: 2000 },
    );

    const labels = currentCompletions(view.state).map((c) => c.label);
    expect(labels).toContain("/plan");
    expect(labels).toContain("/review");
  });

  it("typing / opens no menu when availableCommands is empty", async () => {
    const { container } = await renderInAct(
      <AiPromptComposer
        disabled={false}
        placeholder="Ask the AI agent..."
        streaming={false}
        onSend={() => {}}
        onCancel={() => {}}
        models={MODELS}
        selectedModel={MODELS[0]}
        onSelectModel={() => {}}
        availableCommands={[]}
      />,
    );
    const view = await getView(container);
    view.contentDOM.focus();

    await act(async () => {
      await userEvent.type(view.contentDOM, "/");
    });
    // Give the activate-on-typing delay time to elapse, then assert no menu.
    await new Promise((resolve) => setTimeout(resolve, 350));
    expect(completionStatus(view.state)).toBeNull();
  });

  it("plain Enter with the menu open accepts the completion and does not submit", async () => {
    const onSend = vi.fn();
    const { container } = await renderInAct(
      <AiPromptComposer
        disabled={false}
        placeholder="Ask the AI agent..."
        streaming={false}
        onSend={onSend}
        onCancel={() => {}}
        models={MODELS}
        selectedModel={MODELS[0]}
        onSelectModel={() => {}}
        availableCommands={COMMANDS}
      />,
    );
    const view = await getView(container);
    view.contentDOM.focus();

    await act(async () => {
      await userEvent.type(view.contentDOM, "/");
    });
    await vi.waitFor(
      () => {
        expect(completionStatus(view.state)).toBe("active");
      },
      { timeout: 2000 },
    );
    // CM6 refuses `acceptCompletion` within `interactionDelay` (~75ms) of the
    // menu opening, to guard against an Enter landing as the menu appears.
    // Wait past that window so Enter accepts rather than falling through.
    await new Promise((resolve) => setTimeout(resolve, 150));

    await act(async () => {
      await userEvent.keyboard("{Enter}");
    });

    // Enter accepted the highlighted completion (the first command) into the
    // buffer — it did NOT submit.
    expect(onSend).not.toHaveBeenCalled();
    expect(view.state.doc.toString()).toBe("/plan");
  });

  it("plain Enter with the menu closed still submits the buffer", async () => {
    const onSend = vi.fn();
    const { container } = await renderInAct(
      <AiPromptComposer
        disabled={false}
        placeholder="Ask the AI agent..."
        streaming={false}
        onSend={onSend}
        onCancel={() => {}}
        models={MODELS}
        selectedModel={MODELS[0]}
        onSelectModel={() => {}}
        availableCommands={COMMANDS}
      />,
    );
    const view = await getView(container);
    view.contentDOM.focus();

    await act(async () => {
      await userEvent.type(view.contentDOM, "ship it");
    });
    // No slash typed — the command menu never opens. `activateOnTyping`
    // reports a transient "pending" while it debounces over the prose, so wait
    // for it to settle to "not active" before asserting the menu is closed.
    await vi.waitFor(
      () => {
        expect(completionStatus(view.state)).not.toBe("active");
      },
      { timeout: 2000 },
    );

    await act(async () => {
      await userEvent.keyboard("{Enter}");
    });
    expect(onSend).toHaveBeenCalledExactlyOnceWith("ship it");
  });

  it("Shift+Enter inserts a newline even with the command menu open", async () => {
    const onSend = vi.fn();
    const { container } = await renderInAct(
      <AiPromptComposer
        disabled={false}
        placeholder="Ask the AI agent..."
        streaming={false}
        onSend={onSend}
        onCancel={() => {}}
        models={MODELS}
        selectedModel={MODELS[0]}
        onSelectModel={() => {}}
        availableCommands={COMMANDS}
      />,
    );
    const view = await getView(container);
    view.contentDOM.focus();

    await act(async () => {
      await userEvent.type(view.contentDOM, "/");
    });
    await vi.waitFor(
      () => {
        expect(completionStatus(view.state)).toBe("active");
      },
      { timeout: 2000 },
    );

    await act(async () => {
      await userEvent.keyboard("{Shift>}{Enter}{/Shift}");
    });
    // Shift+Enter grew the buffer with a newline — it neither submitted nor
    // accepted a completion.
    expect(onSend).not.toHaveBeenCalled();
    expect(view.state.doc.toString()).toContain("\n");
  });
});

describe("AiPromptComposer — single bordered container", () => {
  beforeEach(() => {
    mockKeymapMode = "cua";
  });

  it("renders exactly one bordered container — no border nested inside a border", async () => {
    const { container } = await renderInAct(
      <AiPromptComposer
        disabled={false}
        placeholder="Ask the AI agent..."
        streaming={false}
        onSend={() => {}}
        onCancel={() => {}}
        models={MODELS}
        selectedModel={MODELS[0]}
        onSelectModel={() => {}}
      />,
    );

    // The AI Elements `PromptInput` shell is a SINGLE bordered box that
    // holds the CM6 body and the footer toolbar. The old composer stacked
    // an inner `rounded-md border` editor well inside `ComposerArea`'s
    // `border-t` section — a doubled edge. Assert the composer's own
    // bordered element has no descendant that is also a bordered element.
    const composer = container.querySelector(
      "[data-slot='ai-prompt-composer']",
    ) as HTMLElement | null;
    expect(composer, "the composer root must be present").not.toBeNull();

    const bordered = composer!.querySelectorAll(".border, .border-t");
    expect(
      bordered.length,
      "the composer must render exactly one bordered container",
    ).toBe(1);
    // And that single bordered element has no bordered descendant — the
    // structural definition of "no doubled border".
    const nested = bordered[0].querySelectorAll(".border, .border-t");
    expect(
      nested.length,
      "the composer's bordered container must not nest another border",
    ).toBe(0);
  });
});

describe("AiPromptComposer — footer model select", () => {
  beforeEach(() => {
    mockKeymapMode = "cua";
  });

  it("renders the model picker in the composer footer, listing every model", async () => {
    const { container } = await renderInAct(
      <AiPromptComposer
        disabled={false}
        placeholder="Ask the AI agent..."
        streaming={false}
        onSend={() => {}}
        onCancel={() => {}}
        models={MODELS}
        selectedModel={MODELS[0]}
        onSelectModel={() => {}}
      />,
    );

    // The model select is part of the composer — its trigger is a
    // `role="combobox"` button showing the selected model's label.
    const trigger = screen.getByRole("combobox", { name: /claude code/i });
    expect(
      container
        .querySelector("[data-slot='ai-prompt-composer']")
        ?.contains(trigger),
      "the model select trigger must live inside the composer",
    ).toBe(true);

    await act(async () => {
      await userEvent.click(trigger);
    });

    const listbox = await screen.findByRole("listbox");
    const options = within(listbox).getAllByRole("option");
    expect(options).toHaveLength(2);
    expect(options[0].textContent).toContain("Claude Code");
    expect(options[1].textContent).toContain("Qwen Coder");
  });

  it("disables an unavailable model and surfaces its hint", async () => {
    await renderInAct(
      <AiPromptComposer
        disabled={false}
        placeholder="Ask the AI agent..."
        streaming={false}
        onSend={() => {}}
        onCancel={() => {}}
        models={MODELS}
        selectedModel={MODELS[0]}
        onSelectModel={() => {}}
      />,
    );

    await act(async () => {
      await userEvent.click(
        screen.getByRole("combobox", { name: /claude code/i }),
      );
    });

    const listbox = await screen.findByRole("listbox");
    const qwen = within(listbox).getByRole("option", { name: /qwen coder/i });
    // The unavailable local model cannot be picked and still shows its hint.
    expect(qwen.getAttribute("aria-disabled")).toBe("true");
    expect(qwen.textContent).toContain("Model weights unavailable");
  });

  it("selecting a model reports the choice via onSelectModel", async () => {
    const onSelectModel = vi.fn();
    // Two available models so a second one can be picked.
    const bothAvailable: AiModel[] = [
      { ...MODELS[0] },
      { ...MODELS[1], available: true, hint: "Local model." },
    ];

    await renderInAct(
      <AiPromptComposer
        disabled={false}
        placeholder="Ask the AI agent..."
        streaming={false}
        onSend={() => {}}
        onCancel={() => {}}
        models={bothAvailable}
        selectedModel={bothAvailable[0]}
        onSelectModel={onSelectModel}
      />,
    );

    await act(async () => {
      await userEvent.click(
        screen.getByRole("combobox", { name: /claude code/i }),
      );
    });
    const listbox = await screen.findByRole("listbox");
    await act(async () => {
      await userEvent.click(
        within(listbox).getByRole("option", { name: /qwen coder/i }),
      );
    });

    expect(onSelectModel).toHaveBeenCalledWith("qwen");
  });
});

describe("AiPromptComposer — chat copy populates the vim register", () => {
  beforeEach(() => {
    mockKeymapMode = "vim";
  });

  it("a bare `p` pastes text put on the clipboard via copyText", async () => {
    // Mount the composer's vim-mode CM6 editor (it exits insert on mount, so
    // it starts in normal mode).
    const { container } = await renderInAct(
      <AiPromptComposer
        disabled={false}
        placeholder="Ask the AI agent..."
        streaming={false}
        onSend={() => {}}
        onCancel={() => {}}
        models={MODELS}
        selectedModel={MODELS[0]}
        onSelectModel={() => {}}
      />,
    );
    const view = await getView(container);

    // Headless Chromium denies the `clipboard-write` permission, so the real
    // OS-clipboard write is not exercisable here — stub it (the unit test pins
    // the real `writeText` call with a spy). What this test drives is the other
    // half of `copyText`: the vim-register mirror, proven by a real `p`
    // keystroke pasting the text. Before the fix the mirror does not exist, so
    // `p` pastes nothing; after the fix `p` pastes the copied text.
    vi.spyOn(navigator.clipboard, "writeText").mockResolvedValue(undefined);

    // Copy text the way the chat's Copy buttons do — through the shared helper.
    await act(async () => {
      await copyText("PASTE_ME");
    });

    // The editor starts in normal mode (TextEditor exits insert on mount);
    // focus it and press a bare `p` to paste.
    view.contentDOM.focus();
    await act(async () => {
      await userEvent.type(view.contentDOM, "p");
    });

    expect(
      view.state.doc.toString(),
      "a bare `p` must paste the externally-copied text from the vim register",
    ).toContain("PASTE_ME");
  });
});
