import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import {
  normalizeKeyEvent,
  BINDING_TABLES,
  createKeyHandler,
} from "./keybindings";

/* ---------- helpers ---------- */

/** Build a minimal KeyboardEvent-like object for testing normalizeKeyEvent. */
function fakeKeyEvent(
  key: string,
  opts: {
    metaKey?: boolean;
    ctrlKey?: boolean;
    shiftKey?: boolean;
    altKey?: boolean;
    target?: Partial<HTMLElement>;
  } = {},
): KeyboardEvent {
  const target = opts.target ?? document.createElement("div");
  return {
    key,
    metaKey: opts.metaKey ?? false,
    ctrlKey: opts.ctrlKey ?? false,
    shiftKey: opts.shiftKey ?? false,
    altKey: opts.altKey ?? false,
    target,
    preventDefault: vi.fn(),
    stopPropagation: vi.fn(),
  } as unknown as KeyboardEvent;
}

/* ---------- normalizeKeyEvent ---------- */

describe("normalizeKeyEvent", () => {
  it("returns a plain letter key as-is", () => {
    expect(normalizeKeyEvent(fakeKeyEvent("u"))).toBe("u");
  });

  it("returns Escape unchanged", () => {
    expect(normalizeKeyEvent(fakeKeyEvent("Escape"))).toBe("Escape");
  });

  it("returns colon unchanged", () => {
    expect(normalizeKeyEvent(fakeKeyEvent(":"))).toBe(":");
  });

  it("returns colon without Shift prefix (Shift produces the punctuation)", () => {
    // Real keyboard: colon is Shift+semicolon, so shiftKey is true
    // but e.key is already ":", not ";". Shift should NOT be added.
    const e = fakeKeyEvent(":", { shiftKey: true });
    expect(normalizeKeyEvent(e)).toBe(":");
  });

  it("normalizes Meta on Mac to Mod", () => {
    // Simulate Mac: navigator.platform starts with Mac
    const original = Object.getOwnPropertyDescriptor(navigator, "platform");
    Object.defineProperty(navigator, "platform", { value: "MacIntel", configurable: true });
    try {
      const e = fakeKeyEvent("p", { metaKey: true, shiftKey: true });
      expect(normalizeKeyEvent(e)).toBe("Mod+Shift+P");
    } finally {
      if (original) {
        Object.defineProperty(navigator, "platform", original);
      }
    }
  });

  it("normalizes Control on non-Mac to Mod", () => {
    const original = Object.getOwnPropertyDescriptor(navigator, "platform");
    Object.defineProperty(navigator, "platform", { value: "Win32", configurable: true });
    try {
      const e = fakeKeyEvent("p", { ctrlKey: true, shiftKey: true });
      expect(normalizeKeyEvent(e)).toBe("Mod+Shift+P");
    } finally {
      if (original) {
        Object.defineProperty(navigator, "platform", original);
      }
    }
  });

  it("includes Alt modifier", () => {
    const e = fakeKeyEvent("x", { altKey: true });
    expect(normalizeKeyEvent(e)).toBe("Alt+x");
  });

  it("strips lone modifier keys (Meta, Control, Shift, Alt)", () => {
    expect(normalizeKeyEvent(fakeKeyEvent("Meta"))).toBeNull();
    expect(normalizeKeyEvent(fakeKeyEvent("Control"))).toBeNull();
    expect(normalizeKeyEvent(fakeKeyEvent("Shift"))).toBeNull();
    expect(normalizeKeyEvent(fakeKeyEvent("Alt"))).toBeNull();
  });

  it("uppercases letter keys when Shift is pressed", () => {
    const e = fakeKeyEvent("z", { shiftKey: true });
    expect(normalizeKeyEvent(e)).toBe("Shift+Z");
  });

  it("combines Mod+key without Shift", () => {
    const original = Object.getOwnPropertyDescriptor(navigator, "platform");
    Object.defineProperty(navigator, "platform", { value: "MacIntel", configurable: true });
    try {
      const e = fakeKeyEvent("z", { metaKey: true });
      expect(normalizeKeyEvent(e)).toBe("Mod+z");
    } finally {
      if (original) {
        Object.defineProperty(navigator, "platform", original);
      }
    }
  });

  it("normalizes Mod+Shift+Z correctly", () => {
    const original = Object.getOwnPropertyDescriptor(navigator, "platform");
    Object.defineProperty(navigator, "platform", { value: "MacIntel", configurable: true });
    try {
      const e = fakeKeyEvent("z", { metaKey: true, shiftKey: true });
      expect(normalizeKeyEvent(e)).toBe("Mod+Shift+Z");
    } finally {
      if (original) {
        Object.defineProperty(navigator, "platform", original);
      }
    }
  });
});

/* ---------- BINDING_TABLES ---------- */

describe("BINDING_TABLES", () => {
  it("has entries for vim, cua, and emacs modes", () => {
    expect(BINDING_TABLES).toHaveProperty("vim");
    expect(BINDING_TABLES).toHaveProperty("cua");
    expect(BINDING_TABLES).toHaveProperty("emacs");
  });

  it("vim bindings include expected commands", () => {
    const vim = BINDING_TABLES.vim;
    expect(vim[":"]).toBe("app.command");
    expect(vim["Mod+Shift+P"]).toBe("app.palette");
    expect(vim["u"]).toBe("app.undo");
    expect(vim["Mod+r"]).toBe("app.redo");
    expect(vim["Escape"]).toBe("app.dismiss");
  });

  it("cua bindings include expected commands", () => {
    const cua = BINDING_TABLES.cua;
    expect(cua["Mod+Shift+P"]).toBe("app.palette");
    expect(cua["Mod+z"]).toBe("app.undo");
    expect(cua["Mod+Shift+Z"]).toBe("app.redo");
    expect(cua["Escape"]).toBe("app.dismiss");
  });

  it("emacs bindings include expected commands", () => {
    const emacs = BINDING_TABLES.emacs;
    expect(emacs["Mod+Shift+P"]).toBe("app.palette");
    expect(emacs["Escape"]).toBe("app.dismiss");
  });
});

/* ---------- createKeyHandler ---------- */

describe("createKeyHandler", () => {
  let executeCommand: (id: string) => Promise<boolean>;

  beforeEach(() => {
    executeCommand = vi.fn(async () => true) as (id: string) => Promise<boolean>;
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("executes a single-key binding from cua mode", () => {
    const handler = createKeyHandler("cua", executeCommand);
    const e = fakeKeyEvent("Escape");
    handler(e);
    expect(executeCommand).toHaveBeenCalledWith("app.dismiss");
  });

  it("executes a modifier binding from cua mode", () => {
    const original = Object.getOwnPropertyDescriptor(navigator, "platform");
    Object.defineProperty(navigator, "platform", { value: "MacIntel", configurable: true });
    try {
      const handler = createKeyHandler("cua", executeCommand);
      const e = fakeKeyEvent("z", { metaKey: true });
      handler(e);
      expect(executeCommand).toHaveBeenCalledWith("app.undo");
    } finally {
      if (original) {
        Object.defineProperty(navigator, "platform", original);
      }
    }
  });

  it("does not execute for unknown keys", () => {
    const handler = createKeyHandler("cua", executeCommand);
    const e = fakeKeyEvent("q");
    handler(e);
    expect(executeCommand).not.toHaveBeenCalled();
  });

  it("calls preventDefault and stopPropagation on matched key", () => {
    const handler = createKeyHandler("cua", executeCommand);
    const e = fakeKeyEvent("Escape");
    handler(e);
    expect(e.preventDefault).toHaveBeenCalled();
    expect(e.stopPropagation).toHaveBeenCalled();
  });

  it("does not call preventDefault for unmatched keys", () => {
    const handler = createKeyHandler("cua", executeCommand);
    const e = fakeKeyEvent("q");
    handler(e);
    expect(e.preventDefault).not.toHaveBeenCalled();
  });

  /* ---------- editable context skip ---------- */

  it("skips non-modifier keys when target is inside .cm-editor", () => {
    const cmEditor = document.createElement("div");
    cmEditor.className = "cm-editor";
    const inner = document.createElement("div");
    cmEditor.appendChild(inner);
    document.body.appendChild(cmEditor);

    try {
      const handler = createKeyHandler("vim", executeCommand);
      // Single-char key `:` should be skipped inside cm-editor
      handler(fakeKeyEvent(":", { target: inner }));
      expect(executeCommand).not.toHaveBeenCalled();
    } finally {
      document.body.removeChild(cmEditor);
    }
  });

  it("allows modifier combos inside .cm-editor", () => {
    const original = Object.getOwnPropertyDescriptor(navigator, "platform");
    Object.defineProperty(navigator, "platform", { value: "MacIntel", configurable: true });
    const cmEditor = document.createElement("div");
    cmEditor.className = "cm-editor";
    const inner = document.createElement("div");
    cmEditor.appendChild(inner);
    document.body.appendChild(cmEditor);

    try {
      const handler = createKeyHandler("cua", executeCommand);
      // Mod+Z should still work inside cm-editor
      handler(fakeKeyEvent("z", { metaKey: true, target: inner }));
      expect(executeCommand).toHaveBeenCalledWith("app.undo");
    } finally {
      document.body.removeChild(cmEditor);
      if (original) {
        Object.defineProperty(navigator, "platform", original);
      }
    }
  });

  it("skips non-modifier keys when target is an input", () => {
    const input = document.createElement("input");
    document.body.appendChild(input);

    try {
      const handler = createKeyHandler("vim", executeCommand);
      handler(fakeKeyEvent(":", { target: input }));
      expect(executeCommand).not.toHaveBeenCalled();
    } finally {
      document.body.removeChild(input);
    }
  });

  it("skips non-modifier keys when target is a textarea", () => {
    const textarea = document.createElement("textarea");
    document.body.appendChild(textarea);

    try {
      const handler = createKeyHandler("vim", executeCommand);
      handler(fakeKeyEvent("u", { target: textarea }));
      expect(executeCommand).not.toHaveBeenCalled();
    } finally {
      document.body.removeChild(textarea);
    }
  });

  it("skips non-modifier keys when target is inside contenteditable", () => {
    const editable = document.createElement("div");
    editable.setAttribute("contenteditable", "true");
    const inner = document.createElement("span");
    editable.appendChild(inner);
    document.body.appendChild(editable);

    try {
      const handler = createKeyHandler("vim", executeCommand);
      handler(fakeKeyEvent(":", { target: inner }));
      expect(executeCommand).not.toHaveBeenCalled();
    } finally {
      document.body.removeChild(editable);
    }
  });

  /* ---------- vim multi-key sequences ---------- */

  it("handles vim gg sequence", () => {
    const handler = createKeyHandler("vim", executeCommand);

    // First 'g' should not fire immediately
    handler(fakeKeyEvent("g"));
    expect(executeCommand).not.toHaveBeenCalled();

    // Second 'g' completes the sequence
    handler(fakeKeyEvent("g"));
    expect(executeCommand).toHaveBeenCalledWith("board.firstColumn");
  });

  it("handles vim dd sequence", () => {
    const handler = createKeyHandler("vim", executeCommand);

    handler(fakeKeyEvent("d"));
    expect(executeCommand).not.toHaveBeenCalled();

    handler(fakeKeyEvent("d"));
    expect(executeCommand).toHaveBeenCalledWith("entity.archive");
  });

  it("handles vim zo sequence", () => {
    const handler = createKeyHandler("vim", executeCommand);

    handler(fakeKeyEvent("z"));
    expect(executeCommand).not.toHaveBeenCalled();

    handler(fakeKeyEvent("o"));
    expect(executeCommand).toHaveBeenCalledWith("task.toggleCollapse");
  });

  it("clears pending buffer after 500ms timeout", () => {
    const handler = createKeyHandler("vim", executeCommand);

    handler(fakeKeyEvent("g"));
    expect(executeCommand).not.toHaveBeenCalled();

    // Advance past the timeout
    vi.advanceTimersByTime(501);

    // Now pressing 'g' again should start a fresh sequence, not complete 'gg'
    handler(fakeKeyEvent("g"));
    expect(executeCommand).not.toHaveBeenCalled();

    // Complete the fresh sequence
    handler(fakeKeyEvent("g"));
    expect(executeCommand).toHaveBeenCalledWith("board.firstColumn");
  });

  it("clears pending buffer when a non-matching key follows", () => {
    const handler = createKeyHandler("vim", executeCommand);

    // Start a 'g' sequence
    handler(fakeKeyEvent("g"));

    // Press something that doesn't complete any sequence starting with 'g'
    handler(fakeKeyEvent("x"));

    // The buffer should be cleared, and 'x' does not match anything alone
    expect(executeCommand).not.toHaveBeenCalled();
  });

  it("still handles single-key vim bindings alongside multi-key", () => {
    const handler = createKeyHandler("vim", executeCommand);
    handler(fakeKeyEvent("u"));
    expect(executeCommand).toHaveBeenCalledWith("app.undo");
  });

  it("vim colon binding works as single key", () => {
    const handler = createKeyHandler("vim", executeCommand);
    handler(fakeKeyEvent(":"));
    expect(executeCommand).toHaveBeenCalledWith("app.command");
  });
});
