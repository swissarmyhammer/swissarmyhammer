import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import {
  normalizeKeyEvent,
  BINDING_TABLES,
  createKeyHandler,
  extractScopeBindings,
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
    Object.defineProperty(navigator, "platform", {
      value: "MacIntel",
      configurable: true,
    });
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
    Object.defineProperty(navigator, "platform", {
      value: "Win32",
      configurable: true,
    });
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
    Object.defineProperty(navigator, "platform", {
      value: "MacIntel",
      configurable: true,
    });
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
    Object.defineProperty(navigator, "platform", {
      value: "MacIntel",
      configurable: true,
    });
    try {
      const e = fakeKeyEvent("z", { metaKey: true, shiftKey: true });
      expect(normalizeKeyEvent(e)).toBe("Mod+Shift+Z");
    } finally {
      if (original) {
        Object.defineProperty(navigator, "platform", original);
      }
    }
  });

  it("canonicalises spacebar (e.key === ' ') to 'Space'", () => {
    // Browsers report the spacebar as a literal single space
    // (`e.key === " "`). The binding tables and command-keys speak
    // in symbolic names, so the normalizer must translate that
    // single character into the canonical "Space" token. Without
    // this rewrite, command-keys like `keys: { cua: "Space" }`
    // would never match a real keystroke.
    expect(normalizeKeyEvent(fakeKeyEvent(" "))).toBe("Space");
  });

  it("canonicalises spacebar with Mod modifier to 'Mod+Space'", () => {
    const original = Object.getOwnPropertyDescriptor(navigator, "platform");
    Object.defineProperty(navigator, "platform", {
      value: "MacIntel",
      configurable: true,
    });
    try {
      const e = fakeKeyEvent(" ", { metaKey: true });
      expect(normalizeKeyEvent(e)).toBe("Mod+Space");
    } finally {
      if (original) {
        Object.defineProperty(navigator, "platform", original);
      }
    }
  });

  it("prefixes Shift on Space to distinguish Shift+Space from Space", () => {
    // The browser delivers the spacebar as `e.key === " "` (literal
    // space) regardless of whether Shift is held — same disambiguation
    // problem as Tab/Shift+Tab. The normalizer rewrites `" "` to the
    // canonical `"Space"` token *before* the Shift-prefix check, so
    // when Shift is held the result is `"Shift+Space"` rather than
    // `"Shift+ "` (which would be invisible in code review and would
    // never match a binding declared as `Shift+Space`).
    expect(normalizeKeyEvent(fakeKeyEvent(" ", { shiftKey: true }))).toBe(
      "Shift+Space",
    );
    expect(normalizeKeyEvent(fakeKeyEvent(" "))).toBe("Space");
  });

  /* ---------- Shift on symbolic keys ---------- */
  //
  // Symbolic keys (Tab, Enter, Escape, arrows, Home/End/PageUp/PageDown,
  // Insert/Delete/Backspace, F1–F12) report the same `e.key` whether
  // Shift is held or not — the only signal is `e.shiftKey`. Without an
  // explicit `Shift+` prefix, Shift+Tab and Tab hash to the same
  // canonical string and cannot bind to distinct commands. The
  // normalizer prepends `Shift+` for these keys when shiftKey is true.
  // Letter keys keep the existing uppercase-and-prefix behaviour;
  // punctuation produced by Shift (`:`, `?`, etc.) keeps no prefix
  // because `e.key` is already the shifted character.

  it("prefixes Shift on Tab to distinguish Shift+Tab from Tab", () => {
    expect(normalizeKeyEvent(fakeKeyEvent("Tab", { shiftKey: true }))).toBe(
      "Shift+Tab",
    );
    expect(normalizeKeyEvent(fakeKeyEvent("Tab", { shiftKey: false }))).toBe(
      "Tab",
    );
  });

  it("prefixes Shift on Enter and Escape", () => {
    expect(normalizeKeyEvent(fakeKeyEvent("Enter", { shiftKey: true }))).toBe(
      "Shift+Enter",
    );
    expect(normalizeKeyEvent(fakeKeyEvent("Enter"))).toBe("Enter");
    expect(normalizeKeyEvent(fakeKeyEvent("Escape", { shiftKey: true }))).toBe(
      "Shift+Escape",
    );
    expect(normalizeKeyEvent(fakeKeyEvent("Escape"))).toBe("Escape");
  });

  it("prefixes Shift on the four arrow keys", () => {
    for (const arrow of [
      "ArrowUp",
      "ArrowDown",
      "ArrowLeft",
      "ArrowRight",
    ] as const) {
      expect(normalizeKeyEvent(fakeKeyEvent(arrow, { shiftKey: true }))).toBe(
        `Shift+${arrow}`,
      );
      expect(normalizeKeyEvent(fakeKeyEvent(arrow))).toBe(arrow);
    }
  });

  it("prefixes Shift on Home/End/PageUp/PageDown", () => {
    for (const key of ["Home", "End", "PageUp", "PageDown"] as const) {
      expect(normalizeKeyEvent(fakeKeyEvent(key, { shiftKey: true }))).toBe(
        `Shift+${key}`,
      );
      expect(normalizeKeyEvent(fakeKeyEvent(key))).toBe(key);
    }
  });

  it("prefixes Shift on Insert/Delete/Backspace", () => {
    for (const key of ["Insert", "Delete", "Backspace"] as const) {
      expect(normalizeKeyEvent(fakeKeyEvent(key, { shiftKey: true }))).toBe(
        `Shift+${key}`,
      );
      expect(normalizeKeyEvent(fakeKeyEvent(key))).toBe(key);
    }
  });

  it("prefixes Shift on F1–F12", () => {
    for (let n = 1; n <= 12; n++) {
      const key = `F${n}`;
      expect(normalizeKeyEvent(fakeKeyEvent(key, { shiftKey: true }))).toBe(
        `Shift+${key}`,
      );
      expect(normalizeKeyEvent(fakeKeyEvent(key))).toBe(key);
    }
  });

  it("does NOT prefix Shift on punctuation produced by Shift", () => {
    // Real keyboard: `?` is Shift+`/`, but `e.key` is already `?`.
    // Adding `Shift+` would break the punctuation binding lookup.
    expect(normalizeKeyEvent(fakeKeyEvent("?", { shiftKey: true }))).toBe("?");
    // `:` from Shift+`;` — also already covered by an earlier test, but
    // pinned again here alongside the broader shift-on-symbolic
    // contract for documentation.
    expect(normalizeKeyEvent(fakeKeyEvent(":", { shiftKey: true }))).toBe(":");
  });

  it("combines Mod+Shift+Tab correctly on Mac", () => {
    const original = Object.getOwnPropertyDescriptor(navigator, "platform");
    Object.defineProperty(navigator, "platform", {
      value: "MacIntel",
      configurable: true,
    });
    try {
      const e = fakeKeyEvent("Tab", { metaKey: true, shiftKey: true });
      expect(normalizeKeyEvent(e)).toBe("Mod+Shift+Tab");
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
    // Escape is now claimed by `nav.drillOut`, which delegates to
    // `app.dismiss` itself when the spatial registry has nothing to
    // drill out of (no spatial focus or layer-root). The global
    // binding therefore points at the drill command, not directly at
    // `app.dismiss` — the dismiss path still runs, just via the
    // drill closure's null fall-through.
    expect(vim["Escape"]).toBe("nav.drillOut");
    // Enter drives drill-in: descends into a focused zone or no-ops
    // on a focusable leaf without an inline-edit affordance.
    expect(vim["Enter"]).toBe("nav.drillIn");
  });

  it("cua bindings include expected commands", () => {
    const cua = BINDING_TABLES.cua;
    expect(cua["Mod+Shift+P"]).toBe("app.palette");
    expect(cua["Mod+z"]).toBe("app.undo");
    expect(cua["Mod+Shift+Z"]).toBe("app.redo");
    // See vim notes — Escape is now `nav.drillOut`, which falls
    // through to `app.dismiss` on a null kernel result.
    expect(cua["Escape"]).toBe("nav.drillOut");
    expect(cua["Enter"]).toBe("nav.drillIn");
    // Tab / Shift+Tab cycle siblings via `nav.right`/`nav.left`.
    // Inspector scopes can shadow these via `inspector.nextField` /
    // `inspector.prevField` to keep the form-style "Tab moves between
    // fields" behaviour where it matters; everywhere else Tab cycles.
    expect(cua["Tab"]).toBe("nav.right");
    expect(cua["Shift+Tab"]).toBe("nav.left");
  });

  it("emacs bindings include expected commands", () => {
    const emacs = BINDING_TABLES.emacs;
    expect(emacs["Mod+Shift+P"]).toBe("app.palette");
    expect(emacs["Escape"]).toBe("nav.drillOut");
    expect(emacs["Enter"]).toBe("nav.drillIn");
  });
});

/* ---------- createKeyHandler ---------- */

describe("createKeyHandler", () => {
  let executeCommand: (id: string) => Promise<boolean>;

  beforeEach(() => {
    executeCommand = vi.fn(async () => true) as (
      id: string,
    ) => Promise<boolean>;
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("executes a single-key binding from cua mode", () => {
    const handler = createKeyHandler("cua", executeCommand);
    const e = fakeKeyEvent("Escape");
    handler(e);
    // Escape now drives `nav.drillOut`; the dismiss path still
    // runs via that command's null fall-through, but the binding
    // table dispatches `nav.drillOut` directly.
    expect(executeCommand).toHaveBeenCalledWith("nav.drillOut");
  });

  it("executes a modifier binding from cua mode", () => {
    const original = Object.getOwnPropertyDescriptor(navigator, "platform");
    Object.defineProperty(navigator, "platform", {
      value: "MacIntel",
      configurable: true,
    });
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
    Object.defineProperty(navigator, "platform", {
      value: "MacIntel",
      configurable: true,
    });
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
    expect(executeCommand).toHaveBeenCalledWith("nav.first");
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

  it("handles vim gt sequence → perspective.next", () => {
    const handler = createKeyHandler("vim", executeCommand);

    handler(fakeKeyEvent("g"));
    expect(executeCommand).not.toHaveBeenCalled();

    handler(fakeKeyEvent("t"));
    expect(executeCommand).toHaveBeenCalledWith("perspective.next");
  });

  it("handles vim gT (Shift+T) sequence → perspective.prev", () => {
    const handler = createKeyHandler("vim", executeCommand);

    handler(fakeKeyEvent("g"));
    expect(executeCommand).not.toHaveBeenCalled();

    handler(fakeKeyEvent("T", { shiftKey: true }));
    expect(executeCommand).toHaveBeenCalledWith("perspective.prev");
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
    expect(executeCommand).toHaveBeenCalledWith("nav.first");
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

  /* ---------- scope bindings ---------- */

  it("dispatches scope bindings when getScopeBindings is provided", () => {
    const scopeBindings = () => ({ ArrowDown: "inspector.moveDown" });
    const handler = createKeyHandler("cua", executeCommand, scopeBindings);
    handler(fakeKeyEvent("ArrowDown"));
    expect(executeCommand).toHaveBeenCalledWith("inspector.moveDown");
  });

  it("scope bindings shadow global bindings for the same key", () => {
    // Escape is globally `nav.drillOut` (which itself falls through
    // to `app.dismiss` on a null kernel result) — scope can still
    // claim it back when the focused subtree wants its own Escape
    // semantics (e.g. the inspector's own close handler).
    const scopeBindings = () => ({ Escape: "inspector.escape" });
    const handler = createKeyHandler("cua", executeCommand, scopeBindings);
    handler(fakeKeyEvent("Escape"));
    expect(executeCommand).toHaveBeenCalledWith("inspector.escape");
  });

  it("falls through to global bindings when scope has no match", () => {
    const scopeBindings = () => ({ ArrowDown: "inspector.moveDown" });
    const handler = createKeyHandler("cua", executeCommand, scopeBindings);
    handler(fakeKeyEvent("Escape"));
    // Escape now binds globally to `nav.drillOut`; behaviorally the
    // user still ends up at `app.dismiss` via that command's null
    // fall-through, but the immediate binding lookup hits drill-out.
    expect(executeCommand).toHaveBeenCalledWith("nav.drillOut");
  });

  it("scope bindings update dynamically (callback called per keydown)", () => {
    let bindings: Record<string, string> = {};
    const handler = createKeyHandler("cua", executeCommand, () => bindings);

    // No scope binding yet — ArrowDown does nothing
    handler(fakeKeyEvent("ArrowDown"));
    expect(executeCommand).not.toHaveBeenCalled();

    // Simulate inspector focus — now ArrowDown resolves
    bindings = { ArrowDown: "inspector.moveDown" };
    handler(fakeKeyEvent("ArrowDown"));
    expect(executeCommand).toHaveBeenCalledWith("inspector.moveDown");
  });

  it("vim scope bindings (j/k) dispatch inspector commands", () => {
    const scopeBindings = () => ({
      j: "inspector.moveDown",
      k: "inspector.moveUp",
      i: "inspector.edit",
    });
    const handler = createKeyHandler("vim", executeCommand, scopeBindings);

    handler(fakeKeyEvent("j"));
    expect(executeCommand).toHaveBeenCalledWith("inspector.moveDown");

    handler(fakeKeyEvent("k"));
    expect(executeCommand).toHaveBeenCalledWith("inspector.moveUp");

    handler(fakeKeyEvent("i"));
    expect(executeCommand).toHaveBeenCalledWith("inspector.edit");
  });
});

/* ---------- extractScopeBindings ---------- */

interface TestScope {
  commands: Map<string, { id: string; keys?: Record<string, string> }>;
  parent: TestScope | null;
}

/** Build a minimal scope for testing. */
function makeScope(
  commands: Array<{ id: string; keys?: Record<string, string> }>,
  parent: TestScope | null = null,
): TestScope {
  const map = new Map<string, { id: string; keys?: Record<string, string> }>();
  for (const cmd of commands) map.set(cmd.id, cmd);
  return { commands: map, parent };
}

describe("extractScopeBindings", () => {
  it("extracts keys for the given mode", () => {
    const scope = makeScope([
      { id: "inspector.moveUp", keys: { vim: "k", cua: "ArrowUp" } },
      { id: "inspector.moveDown", keys: { vim: "j", cua: "ArrowDown" } },
    ]);
    const bindings = extractScopeBindings(scope, "cua");
    expect(bindings).toEqual({
      ArrowUp: "inspector.moveUp",
      ArrowDown: "inspector.moveDown",
    });
  });

  it("extracts vim keys", () => {
    const scope = makeScope([
      { id: "inspector.moveUp", keys: { vim: "k", cua: "ArrowUp" } },
    ]);
    const bindings = extractScopeBindings(scope, "vim");
    expect(bindings).toEqual({ k: "inspector.moveUp" });
  });

  it("skips commands without keys", () => {
    const scope = makeScope([
      { id: "inspector.moveUp", keys: { vim: "k" } },
      { id: "inspector.deleteRow" }, // no keys
    ]);
    const bindings = extractScopeBindings(scope, "vim");
    expect(bindings).toEqual({ k: "inspector.moveUp" });
  });

  it("skips commands without keys for the requested mode", () => {
    const scope = makeScope([
      { id: "inspector.nextField", keys: { cua: "Tab" } }, // no vim key
    ]);
    const bindings = extractScopeBindings(scope, "vim");
    expect(bindings).toEqual({});
  });

  it("inner scope shadows outer scope for same key", () => {
    const outer = makeScope([
      { id: "grid.moveDown", keys: { cua: "ArrowDown" } },
    ]);
    const inner = makeScope(
      [{ id: "inspector.moveDown", keys: { cua: "ArrowDown" } }],
      outer,
    );
    const bindings = extractScopeBindings(inner, "cua");
    expect(bindings["ArrowDown"]).toBe("inspector.moveDown");
  });

  it("includes parent scope commands not shadowed by inner", () => {
    const outer = makeScope([{ id: "app.dismiss", keys: { cua: "Escape" } }]);
    const inner = makeScope(
      [{ id: "inspector.moveDown", keys: { cua: "ArrowDown" } }],
      outer,
    );
    const bindings = extractScopeBindings(inner, "cua");
    expect(bindings).toEqual({
      ArrowDown: "inspector.moveDown",
      Escape: "app.dismiss",
    });
  });

  it("returns empty for null scope", () => {
    expect(extractScopeBindings(null, "cua")).toEqual({});
  });

  /* ---------- ui.entity.startRename — perspective-scoped Enter binding ---------- */
  //
  // The active perspective tab's `<CommandScopeProvider>` (in
  // `kanban-app/ui/src/components/perspective-tab-bar.tsx`) registers
  // `ui.entity.startRename` with `keys: { cua: "Enter", vim: "Enter", emacs: "Enter" }`
  // when the tab is the currently active perspective. The YAML mirror
  // (`swissarmyhammer-commands/builtin/commands/ui.yaml`) carries the same
  // `keys` block plus `scope: "entity:perspective"` so the palette / context
  // menu sees the binding too.
  //
  // These three guards pin the React-side contract: from any perspective scope
  // that surfaces the command, `extractScopeBindings` must return
  // `{ Enter: "ui.entity.startRename" }` for cua, vim, AND emacs. The
  // cross-cutting tests below pin the dispatch side; here we pin the
  // extraction side independently so a future regression that drops one of
  // the three modes from the React-side `keys` block fails this assertion
  // before any browser test runs.

  it("ui.entity.startRename surfaces Enter on a perspective scope (cua)", () => {
    const scope = makeScope([
      {
        id: "ui.entity.startRename",
        keys: { cua: "Enter", vim: "Enter", emacs: "Enter" },
      },
    ]);
    const bindings = extractScopeBindings(scope, "cua");
    expect(bindings).toEqual({ Enter: "ui.entity.startRename" });
  });

  it("ui.entity.startRename surfaces Enter on a perspective scope (vim)", () => {
    const scope = makeScope([
      {
        id: "ui.entity.startRename",
        keys: { cua: "Enter", vim: "Enter", emacs: "Enter" },
      },
    ]);
    const bindings = extractScopeBindings(scope, "vim");
    expect(bindings).toEqual({ Enter: "ui.entity.startRename" });
  });

  it("ui.entity.startRename surfaces Enter on a perspective scope (emacs)", () => {
    const scope = makeScope([
      {
        id: "ui.entity.startRename",
        keys: { cua: "Enter", vim: "Enter", emacs: "Enter" },
      },
    ]);
    const bindings = extractScopeBindings(scope, "emacs");
    expect(bindings).toEqual({ Enter: "ui.entity.startRename" });
  });

  it("ui.entity.startRename's Enter shadows a parent nav.drillIn: Enter", () => {
    // The global drill-in binding lives at the AppShell root — a perspective
    // scope that registers `ui.entity.startRename: Enter` must shadow it so
    // Enter inside the perspective scope chain triggers rename, not drill-in.
    // `extractScopeBindings` walks innermost-first with first-key-wins
    // semantics, so the inner perspective scope's command claims `Enter`
    // before the parent scope's nav.drillIn is reached.
    const outer = makeScope([{ id: "nav.drillIn", keys: { cua: "Enter" } }]);
    const inner = makeScope(
      [
        {
          id: "ui.entity.startRename",
          keys: { cua: "Enter", vim: "Enter", emacs: "Enter" },
        },
      ],
      outer,
    );
    const bindings = extractScopeBindings(inner, "cua");
    expect(bindings.Enter).toBe("ui.entity.startRename");
  });
});

/* ---------- cross-cutting command keybinding dispatch ---------- */
//
// These tests exercise the full dispatch cycle for every keybinding declared
// on a cross-cutting command:
//
//   - `entity.delete`  — cua `Mod+Backspace` (migrated from the retired
//                        type-specific `task.delete` so the delete shortcut
//                        now works on any entity — task, tag, column, etc.)
//   - `entity.archive` — vim `dd`
//   - `entity.cut`     — cua `Mod+X`, vim `x`
//   - `entity.copy`    — cua `Mod+C`, vim `y`
//   - `entity.paste`   — cua `Mod+V`, vim `p`
//   - `ui.inspector.close` — cua `Escape`, vim `q`
//   - `ui.palette.open`    — cua `Mod+K`, vim `:`
//
// Cross-cutting commands auto-emit into the scope chain for every entity
// moniker, so their `keys` get wired up through `extractScopeBindings` when a
// focused entity scope includes them. We simulate that here by building a
// scope whose `commands` map carries the cross-cutting command definition,
// then run the full keystroke through `createKeyHandler` and assert which
// command id fires.

describe("cross-cutting command keybinding dispatch", () => {
  let executeCommand: (id: string) => Promise<boolean>;

  beforeEach(() => {
    executeCommand = vi.fn(async () => true) as (
      id: string,
    ) => Promise<boolean>;
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  /**
   * Force macOS platform detection so `Mod+` resolves through `metaKey`.
   * Returns the original descriptor so callers can restore it.
   */
  function withMacPlatform<T>(fn: () => T): T {
    const original = Object.getOwnPropertyDescriptor(navigator, "platform");
    Object.defineProperty(navigator, "platform", {
      value: "MacIntel",
      configurable: true,
    });
    try {
      return fn();
    } finally {
      if (original) {
        Object.defineProperty(navigator, "platform", original);
      }
    }
  }

  it("vim: dd on a task-scoped focus dispatches entity.archive", () => {
    // `entity.archive` declares `keys.vim: dd` in
    // swissarmyhammer-commands/builtin/commands/entity.yaml. The
    // cross-cutting emit pass surfaces the command with its keys into the
    // task's scope; `extractScopeBindings` pulls it out.
    const scope = makeScope([{ id: "entity.archive", keys: { vim: "dd" } }]);
    const handler = createKeyHandler("vim", executeCommand, () =>
      extractScopeBindings(scope, "vim"),
    );

    // First `d` is buffered by the sequence logic.
    handler(fakeKeyEvent("d"));
    expect(executeCommand).not.toHaveBeenCalled();

    // Second `d` completes the sequence and fires.
    handler(fakeKeyEvent("d"));
    expect(executeCommand).toHaveBeenCalledWith("entity.archive");
  });

  it("vim: dd on a tag-scoped focus also dispatches entity.archive", () => {
    // Identical extraction path regardless of whether the host scope is a
    // task, tag, project, or any other entity that auto-emits
    // `entity.archive`. The binding is a function of the command's keys,
    // not the scope type.
    const scope = makeScope([{ id: "entity.archive", keys: { vim: "dd" } }]);
    const handler = createKeyHandler("vim", executeCommand, () =>
      extractScopeBindings(scope, "vim"),
    );

    handler(fakeKeyEvent("d"));
    handler(fakeKeyEvent("d"));
    expect(executeCommand).toHaveBeenCalledWith("entity.archive");
  });

  it("cua: Mod+Backspace dispatches entity.delete from a task scope", () => {
    // `entity.delete` declares `keys.cua: Mod+Backspace` in
    // swissarmyhammer-commands/builtin/commands/entity.yaml. The keybinding
    // migrated from the retired type-specific `task.delete` so the delete
    // shortcut now works on any entity (task, tag, column, project, actor).
    // The cross-cutting emit surfaces the command with its keys into every
    // entity scope; `extractScopeBindings` pulls it out.
    const scope = makeScope([
      { id: "entity.delete", keys: { cua: "Mod+Backspace" } },
    ]);

    withMacPlatform(() => {
      const handler = createKeyHandler("cua", executeCommand, () =>
        extractScopeBindings(scope, "cua"),
      );
      handler(fakeKeyEvent("Backspace", { metaKey: true }));
      expect(executeCommand).toHaveBeenCalledWith("entity.delete");
    });
  });

  it("cua: Mod+C dispatches entity.copy", () => {
    // entity.copy declares `keys.cua: Mod+C` / `keys.vim: y`.
    const scope = makeScope([
      { id: "entity.copy", keys: { cua: "Mod+C", vim: "y" } },
    ]);

    withMacPlatform(() => {
      const handler = createKeyHandler("cua", executeCommand, () =>
        extractScopeBindings(scope, "cua"),
      );
      // `C` uppercased by normalizeKeyEvent when Shift-or-Meta is held.
      handler(fakeKeyEvent("C", { metaKey: true }));
      expect(executeCommand).toHaveBeenCalledWith("entity.copy");
    });
  });

  it("cua: Mod+X dispatches entity.cut", () => {
    const scope = makeScope([
      { id: "entity.cut", keys: { cua: "Mod+X", vim: "x" } },
    ]);

    withMacPlatform(() => {
      const handler = createKeyHandler("cua", executeCommand, () =>
        extractScopeBindings(scope, "cua"),
      );
      handler(fakeKeyEvent("X", { metaKey: true }));
      expect(executeCommand).toHaveBeenCalledWith("entity.cut");
    });
  });

  it("cua: Mod+V dispatches entity.paste", () => {
    const scope = makeScope([
      { id: "entity.paste", keys: { cua: "Mod+V", vim: "p" } },
    ]);

    withMacPlatform(() => {
      const handler = createKeyHandler("cua", executeCommand, () =>
        extractScopeBindings(scope, "cua"),
      );
      handler(fakeKeyEvent("V", { metaKey: true }));
      expect(executeCommand).toHaveBeenCalledWith("entity.paste");
    });
  });

  it("vim: y dispatches entity.copy from a task scope", () => {
    const scope = makeScope([
      { id: "entity.copy", keys: { cua: "Mod+C", vim: "y" } },
    ]);
    const handler = createKeyHandler("vim", executeCommand, () =>
      extractScopeBindings(scope, "vim"),
    );
    handler(fakeKeyEvent("y"));
    expect(executeCommand).toHaveBeenCalledWith("entity.copy");
  });

  it("vim: x dispatches entity.cut from a task scope", () => {
    const scope = makeScope([
      { id: "entity.cut", keys: { cua: "Mod+X", vim: "x" } },
    ]);
    const handler = createKeyHandler("vim", executeCommand, () =>
      extractScopeBindings(scope, "vim"),
    );
    handler(fakeKeyEvent("x"));
    expect(executeCommand).toHaveBeenCalledWith("entity.cut");
  });

  it("vim: p dispatches entity.paste from a task scope", () => {
    const scope = makeScope([
      { id: "entity.paste", keys: { cua: "Mod+V", vim: "p" } },
    ]);
    const handler = createKeyHandler("vim", executeCommand, () =>
      extractScopeBindings(scope, "vim"),
    );
    handler(fakeKeyEvent("p"));
    expect(executeCommand).toHaveBeenCalledWith("entity.paste");
  });

  it("cua: Escape dispatches ui.inspector.close when its scope claims it", () => {
    // `ui.inspector.close` declares `keys.cua: Escape` — when an inspector
    // scope is focused, its Escape binding shadows the global
    // `app.dismiss` that would otherwise fire.
    const scope = makeScope([
      { id: "ui.inspector.close", keys: { cua: "Escape", vim: "q" } },
    ]);
    const handler = createKeyHandler("cua", executeCommand, () =>
      extractScopeBindings(scope, "cua"),
    );
    handler(fakeKeyEvent("Escape"));
    expect(executeCommand).toHaveBeenCalledWith("ui.inspector.close");
  });

  it("vim: q dispatches ui.inspector.close from an inspector scope", () => {
    const scope = makeScope([
      { id: "ui.inspector.close", keys: { cua: "Escape", vim: "q" } },
    ]);
    const handler = createKeyHandler("vim", executeCommand, () =>
      extractScopeBindings(scope, "vim"),
    );
    handler(fakeKeyEvent("q"));
    expect(executeCommand).toHaveBeenCalledWith("ui.inspector.close");
  });

  it("cua: Mod+K dispatches ui.palette.open when a scope claims it", () => {
    // `ui.palette.open` declares `keys.cua: Mod+K`. If the focused scope
    // surfaces the command, the binding resolves and `Mod+K` fires it. The
    // production BINDING_TABLES uses `Mod+Shift+P` for the palette; this
    // test pins the scope-bound path independently.
    const scope = makeScope([
      { id: "ui.palette.open", keys: { cua: "Mod+K", vim: ":" } },
    ]);

    withMacPlatform(() => {
      const handler = createKeyHandler("cua", executeCommand, () =>
        extractScopeBindings(scope, "cua"),
      );
      // `Mod+K` in the YAML has an uppercase K. `normalizeKeyEvent` only
      // adds `Shift+` when `shiftKey` is truly held, so to produce the
      // canonical `Mod+K` we fake a `key: "K"` event with just metaKey —
      // mirroring the character the OS hands us when the physical `k` key
      // is pressed together with a shift-style chord target (Caps Lock or a
      // pre-uppercased key map).
      handler(fakeKeyEvent("K", { metaKey: true }));
      expect(executeCommand).toHaveBeenCalledWith("ui.palette.open");
    });
  });

  it("vim: : dispatches ui.palette.open from a scope that claims it", () => {
    // Without a scope override, vim `:` hits the global `app.command`
    // binding — this test pins the scope-bound ui.palette.open path. Since
    // scope bindings shadow global ones, the scope's `:` wins.
    const scope = makeScope([
      { id: "ui.palette.open", keys: { cua: "Mod+K", vim: ":" } },
    ]);
    const handler = createKeyHandler("vim", executeCommand, () =>
      extractScopeBindings(scope, "vim"),
    );
    handler(fakeKeyEvent(":"));
    expect(executeCommand).toHaveBeenCalledWith("ui.palette.open");
  });
});
