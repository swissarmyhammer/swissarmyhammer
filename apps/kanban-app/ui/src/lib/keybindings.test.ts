import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import {
  normalizeKeyEvent,
  BINDING_TABLES,
  createKeyHandler,
  extractChainBindings,
  extractKeymapBindings,
  type BindingScope,
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
    // but e.fq is already ":", not ";". Shift should NOT be added.
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

  it("canonicalises spacebar (e.fq === ' ') to 'Space'", () => {
    // Browsers report the spacebar as a literal single space
    // (`e.fq === " "`). The binding tables and command-keys speak
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
    // The browser delivers the spacebar as `e.fq === " "` (literal
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
  // Insert/Delete/Backspace, F1–F12) report the same `e.fq` whether
  // Shift is held or not — the only signal is `e.shiftKey`. Without an
  // explicit `Shift+` prefix, Shift+Tab and Tab hash to the same
  // canonical string and cannot bind to distinct commands. The
  // normalizer prepends `Shift+` for these keys when shiftKey is true.
  // Letter keys keep the existing uppercase-and-prefix behaviour;
  // punctuation produced by Shift (`:`, `?`, etc.) keeps no prefix
  // because `e.fq` is already the shifted character.

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
    // Real keyboard: `?` is Shift+`/`, but `e.fq` is already `?`.
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
    // The palette opener is the unified `app.palette.open` (folded from the
    // old `ui.palette.open`). Its vim `:` binding rides on the plugin command
    // metadata (resolved by `extractKeymapBindings`), so the static table no
    // longer carries a `:` entry — that would only duplicate the dynamic
    // binding. `Mod+Shift+P` is not in the command's `keys`, so it stays a
    // static binding, now pointing at the unified id.
    expect(vim[":"]).toBeUndefined();
    expect(vim["Mod+Shift+P"]).toBe("app.palette.open");
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
    expect(cua["Mod+Shift+P"]).toBe("app.palette.open");
    expect(cua["Mod+z"]).toBe("app.undo");
    expect(cua["Mod+Shift+Z"]).toBe("app.redo");
    // See vim notes — Escape is now `nav.drillOut`, which falls
    // through to `app.dismiss` on a null kernel result.
    expect(cua["Escape"]).toBe("nav.drillOut");
    expect(cua["Enter"]).toBe("nav.drillIn");
    // Tab / Shift+Tab cycle siblings via global `nav.right` / `nav.left`.
    // Card `01KQCKVN140DGBCK8NF8RZM4R5` deleted the
    // `inspector.nextField` / `inspector.prevField` shadows that used
    // to claim Tab / Shift+Tab inside the inspector — under the
    // unified-nav contract, the kernel cascade (iter 0 → iter 1)
    // routes Tab to the next field zone naturally without a per-context
    // shadow command.
    expect(cua["Tab"]).toBe("nav.right");
    expect(cua["Shift+Tab"]).toBe("nav.left");
  });

  it("emacs bindings include expected commands", () => {
    const emacs = BINDING_TABLES.emacs;
    expect(emacs["Mod+Shift+P"]).toBe("app.palette.open");
    expect(emacs["Escape"]).toBe("nav.drillOut");
    expect(emacs["Enter"]).toBe("nav.drillIn");
  });

  it("every keymap binds the AI panel commands consistently", () => {
    // The window-layer `ai.*` commands are registered in `app-shell.tsx`'s
    // global scope; their `BINDING_TABLES` entries cover the no-focus case
    // where `extractChainBindings` yields nothing. All three keymaps bind the
    // same keys — there is no per-keymap divergence for the AI panel.
    // `ai.model` is intentionally key-less (it takes a `model` arg).
    for (const mode of ["vim", "cua", "emacs"] as const) {
      const table = BINDING_TABLES[mode];
      expect(table["Mod+j"], `${mode} Mod+j`).toBe("ai.toggle");
      expect(table["Mod+i"], `${mode} Mod+i`).toBe("ai.focus");
      expect(table["Mod+Shift+J"], `${mode} Mod+Shift+J`).toBe("ai.newChat");
      expect(table["Mod+."], `${mode} Mod+.`).toBe("ai.cancel");
    }
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
    // The palette opener's vim `:` binding is no longer a static entry — it
    // rides on the `app.palette.open` plugin command metadata, surfaced to the
    // global layer via `extractKeymapBindings`. Pass that dynamic binding as
    // the handler's `globalBindings` to prove `:` still resolves (now to the
    // unified `app.palette.open`).
    const handler = createKeyHandler("vim", executeCommand, undefined, {
      ":": "app.palette.open",
    });
    handler(fakeKeyEvent(":"));
    expect(executeCommand).toHaveBeenCalledWith("app.palette.open");
  });

  /* ---------- scope bindings ---------- */

  it("dispatches scope bindings when getScopeBindings is provided", () => {
    // `field.edit` (cua: Enter) is a real example of a scope-level
    // binding the focused inspector field zone surfaces. Card
    // `01KQCKVN140DGBCK8NF8RZM4R5` deleted the inspector nav shadows;
    // card `01KQCTJY1QZ710A05SE975GHNR` then deleted the inspector edit
    // commands and the `<InspectorFocusBridge>` itself, leaving
    // `field.edit` as the only scope-level edit-mode command in the
    // inspector.
    const scopeBindings = () => ({ Enter: "field.edit" });
    const handler = createKeyHandler("cua", executeCommand, scopeBindings);
    handler(fakeKeyEvent("Enter"));
    expect(executeCommand).toHaveBeenCalledWith("field.edit");
  });

  it("scope bindings shadow global bindings for the same key", () => {
    // Escape is globally `nav.drillOut` (which itself falls through
    // to `app.dismiss` on a null kernel result) — scope can still
    // claim it back when a focused subtree wants its own Escape
    // semantics. `dialog.cancel` is a representative example.
    const scopeBindings = () => ({ Escape: "dialog.cancel" });
    const handler = createKeyHandler("cua", executeCommand, scopeBindings);
    handler(fakeKeyEvent("Escape"));
    expect(executeCommand).toHaveBeenCalledWith("dialog.cancel");
  });

  it("falls through to global bindings when scope has no match", () => {
    // Scope claims Enter via `field.edit`; pressing Escape falls
    // through to the global `nav.drillOut` binding because the scope
    // has no Escape entry.
    const scopeBindings = () => ({ Enter: "field.edit" });
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

    // No scope binding yet — Enter resolves through the global keymap
    // to `nav.drillIn`. We're testing the dynamic-update path, so just
    // assert no scope-specific binding fires before bindings are seeded.
    handler(fakeKeyEvent("Enter"));
    // Global `nav.drillIn: Enter` fires. (See the next assertion for
    // the post-seed shadowing.)
    expect(executeCommand).toHaveBeenCalledWith("nav.drillIn");
    (executeCommand as ReturnType<typeof vi.fn>).mockClear();

    // Simulate inspector field focus — `field.edit` now claims Enter
    // and shadows the global `nav.drillIn`.
    bindings = { Enter: "field.edit" };
    handler(fakeKeyEvent("Enter"));
    expect(executeCommand).toHaveBeenCalledWith("field.edit");
  });

  it("vim scope bindings dispatch field edit commands", () => {
    // The surviving inspector-field edit-mode commands declare vim keys:
    //   - `field.edit`      — vim `i` (also cua `Enter`)
    //   - `field.editEnter` — vim `Enter`
    // Nav commands (`nav.up`, `nav.down`, …) are global, not scope-
    // bound — vim `j`/`k` resolve through `BINDING_TABLES.vim` and the
    // `nav-commands` builtin plugin's catalogue, never through scope.
    const scopeBindings = () => ({
      i: "field.edit",
      Enter: "field.editEnter",
    });
    const handler = createKeyHandler("vim", executeCommand, scopeBindings);

    handler(fakeKeyEvent("i"));
    expect(executeCommand).toHaveBeenCalledWith("field.edit");

    handler(fakeKeyEvent("Enter"));
    expect(executeCommand).toHaveBeenCalledWith("field.editEnter");
  });
});

/* ---------- extractChainBindings — component-def walk ---------- */

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

/**
 * Build a def-less moniker chain (innermost first) — the chain shape the
 * keymap layer sees when only zone-marker monikers gate bindings, with no
 * component `CommandDef`s contending.
 */
function monikerChain(monikers: readonly string[]): BindingScope | null {
  let chain: BindingScope | null = null;
  for (let i = monikers.length - 1; i >= 0; i--) {
    chain = { commands: new Map(), moniker: monikers[i], parent: chain };
  }
  return chain;
}

// The component-def layer of the depth-interleaved walk: with no registry
// commands, `extractChainBindings([], mode, scope)` collects `keys[mode]`
// from every scope's component defs, innermost-first with first-key-wins.
describe("extractChainBindings — component defs only", () => {
  // The card `01KQCKVN140DGBCK8NF8RZM4R5` deleted the six
  // `inspector.move{Up,Down,ToFirst,ToLast}` and `inspector.{nextField,
  // prevField}` commands; card `01KQCTJY1QZ710A05SE975GHNR` then
  // deleted the inspector edit commands and the
  // `<InspectorFocusBridge>` itself. Under the unified-nav contract
  // every Up/Down/Left/Right/Home/End/Tab/Shift+Tab resolves through
  // the global keymap in `BINDING_TABLES.cua` plus the `nav-commands`
  // builtin plugin's catalogue — there are no inspector-scoped nav
  // variants.
  // Edit-mode entry on the focused inspector field zone is owned by
  // the field-zone-scoped commands `field.edit` / `field.editEnter`.

  it("extracts keys for the given mode", () => {
    const scope = makeScope([
      { id: "field.edit", keys: { vim: "i", cua: "Enter" } },
      { id: "ui.entity.startRename", keys: { vim: "F2", cua: "F2" } },
    ]);
    const bindings = extractChainBindings([], "cua", scope);
    expect(bindings).toEqual({
      Enter: "field.edit",
      F2: "ui.entity.startRename",
    });
  });

  it("extracts vim keys", () => {
    const scope = makeScope([
      { id: "field.edit", keys: { vim: "i", cua: "Enter" } },
    ]);
    const bindings = extractChainBindings([], "vim", scope);
    expect(bindings).toEqual({ i: "field.edit" });
  });

  it("skips commands without keys", () => {
    const scope = makeScope([
      { id: "field.edit", keys: { vim: "i" } },
      { id: "field.dummy" }, // no keys — should be skipped
    ]);
    const bindings = extractChainBindings([], "vim", scope);
    expect(bindings).toEqual({ i: "field.edit" });
  });

  it("skips commands without keys for the requested mode", () => {
    const scope = makeScope([
      { id: "field.editEnter", keys: { vim: "Enter" } }, // no cua key
    ]);
    const bindings = extractChainBindings([], "cua", scope);
    expect(bindings).toEqual({});
  });

  it("inner scope shadows outer scope for same key", () => {
    // Inner field-zone scope claims Enter for `field.edit`; an outer
    // hypothetical scope also claims Enter for some other command. The
    // innermost wins — pressing Enter on a focused field zone fires
    // `field.edit` (which drills into pills first, then opens the
    // editor on a leaf field).
    const outer = makeScope([{ id: "outer.handler", keys: { cua: "Enter" } }]);
    const inner = makeScope(
      [{ id: "field.edit", keys: { cua: "Enter" } }],
      outer,
    );
    const bindings = extractChainBindings([], "cua", inner);
    expect(bindings["Enter"]).toBe("field.edit");
  });

  it("includes parent scope commands not shadowed by inner", () => {
    const outer = makeScope([{ id: "app.dismiss", keys: { cua: "Escape" } }]);
    const inner = makeScope(
      [{ id: "field.edit", keys: { cua: "Enter" } }],
      outer,
    );
    const bindings = extractChainBindings([], "cua", inner);
    expect(bindings).toEqual({
      Enter: "field.edit",
      Escape: "app.dismiss",
    });
  });

  it("returns empty for null scope", () => {
    expect(extractChainBindings([], "cua", null)).toEqual({});
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
  // that surfaces the command, `extractChainBindings` must return
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
    const bindings = extractChainBindings([], "cua", scope);
    expect(bindings).toEqual({ Enter: "ui.entity.startRename" });
  });

  it("ui.entity.startRename surfaces Enter on a perspective scope (vim)", () => {
    const scope = makeScope([
      {
        id: "ui.entity.startRename",
        keys: { cua: "Enter", vim: "Enter", emacs: "Enter" },
      },
    ]);
    const bindings = extractChainBindings([], "vim", scope);
    expect(bindings).toEqual({ Enter: "ui.entity.startRename" });
  });

  it("ui.entity.startRename surfaces Enter on a perspective scope (emacs)", () => {
    const scope = makeScope([
      {
        id: "ui.entity.startRename",
        keys: { cua: "Enter", vim: "Enter", emacs: "Enter" },
      },
    ]);
    const bindings = extractChainBindings([], "emacs", scope);
    expect(bindings).toEqual({ Enter: "ui.entity.startRename" });
  });

  it("ui.entity.startRename's Enter shadows a parent nav.drillIn: Enter", () => {
    // The global drill-in binding lives at the AppShell root — a perspective
    // scope that registers `ui.entity.startRename: Enter` must shadow it so
    // Enter inside the perspective scope chain triggers rename, not drill-in.
    // `extractChainBindings` walks innermost-first with first-key-wins
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
    const bindings = extractChainBindings([], "cua", inner);
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
//   - `app.palette.open`   — cua `Mod+K`, vim `:`
//
// Cross-cutting commands auto-emit into the scope chain for every entity
// moniker, so their `keys` get wired up through `extractChainBindings` when a
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
    // task's scope; `extractChainBindings` pulls it out.
    const scope = makeScope([{ id: "entity.archive", keys: { vim: "dd" } }]);
    const handler = createKeyHandler("vim", executeCommand, () =>
      extractChainBindings([], "vim", scope),
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
      extractChainBindings([], "vim", scope),
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
    // entity scope; `extractChainBindings` pulls it out.
    const scope = makeScope([
      { id: "entity.delete", keys: { cua: "Mod+Backspace" } },
    ]);

    withMacPlatform(() => {
      const handler = createKeyHandler("cua", executeCommand, () =>
        extractChainBindings([], "cua", scope),
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
        extractChainBindings([], "cua", scope),
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
        extractChainBindings([], "cua", scope),
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
        extractChainBindings([], "cua", scope),
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
      extractChainBindings([], "vim", scope),
    );
    handler(fakeKeyEvent("y"));
    expect(executeCommand).toHaveBeenCalledWith("entity.copy");
  });

  it("vim: x dispatches entity.cut from a task scope", () => {
    const scope = makeScope([
      { id: "entity.cut", keys: { cua: "Mod+X", vim: "x" } },
    ]);
    const handler = createKeyHandler("vim", executeCommand, () =>
      extractChainBindings([], "vim", scope),
    );
    handler(fakeKeyEvent("x"));
    expect(executeCommand).toHaveBeenCalledWith("entity.cut");
  });

  it("vim: p dispatches entity.paste from a task scope", () => {
    const scope = makeScope([
      { id: "entity.paste", keys: { cua: "Mod+V", vim: "p" } },
    ]);
    const handler = createKeyHandler("vim", executeCommand, () =>
      extractChainBindings([], "vim", scope),
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
      extractChainBindings([], "cua", scope),
    );
    handler(fakeKeyEvent("Escape"));
    expect(executeCommand).toHaveBeenCalledWith("ui.inspector.close");
  });

  it("vim: q dispatches ui.inspector.close from an inspector scope", () => {
    const scope = makeScope([
      { id: "ui.inspector.close", keys: { cua: "Escape", vim: "q" } },
    ]);
    const handler = createKeyHandler("vim", executeCommand, () =>
      extractChainBindings([], "vim", scope),
    );
    handler(fakeKeyEvent("q"));
    expect(executeCommand).toHaveBeenCalledWith("ui.inspector.close");
  });

  it("cua: Mod+K dispatches app.palette.open when a scope claims it", () => {
    // `app.palette.open` declares `keys.cua: Mod+K`. If the focused scope
    // surfaces the command, the binding resolves and `Mod+K` fires it. The
    // production BINDING_TABLES uses `Mod+Shift+P` for the palette; this
    // test pins the scope-bound path independently.
    const scope = makeScope([
      { id: "app.palette.open", keys: { cua: "Mod+K", vim: ":" } },
    ]);

    withMacPlatform(() => {
      const handler = createKeyHandler("cua", executeCommand, () =>
        extractChainBindings([], "cua", scope),
      );
      // `Mod+K` in the YAML has an uppercase K. `normalizeKeyEvent` only
      // adds `Shift+` when `shiftKey` is truly held, so to produce the
      // canonical `Mod+K` we fake a `key: "K"` event with just metaKey —
      // mirroring the character the OS hands us when the physical `k` key
      // is pressed together with a shift-style chord target (Caps Lock or a
      // pre-uppercased key map).
      handler(fakeKeyEvent("K", { metaKey: true }));
      expect(executeCommand).toHaveBeenCalledWith("app.palette.open");
    });
  });

  it("vim: : dispatches app.palette.open from a scope that claims it", () => {
    // The static global table no longer binds `:` (the palette opener's `:`
    // is sourced dynamically from the `app.palette.open` command metadata).
    // This test pins the scope-bound path: a focused scope that surfaces
    // `app.palette.open` with `keys.vim: ":"` claims `:`.
    const scope = makeScope([
      { id: "app.palette.open", keys: { cua: "Mod+K", vim: ":" } },
    ]);
    const handler = createKeyHandler("vim", executeCommand, () =>
      extractChainBindings([], "vim", scope),
    );
    handler(fakeKeyEvent(":"));
    expect(executeCommand).toHaveBeenCalledWith("app.palette.open");
  });
});

/* ---------- Escape = nav.drillOut (full production resolution) ---------- */

// These pin the load-bearing claim of card
// `01KTPDTH772HSEV5F7R1DKYDNJ`: with the production wiring — the global
// keybinding layer built from the metadata-driven Command registry
// (`extractKeymapBindings(registryCommands, mode)`) merged under the focused
// scope chain (`extractChainBindings(registryCommands, mode, focusedScope)`) — Escape must
// resolve to `nav.drillOut`, NOT `app.dismiss` and NOT `ui.inspector.close`.
//
// The fixtures mirror the ACTUAL plugin sources after the fix:
//   - `app.ts` `app.dismiss` carries no Escape key.
//   - `ui-commands/index.ts` `ui.inspector.close` keeps only vim `q`.
//   - `app-shell.tsx` registers no static global defs at all (Card I deleted
//     `STATIC_GLOBAL_COMMANDS` outright), so the root command scope surfaces
//     no Escape binding to shadow the global `nav.drillOut`.
// To stay an honest RED-first guard, the fixtures here are kept in lockstep
// with those sources: a regression that re-adds an Escape key to any of the
// three legacy bindings would make this `expect` fail.
describe("Escape resolves to nav.drillOut (production registry + scope wiring)", () => {
  let executeCommand: (id: string) => Promise<boolean>;

  beforeEach(() => {
    executeCommand = vi.fn(async () => true) as (
      id: string,
    ) => Promise<boolean>;
  });

  /**
   * The Escape-bearing slice of the live command registry, in the order
   * `useCommandList` returns it. After the fix `nav.drillOut` is the sole
   * Escape owner; `app.dismiss` carries no key and `ui.inspector.close` keeps
   * only its vim `q`.
   */
  const REGISTRY = [
    {
      id: "nav.drillOut",
      name: "Drill Out",
      keys: { cua: "Escape", vim: "Escape", emacs: "Escape" },
    },
    { id: "ui.inspector.close", name: "Close Inspector", keys: { vim: "q" } },
    { id: "app.dismiss", name: "Dismiss" },
    {
      id: "ui.inspector.close_all",
      name: "Close All",
      keys: { cua: "Mod+Escape", vim: "Q" },
    },
  ] as const;

  for (const mode of ["cua", "vim", "emacs"] as const) {
    it(`${mode}: Escape resolves to nav.drillOut from the global registry layer`, () => {
      const globalBindings = extractKeymapBindings([...REGISTRY], mode);
      // The registry global layer must bind Escape to nav.drillOut and to
      // nothing else.
      expect(globalBindings["Escape"]).toBe("nav.drillOut");

      const handler = createKeyHandler(
        mode,
        executeCommand,
        undefined,
        globalBindings,
      );
      handler(fakeKeyEvent("Escape"));
      expect(executeCommand).toHaveBeenCalledWith("nav.drillOut");
    });
  }

  it("cua: Escape resolves to nav.drillOut with the root scope focused (no app.dismiss shadow)", () => {
    // The root command scope (`globalCommands` in app-shell.tsx) must NOT
    // carry an `app.dismiss` Escape binding — that scope-level binding beat
    // the global `nav.drillOut` (scope wins over global in `createKeyHandler`,
    // which merges `{...global, ...scope}`). The fix removes `app.dismiss`
    // from `STATIC_GLOBAL_COMMANDS`, so the root scope surfaces no Escape
    // binding and Escape falls through to the global `nav.drillOut`.
    const rootScope = makeScope([
      { id: "app.palette.open", keys: { cua: "Mod+K", vim: ":" } },
      { id: "app.undo", keys: { cua: "Mod+z", vim: "u" } },
      { id: "entity.inspect", keys: { cua: "Space", vim: "Space" } },
      // No app.dismiss — the legacy Escape scope binding is gone.
    ]);
    const globalBindings = extractKeymapBindings([...REGISTRY], "cua");
    const handler = createKeyHandler(
      "cua",
      executeCommand,
      () => extractChainBindings([], "cua", rootScope),
      globalBindings,
    );
    handler(fakeKeyEvent("Escape"));
    expect(executeCommand).toHaveBeenCalledWith("nav.drillOut");
  });
});

/* ---------- Enter = nav.drillIn regardless of registry order ---------- */

// Third-window drill regression (card 01KTQ6QZNB3VN4MAND7VPASM21).
//
// The Command service's `list command` returns commands in UNSPECIFIED order
// (`CommandRegistry::list()`: "Order is unspecified … callers that need a
// stable order must sort"). Each per-board plugin runtime owns its own
// registry instance, so each board gets its own iteration order. TWO registry
// commands declare an Enter key: the GLOBAL `nav.drillIn` and the SCOPE-GATED
// `ui.entity.startRename` (scope `["entity:perspective"]`, ui-commands). With
// first-id-wins extraction and no scope awareness, whichever id happened to
// iterate first claimed Enter — drill-in worked in the two windows sharing
// one board runtime and silently died in the third window (a DIFFERENT board,
// therefore a different runtime whose order put `ui.entity.startRename`
// first; that id resolves to the root-scope client-side `triggerStartRename`
// and never reaches the backend, so the live log showed no `nav.drillIn`
// dispatch at all for that window).
//
// The contract pinned here: a command carrying a non-empty `scope` filter
// contributes NO global keybinding — its keys apply only through the
// focused-scope walk (`extractChainBindings`), exactly as the ui-commands
// source comments intend ("The scope filter keeps Enter from claiming
// nav.drillIn on board/column/card focus"). Global key ownership is therefore
// order-independent.
describe("Enter resolves to nav.drillIn regardless of registry order (third-window regression)", () => {
  /** The Enter-bearing registry slice in the THIRD window's adverse order:
   * the scope-gated rename command iterates before the global drill. */
  const ADVERSE_ORDER = [
    {
      id: "ui.entity.startRename",
      name: "Rename Perspective",
      scope: ["entity:perspective"],
      keys: { cua: "Enter", vim: "Enter", emacs: "Enter" },
    },
    {
      id: "nav.drillIn",
      name: "Drill In",
      keys: { vim: "Enter", cua: "Enter", emacs: "Enter" },
    },
  ] as const;

  /** The same slice in the order the two working windows happened to see. */
  const FAVORABLE_ORDER = [ADVERSE_ORDER[1], ADVERSE_ORDER[0]] as const;

  for (const mode of ["cua", "vim", "emacs"] as const) {
    it(`${mode}: Enter binds to nav.drillIn even when the scoped rename command iterates first`, () => {
      const bindings = extractKeymapBindings([...ADVERSE_ORDER], mode);
      expect(bindings["Enter"]).toBe("nav.drillIn");
    });
  }

  it("global extraction is registry-order independent", () => {
    for (const mode of ["cua", "vim", "emacs"] as const) {
      expect(extractKeymapBindings([...ADVERSE_ORDER], mode)).toEqual(
        extractKeymapBindings([...FAVORABLE_ORDER], mode),
      );
    }
  });

  it("an empty scope list is global — its keys still bind", () => {
    // `scope: []` means global (mirrors the service's list_filter_matches:
    // None | Some([]) both match every scope filter).
    const bindings = extractKeymapBindings(
      [{ id: "nav.drillIn", scope: [], keys: { cua: "Enter" } }],
      "cua",
    );
    expect(bindings["Enter"]).toBe("nav.drillIn");
  });
});

/* ---------- extractChainBindings — zone-gated registry keys ---------- */

// Card C (grid-commands plugin): the `grid.*` commands are DEFINED by the
// `grid-commands` builtin plugin with `scope: ["ui:grid"]` and no client-side
// `CommandDef` carries their keys anymore. Their keybindings therefore come
// from the registry metadata, gated on the focused scope chain containing the
// zone's LITERAL moniker (`ui:grid`). Entity-typed scope expressions
// (`entity:task` etc.) intentionally do NOT light up from the registry alone:
// their keys stay component-registered (the React `CommandDef` the owning
// component mounts, e.g. `task.untag` on a tag pill), because an entity-typed
// match would bind keys for behaviors the focused component never wired.
// The chains here are def-less moniker chains (`monikerChain`), so only the
// registry layer of the depth-interleaved walk contends.
describe("extractChainBindings — zone-gated registry keys", () => {
  const REGISTRY: {
    id: string;
    name: string;
    scope?: string[];
    keys?: Record<string, string>;
  }[] = [
    {
      id: "grid.moveToRowStart",
      name: "Row Start",
      scope: ["ui:grid"],
      keys: { vim: "0", cua: "Home" },
    },
    {
      id: "grid.edit",
      name: "Edit Cell",
      scope: ["ui:grid"],
      keys: { vim: "i", cua: "Enter" },
    },
    {
      id: "task.untag",
      name: "Untag",
      scope: ["entity:tag", "entity:task"],
      keys: { vim: "x", cua: "Delete" },
    },
    {
      id: "nav.drillIn",
      name: "Drill In",
      keys: { vim: "Enter", cua: "Enter", emacs: "Enter" },
    },
  ];

  /** The focused chain when a grid cell is focused: cell → row entity →
   * grid zone → window. Contains the literal `ui:grid` zone moniker AND a
   * `task:`-typed entity moniker (the row). */
  const GRID_CHAIN = ["grid_cell:0:title", "task:t1", "ui:grid", "window:main"];

  it("binds a zone-literal scoped command's keys when the zone moniker is in the chain", () => {
    const cua = extractChainBindings(REGISTRY, "cua", monikerChain(GRID_CHAIN));
    expect(cua["Home"]).toBe("grid.moveToRowStart");
    expect(cua["Enter"]).toBe("grid.edit");
    const vim = extractChainBindings(REGISTRY, "vim", monikerChain(GRID_CHAIN));
    expect(vim["0"]).toBe("grid.moveToRowStart");
    expect(vim["i"]).toBe("grid.edit");
  });

  it("contributes nothing when the zone moniker is not in the chain", () => {
    const bindings = extractChainBindings(
      REGISTRY,
      "cua",
      monikerChain(["task:t1", "column:todo", "window:main"]),
    );
    expect(bindings).toEqual({});
  });

  it("does not expand entity-typed scope expressions (component-registered keys stay component-owned)", () => {
    // The chain contains `task:t1`, which would admit `entity:task` under the
    // context-menu expansion — but the keymap layer must NOT light up
    // `task.untag`'s keys from the registry alone: pressing Delete on a grid
    // row would otherwise untag through a behavior the grid never wired.
    const bindings = extractChainBindings(
      REGISTRY,
      "cua",
      monikerChain(GRID_CHAIN),
    );
    expect(bindings["Delete"]).toBeUndefined();
  });

  it("global (unscoped) commands contribute nothing — they own the global table", () => {
    const bindings = extractChainBindings(
      REGISTRY,
      "vim",
      monikerChain(GRID_CHAIN),
    );
    expect(Object.values(bindings)).not.toContain("nav.drillIn");
  });

  it("an empty chain yields no bindings", () => {
    expect(extractChainBindings(REGISTRY, "cua", monikerChain([]))).toEqual({});
  });
});

/* ---------- scoped registry bindings shadow the global table ---------- */

// End-to-end through `createKeyHandler`: with `ui:grid` in the focused chain,
// the scoped registry binding for Enter (grid.edit) must shadow the global
// `nav.drillIn: Enter` — the same shadowing the retired React `CommandDef`s
// provided via `extractChainBindings`.
describe("scoped registry bindings shadow globals inside the zone", () => {
  const REGISTRY: {
    id: string;
    name: string;
    scope?: string[];
    keys?: Record<string, string>;
  }[] = [
    {
      id: "grid.edit",
      name: "Edit Cell",
      scope: ["ui:grid"],
      keys: { vim: "i", cua: "Enter" },
    },
    {
      id: "nav.drillIn",
      name: "Drill In",
      keys: { vim: "Enter", cua: "Enter", emacs: "Enter" },
    },
  ];

  it("Enter dispatches grid.edit inside the grid, nav.drillIn outside", () => {
    const exec = vi.fn(async () => true);
    const globalBindings = extractKeymapBindings(REGISTRY, "cua");

    // Inside the grid: scoped bindings merge over the global table.
    const inside = createKeyHandler(
      "cua",
      exec,
      () => extractChainBindings(REGISTRY, "cua", monikerChain(["ui:grid"])),
      globalBindings,
    );
    inside(fakeKeyEvent("Enter"));
    expect(exec).toHaveBeenCalledWith("grid.edit");

    // Outside the grid: the global drill keeps Enter.
    exec.mockClear();
    const outside = createKeyHandler(
      "cua",
      exec,
      () => extractChainBindings(REGISTRY, "cua", monikerChain(["task:t1"])),
      globalBindings,
    );
    outside(fakeKeyEvent("Enter"));
    expect(exec).toHaveBeenCalledWith("nav.drillIn");
  });
});

/* ---------- extractChainBindings — depth-interleaved chain walk ---------- */

// Card D (ui-commands plugin UI-surface commands): the two binding layers —
// component-registered `CommandDef`s and scope-gated registry commands — must
// resolve as ONE inner-first walk over the focused chain, not as two flat
// layers where every component def beats every registry binding. The failure
// the interleave prevents: a focused `<Pressable>` sits inside an
// `<Inspectable>` whose scope-level `entity.inspect` def claims Space; the
// registry's `pressable.activateSpace` (gated on the pressable's INNER
// `ui:pressable` marker) must win Space, exactly as the retired leaf-level
// `CommandDef` did. Conversely an inner component def must keep beating an
// outer-matched registry command (inner knowledge beats catalogue metadata at
// equal-or-shallower depth).
describe("extractChainBindings", () => {
  interface ChainScope {
    commands: Map<string, { id: string; keys?: Record<string, string> }>;
    moniker?: string;
    parent: ChainScope | null;
  }

  /** Build one chain node with optional moniker + component defs. */
  function chainNode(
    moniker: string | undefined,
    commands: Array<{ id: string; keys?: Record<string, string> }>,
    parent: ChainScope | null,
  ): ChainScope {
    const map = new Map<
      string,
      { id: string; keys?: Record<string, string> }
    >();
    for (const cmd of commands) map.set(cmd.id, cmd);
    return { commands: map, moniker, parent };
  }

  const REGISTRY: {
    id: string;
    name: string;
    scope?: string[];
    keys?: Record<string, string>;
  }[] = [
    {
      id: "pressable.activate",
      name: "Activate",
      scope: ["ui:pressable"],
      keys: { vim: "Enter", cua: "Enter" },
    },
    {
      id: "pressable.activateSpace",
      name: "Activate (Space)",
      scope: ["ui:pressable"],
      keys: { cua: "Space" },
    },
    {
      id: "field.edit",
      name: "Edit Field",
      scope: ["ui:field"],
      keys: { vim: "i", cua: "Enter" },
    },
    {
      id: "task.untag",
      name: "Untag",
      scope: ["entity:task"],
      keys: { cua: "Delete" },
    },
    {
      id: "nav.drillIn",
      name: "Drill In",
      keys: { vim: "Enter", cua: "Enter", emacs: "Enter" },
    },
  ];

  /** The production shape for a focused pressable nested in an Inspectable:
   * leaf scope (no defs) → `ui:pressable` marker → Inspectable scope (Space
   * def) → root scope (Space def + ai keys). */
  function pressableInInspectableChain(): ChainScope {
    const root = chainNode(
      undefined,
      [
        { id: "entity.inspect", keys: { vim: "Space", cua: "Space" } },
        { id: "ai.toggle", keys: { cua: "Mod+j" } },
      ],
      null,
    );
    const inspectable = chainNode(
      undefined,
      [{ id: "entity.inspect", keys: { vim: "Space", cua: "Space" } }],
      root,
    );
    const marker = chainNode("ui:pressable", [], inspectable);
    return chainNode("ui:column.add-task:c1", [], marker);
  }

  it("an inner-matched registry binding beats an outer component def for the same key", () => {
    const bindings = extractChainBindings(
      REGISTRY,
      "cua",
      pressableInInspectableChain(),
    );
    // Space: the pressable marker (depth 1) beats the Inspectable's
    // entity.inspect def (depth 2) and the root's (depth 3).
    expect(bindings["Space"]).toBe("pressable.activateSpace");
    // Enter: only the registry contends — pressable.activate wins.
    expect(bindings["Enter"]).toBe("pressable.activate");
    // Unshadowed outer component defs still contribute.
    expect(bindings["Mod+j"]).toBe("ai.toggle");
  });

  it("an inner component def beats an outer-matched registry binding for the same key", () => {
    // A pill leaf with its own Enter def inside a field zone: the pill's def
    // (depth 0) must beat the registry's field.edit matched at the `ui:field`
    // marker (depth 2).
    const marker = chainNode("ui:field", [], null);
    const fieldZone = chainNode("field:task:t1.tags", [], marker);
    const pill = chainNode(
      "mention:tag:bug",
      [{ id: "pill.open", keys: { cua: "Enter" } }],
      fieldZone,
    );
    const bindings = extractChainBindings(REGISTRY, "cua", pill);
    expect(bindings["Enter"]).toBe("pill.open");
  });

  it("matches scope expressions by literal chain moniker only (no entity-typed expansion)", () => {
    // `task:t1` admits `entity:task` in the context menu's expansion, but the
    // keymap layer must not light up component-owned keys from the registry.
    const chain = chainNode("task:t1", [], null);
    const bindings = extractChainBindings(REGISTRY, "cua", chain);
    expect(bindings["Delete"]).toBeUndefined();
  });

  it("global (unscoped) registry commands contribute nothing — they own the global table", () => {
    const bindings = extractChainBindings(
      REGISTRY,
      "emacs",
      pressableInInspectableChain(),
    );
    expect(Object.values(bindings)).not.toContain("nav.drillIn");
  });

  it("a null scope yields no bindings", () => {
    expect(extractChainBindings(REGISTRY, "cua", null)).toEqual({});
  });

  it("with no registry commands it degrades to the pure component-def walk", () => {
    const chain = pressableInInspectableChain();
    // Only the component defs contribute: the Inspectable's Space and the
    // root's Mod+j — no registry-gated pressable keys.
    expect(extractChainBindings([], "cua", chain)).toEqual({
      Space: "entity.inspect",
      "Mod+j": "ai.toggle",
    });
  });

  it("field zone chain: Enter and vim i bind field.edit; vim Enter binds field.editEnter", () => {
    const fullRegistry = [
      ...REGISTRY,
      {
        id: "field.editEnter",
        name: "Edit Field (Enter)",
        scope: ["ui:field"],
        keys: { vim: "Enter" },
      },
    ];
    const marker = chainNode("ui:field", [], null);
    const fieldZone = chainNode("field:task:t1.title", [], marker);
    const cua = extractChainBindings(fullRegistry, "cua", fieldZone);
    expect(cua["Enter"]).toBe("field.edit");
    const vim = extractChainBindings(fullRegistry, "vim", fieldZone);
    expect(vim["i"]).toBe("field.edit");
    expect(vim["Enter"]).toBe("field.editEnter");
  });
});
