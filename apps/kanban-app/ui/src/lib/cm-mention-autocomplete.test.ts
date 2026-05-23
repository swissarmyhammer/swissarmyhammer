/**
 * Tests for createMentionCompletionSource.
 *
 * The dropdown must preview what the widget will show after insertion:
 *   - label shows `${prefix}${displayName}` (matches the widget visible text)
 *   - apply writes `${prefix}${slug}` (what actually lands in the buffer)
 *   - detail shows the slug as a secondary hint
 *   - info renders a colored dot followed by the display name
 */

import { describe, it, expect } from "vitest";
import { EditorView } from "@codemirror/view";
import { EditorState } from "@codemirror/state";
import type {
  CompletionContext,
  CompletionResult,
} from "@codemirror/autocomplete";
import {
  createMentionCompletionSource,
  type MentionSearchResult,
  type CommandSearchResult,
} from "./cm-mention-autocomplete";

/**
 * Build a real CompletionContext by mounting a tiny EditorView, placing the
 * cursor after the given doc text, and returning a context with `explicit: true`
 * so the source will fire for short queries like just a bare prefix.
 */
function makeContext(
  doc: string,
  explicit = true,
): {
  context: CompletionContext;
  cleanup: () => void;
} {
  const parent = document.createElement("div");
  document.body.appendChild(parent);
  const view = new EditorView({
    state: EditorState.create({ doc }),
    parent,
  });
  view.dispatch({ selection: { anchor: doc.length } });
  // Minimal CompletionContext shim that uses the real view state. CM6
  // exposes CompletionContext as a class, but its behavior when driven
  // from a source only depends on `state`, `pos`, `explicit`, and the
  // `matchBefore` helper — all of which we can provide here.
  const pos = view.state.selection.main.head;
  const context = {
    state: view.state,
    pos,
    explicit,
    matchBefore(regex: RegExp) {
      const line = view.state.doc.lineAt(pos);
      const before = line.text.slice(0, pos - line.from);
      const match = before.match(regex);
      if (!match || typeof match.index !== "number") return null;
      const from = line.from + match.index;
      const to = pos;
      if (from === to) return null;
      return { from, to, text: match[0] };
    },
    aborted: false,
    tokenBefore() {
      return null;
    },
  } as unknown as CompletionContext;

  return {
    context,
    cleanup: () => {
      view.destroy();
      parent.remove();
    },
  };
}

describe("createMentionCompletionSource Completion shape", () => {
  it("returns label=prefix+displayName, apply=prefix+slug, detail=slug", async () => {
    const result: MentionSearchResult = {
      slug: "my-project",
      displayName: "My Project",
      color: "ff0000",
    };
    const source = createMentionCompletionSource("$", () => [result]);

    const { context, cleanup } = makeContext("$my");
    try {
      const completionResult = (await source(context)) as CompletionResult;
      expect(completionResult).not.toBeNull();
      expect(completionResult.options).toHaveLength(1);

      const opt = completionResult.options[0];
      expect(opt.label).toBe("$My Project");
      expect(opt.apply).toBe("$my-project");
      expect(opt.detail).toBe("my-project");
    } finally {
      cleanup();
    }
  });

  it("info renders a colored dot followed by the display name", async () => {
    const result: MentionSearchResult = {
      slug: "my-project",
      displayName: "My Project",
      color: "ff0000",
    };
    const source = createMentionCompletionSource("$", () => [result]);

    const { context, cleanup } = makeContext("$my");
    try {
      const completionResult = (await source(context)) as CompletionResult;
      const opt = completionResult.options[0];
      expect(typeof opt.info).toBe("function");

      // CM6's info callback is `(completion) => Node | Promise<Node | null> | null`.
      // Our implementation is a zero-arg function that returns a DOM Node.
      const infoFn = opt.info as (c: typeof opt) => unknown;
      const dom = infoFn(opt) as HTMLElement;
      expect(dom).toBeInstanceOf(HTMLElement);
      expect(dom.textContent).toBe("My Project");

      const dot = dom.querySelector("span");
      expect(dot).toBeTruthy();
      expect((dot as HTMLElement).style.backgroundColor).toBe("rgb(255, 0, 0)");
    } finally {
      cleanup();
    }
  });

  it("works with an async search function", async () => {
    const result: MentionSearchResult = {
      slug: "my-project",
      displayName: "My Project",
      color: "ff0000",
    };
    const source = createMentionCompletionSource("$", async () => [result]);

    const { context, cleanup } = makeContext("$my");
    try {
      const completionResult = (await source(context)) as CompletionResult;
      expect(completionResult.options).toHaveLength(1);
      const opt = completionResult.options[0];
      expect(opt.label).toBe("$My Project");
      expect(opt.apply).toBe("$my-project");
      expect(opt.detail).toBe("my-project");
    } finally {
      cleanup();
    }
  });

  it("yields null when the search returns no results", async () => {
    // An empty result set must close the menu, not open an empty one — the
    // distinction the composer's Enter-yield guard relies on: no menu means
    // `completionStatus` stays null and Enter submits.
    const source = createMentionCompletionSource("#", () => []);

    const { context, cleanup } = makeContext("#zzz");
    try {
      const completionResult = await source(context);
      expect(completionResult).toBeNull();
    } finally {
      cleanup();
    }
  });
});

describe("createMentionCompletionSource — slash command flavor", () => {
  it("returns label=/name and apply=/name for a command result", async () => {
    const command: CommandSearchResult = {
      name: "plan",
      description: "Draft an execution plan",
    };
    const source = createMentionCompletionSource("/", () => [command]);

    const { context, cleanup } = makeContext("/pl");
    try {
      const completionResult = (await source(context)) as CompletionResult;
      expect(completionResult).not.toBeNull();
      expect(completionResult.options).toHaveLength(1);

      const opt = completionResult.options[0];
      // The command name lands literally as `/name`, in both the dropdown
      // label and what `apply` writes to the buffer — the agent owns command
      // execution, the composer only inserts the literal text.
      expect(opt.label).toBe("/plan");
      expect(opt.apply).toBe("/plan");
    } finally {
      cleanup();
    }
  });

  it("info renders the description text with no color dot", async () => {
    const command: CommandSearchResult = {
      name: "plan",
      description: "Draft an execution plan",
    };
    const source = createMentionCompletionSource("/", () => [command]);

    const { context, cleanup } = makeContext("/pl");
    try {
      const completionResult = (await source(context)) as CompletionResult;
      const opt = completionResult.options[0];
      expect(typeof opt.info).toBe("function");

      const infoFn = opt.info as (c: typeof opt) => unknown;
      const dom = infoFn(opt) as HTMLElement;
      expect(dom).toBeInstanceOf(HTMLElement);
      expect(dom.textContent).toBe("Draft an execution plan");
      // Command completions carry a description, not a color — there is no
      // nested colored-dot span like the entity-mention info widget.
      expect(dom.querySelector("span")).toBeNull();
    } finally {
      cleanup();
    }
  });

  it("yields null when the command search returns no results", async () => {
    // Mirrors an empty `availableCommands`: typing `/` opens no menu.
    const source = createMentionCompletionSource("/", () => []);

    const { context, cleanup } = makeContext("/");
    try {
      const completionResult = await source(context);
      expect(completionResult).toBeNull();
    } finally {
      cleanup();
    }
  });

  it("opens on a bare `/` during auto-typing when openOnBarePrefix is set", async () => {
    // Auto-trigger (explicit: false) on just `/` must list every command — the
    // slash menu's discoverability contract. The default mention sources stay
    // suppressed on a bare prefix; this is opt-in.
    const command: CommandSearchResult = {
      name: "plan",
      description: "Draft an execution plan",
    };
    const source = createMentionCompletionSource("/", () => [command], {
      openOnBarePrefix: true,
    });

    const { context, cleanup } = makeContext("/", false);
    try {
      const completionResult = (await source(context)) as CompletionResult;
      expect(completionResult).not.toBeNull();
      expect(completionResult.options[0].label).toBe("/plan");
    } finally {
      cleanup();
    }
  });

  it("stays suppressed on a bare prefix during auto-typing by default", async () => {
    // Without the opt-in, a bare prefix during auto-typing yields null — the
    // existing `#`/`@` behavior is unchanged.
    const result: MentionSearchResult = {
      slug: "bug",
      displayName: "bug",
      color: "ff0000",
    };
    const source = createMentionCompletionSource("#", () => [result]);

    const { context, cleanup } = makeContext("#", false);
    try {
      const completionResult = await source(context);
      expect(completionResult).toBeNull();
    } finally {
      cleanup();
    }
  });
});
