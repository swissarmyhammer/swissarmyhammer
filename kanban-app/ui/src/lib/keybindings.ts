/**
 * Keybinding layer for the Tauri kanban app.
 *
 * Maps keyboard events to command IDs based on the active keymap mode
 * (vim / cua / emacs). Supports modifier combos, vim-style multi-key
 * sequences with a 500ms timeout, and skips events originating from
 * CodeMirror 6 editors.
 */

/* ---------- types ---------- */

/** The three supported editor keymap modes. */
export type KeymapMode = "cua" | "vim" | "emacs";

/** A flat mapping from canonical key strings to command IDs. */
export type BindingTable = Record<string, string>;

/** Multi-key sequence entry: first key prefix maps to second-key -> command. */
type SequenceTable = Record<string, Record<string, string>>;

/* ---------- binding tables ---------- */

/**
 * Binding tables per keymap mode. Each maps a canonical key string
 * (as produced by `normalizeKeyEvent`) to a command ID.
 */
export const BINDING_TABLES: Record<KeymapMode, BindingTable> = {
  vim: {
    ":": "app.command",
    "/": "app.search",
    "Mod+f": "app.search",
    "Mod+Shift+P": "app.palette",
    u: "app.undo",
    "Mod+r": "app.redo",
    Enter: "nav.drillIn",
    Escape: "nav.drillOut",
    "Mod+w": "file.closeBoard",
  },
  cua: {
    "Mod+Shift+P": "app.palette",
    "Mod+f": "app.search",
    "Mod+z": "app.undo",
    "Mod+Shift+Z": "app.redo",
    Enter: "nav.drillIn",
    Escape: "nav.drillOut",
    "Mod+w": "file.closeBoard",
    // Tab / Shift+Tab cycle to the next / previous spatial sibling.
    // `nav.right` / `nav.left` are global commands defined in
    // `app-shell.tsx`'s `NAV_COMMAND_SPEC`; their execute closures
    // dispatch `spatial_navigate(focusedKey, "right" | "left")`. The
    // Rust kernel's cascade (iter 0: same-kind siblings, iter 1:
    // escalate to parent zone) picks the next focusable. Inside an
    // inspector the vertical layout means iter 0 finds no horizontal
    // sibling and iter 1 escalates to the panel zone — so Tab still
    // moves between fields without any inspector-scoped shadow command.
    // (Card `01KQCKVN140DGBCK8NF8RZM4R5` deleted the
    // `inspector.nextField` / `inspector.prevField` shadows that used
    // to claim Tab / Shift+Tab inside the inspector.)
    Tab: "nav.right",
    "Shift+Tab": "nav.left",
  },
  emacs: {
    "Mod+Shift+P": "app.palette",
    Enter: "nav.drillIn",
    Escape: "nav.drillOut",
    "Mod+w": "file.closeBoard",
    // Emacs navigation — Ctrl+ entries match macOS where Ctrl is distinct from
    // Cmd (Mod). Mod+ entries cover non-Mac where Ctrl normalises to Mod.
    "Ctrl+p": "nav.up",
    "Mod+p": "nav.up",
    "Ctrl+n": "nav.down",
    "Mod+n": "nav.down",
    "Ctrl+b": "nav.left",
    "Mod+b": "nav.left",
    "Ctrl+f": "nav.right",
    "Mod+f": "nav.right",
    "Alt+<": "nav.first",
    "Alt+>": "nav.last",
  },
};

/**
 * Multi-key sequence tables per mode. Only vim uses these currently.
 * Keyed by first key, then second key, value is command ID.
 */
const SEQUENCE_TABLES: Record<KeymapMode, SequenceTable> = {
  vim: {
    g: { g: "nav.first", t: "perspective.next", "Shift+T": "perspective.prev" },
    d: { d: "entity.archive" },
    z: { o: "task.toggleCollapse" },
  },
  cua: {},
  emacs: {},
};

/* ---------- normalizeKeyEvent ---------- */

/** Set of keys that are modifiers themselves and should not produce a binding. */
const MODIFIER_KEYS = new Set(["Meta", "Control", "Shift", "Alt"]);

/**
 * Symbolic keys that gain a `"Shift+"` prefix when `e.shiftKey` is true.
 *
 * Letter keys (e.key.length === 1, /[a-z]/) are handled separately —
 * they get uppercased and prefixed (e.g. `p` → `Shift+P`).
 *
 * Punctuation produced by Shift (e.g. `:` from Shift+`;`, `?` from
 * Shift+`/`) is also handled separately — `e.key` is already the
 * shifted character, so no prefix is added.
 *
 * For symbolic keys like `Tab`, `Enter`, `Escape`, the arrows, and the
 * navigation/editing block, the browser reports the same `e.key` whether
 * Shift is held or not, so without an explicit prefix the two
 * keystrokes hash to the same canonical string. This set enumerates the
 * keys that need that disambiguation.
 *
 * `Space` is in this set under its canonical name (not the literal `" "`
 * the browser delivers) — the spacebar's `e.key` is rewritten to
 * `"Space"` before the Shift-prefix check runs, so the membership test
 * sees the canonical token. This keeps the set semantically clean
 * (canonical names only) and produces `"Shift+Space"` rather than
 * `"Shift+ "` for `Shift+Space`.
 *
 * F1–F12 are included so a future binding like `Shift+F1` can be
 * registered distinctly from `F1`.
 */
const SHIFT_PREFIXED_SYMBOLIC_KEYS = new Set<string>([
  "Tab",
  "Enter",
  "Escape",
  "Space",
  "ArrowUp",
  "ArrowDown",
  "ArrowLeft",
  "ArrowRight",
  "Home",
  "End",
  "PageUp",
  "PageDown",
  "Insert",
  "Delete",
  "Backspace",
  "F1",
  "F2",
  "F3",
  "F4",
  "F5",
  "F6",
  "F7",
  "F8",
  "F9",
  "F10",
  "F11",
  "F12",
]);

/**
 * Detect whether the current platform is macOS.
 *
 * @returns true on macOS, false otherwise.
 */
function isMac(): boolean {
  return /Mac|iPhone|iPad|iPod/.test(navigator.platform);
}

/**
 * Convert a KeyboardEvent into a canonical key string.
 *
 * The canonical form uses "Mod" as the platform-aware modifier (Meta on Mac,
 * Control elsewhere). Modifiers appear in the order Mod+Alt+Shift, followed
 * by the key.
 *
 * Shift handling:
 *
 * - **Letter keys** (`a`–`z`) are uppercased and prefixed when Shift is
 *   held (`p` + Shift → `Shift+P`).
 * - **Punctuation produced by Shift** (`:` from Shift+`;`, `?` from
 *   Shift+`/`, etc.) keeps no prefix — `e.key` is already the shifted
 *   character, so adding `Shift+` would be redundant and break lookups.
 * - **Symbolic keys** in `SHIFT_PREFIXED_SYMBOLIC_KEYS` (`Tab`, `Enter`,
 *   `Escape`, `Space`, the arrows, the navigation/editing block,
 *   `F1`–`F12`) gain an explicit `Shift+` prefix when Shift is held.
 *   The browser reports the same `e.key` for these whether Shift is
 *   held or not, so the prefix is the only way to distinguish Shift+Tab
 *   from Tab in the binding tables. The spacebar (`e.key === " "`) is
 *   rewritten to the canonical `"Space"` token *before* the Shift
 *   check so the membership test sees the canonical name.
 *
 * @param e - The keyboard event to normalize.
 * @returns A canonical string like "Mod+Shift+P", "Shift+Tab",
 *          "Shift+Space", "Escape", ":", or null if the event is a
 *          lone modifier press.
 */
export function normalizeKeyEvent(e: KeyboardEvent): string | null {
  // Ignore lone modifier presses
  if (MODIFIER_KEYS.has(e.key)) return null;

  const mac = isMac();
  const mod = mac ? e.metaKey : e.ctrlKey;

  const parts: string[] = [];

  if (mod) parts.push("Mod");
  // On macOS, Ctrl is a distinct physical key from Cmd (which maps to Mod).
  // Emit a "Ctrl" prefix so emacs-style C-p / C-n bindings can be expressed.
  if (mac && e.ctrlKey) parts.push("Ctrl");
  if (e.altKey) parts.push("Alt");

  let key = e.key;

  // Browsers report the spacebar as `e.key === " "` (literal space). The
  // canonical form uses the symbolic name "Space" so command bindings can
  // declare `keys: { cua: "Space" }` without embedding a single-character
  // space literal in source — that would be invisible in code review and
  // collide with how the rest of the binding table treats whitespace.
  //
  // Rewriting before the Shift-prefix check below is load-bearing:
  // without it, `Shift+Space` would canonicalize to `"Space"` (the set
  // membership test would see `" "`, miss, and the Shift prefix would
  // never fire) — the same disambiguation bug that was fixed for Tab.
  // After the rewrite, the Shift branch sees `"Space"` and treats it
  // like any other symbolic key in `SHIFT_PREFIXED_SYMBOLIC_KEYS`.
  if (key === " ") {
    key = "Space";
  }

  // Shift modifier: applies to letter keys (where we uppercase the
  // letter) and to the symbolic keys enumerated in
  // SHIFT_PREFIXED_SYMBOLIC_KEYS. Punctuation produced by Shift (like
  // ":" from Shift+;) keeps no prefix because e.key is already the
  // shifted character.
  if (e.shiftKey && key.length === 1 && /[a-z]/.test(key)) {
    parts.push("Shift");
    key = key.toUpperCase();
  } else if (e.shiftKey && key.length === 1 && /[A-Z]/.test(key)) {
    // Already uppercase letter — add Shift (e.g. Mod+Shift+P)
    parts.push("Shift");
  } else if (e.shiftKey && SHIFT_PREFIXED_SYMBOLIC_KEYS.has(key)) {
    // Symbolic key whose `e.key` is identical with or without Shift —
    // emit an explicit Shift+ prefix so Shift+Tab is distinct from Tab.
    parts.push("Shift");
  }

  parts.push(key);
  return parts.join("+");
}

/* ---------- createKeyHandler ---------- */

/** Timeout for vim multi-key sequence buffer in milliseconds. */
const SEQUENCE_TIMEOUT_MS = 500;

/**
 * Create a keydown event handler that looks up bindings for the given
 * keymap mode and executes commands via the provided callback.
 *
 * The handler:
 * - Skips events originating inside `.cm-editor` (CM6 handles its own keys).
 * - Supports single-key and modifier-key bindings from BINDING_TABLES.
 * - Supports multi-key sequences (e.g. vim "gg", "dd", "zo") with a
 *   500ms timeout on the pending buffer.
 * - Calls `preventDefault()` and `stopPropagation()` on matched events.
 *
 * @param mode - The active keymap mode ("vim", "cua", or "emacs").
 * @param executeCommand - Callback to run a command by ID, typically from
 *        `useDispatchCommand()` in the command scope.
 * @returns A function suitable for `addEventListener("keydown", ...)`.
 */
/**
 * Create a keydown event handler that resolves keybindings and executes commands.
 *
 * Bindings come from two sources, checked in order (scope wins over global):
 * 1. **Scope bindings** — dynamic, from the focused scope's commands' `keys` property.
 *    Provided via `getScopeBindings()` callback so the handler always sees the current scope.
 * 2. **Global bindings** — static, from `BINDING_TABLES` for the active keymap mode.
 *
 * @param mode - The active keymap mode ("vim", "cua", or "emacs").
 * @param executeCommand - Callback to run a command by ID.
 * @param getScopeBindings - Returns key→commandId bindings from the focused scope.
 */
/**
 * Check if a key event targets an editable context that should not trigger
 * global keybindings (inputs, textareas, CM6 editors, contenteditable).
 */
function isEditableTarget(
  normalized: string,
  target: HTMLElement | null,
): boolean {
  const hasModifier =
    normalized.includes("Mod") ||
    normalized.includes("Alt") ||
    normalized.includes("Ctrl");
  if (hasModifier || !target) return false;
  const tag = target.tagName;
  return (
    tag === "INPUT" ||
    tag === "TEXTAREA" ||
    tag === "SELECT" ||
    !!target.closest?.(".cm-editor") ||
    !!target.closest?.("[contenteditable]")
  );
}

/**
 * Create the key event handler for the active keymap mode.
 *
 * Bindings come from two sources, checked in order (scope wins over global):
 * 1. **Scope bindings** — dynamic, from the focused scope's commands' `keys` property.
 *    Provided via `getScopeBindings()` callback so the handler always sees the current scope.
 * 2. **Global bindings** — static, from `BINDING_TABLES` for the active keymap mode.
 *
 * @param mode - The active keymap mode ("vim", "cua", or "emacs").
 * @param executeCommand - Callback to run a command by ID.
 * @param getScopeBindings - Returns key→commandId bindings from the focused scope.
 */
export function createKeyHandler(
  mode: KeymapMode,
  executeCommand: (id: string) => Promise<boolean>,
  getScopeBindings?: () => BindingTable,
): (e: KeyboardEvent) => void {
  const globalBindings = BINDING_TABLES[mode];
  const sequences = SEQUENCE_TABLES[mode];

  let pending: string | null = null;
  let pendingTimer: ReturnType<typeof setTimeout> | null = null;

  /** Clear the pending buffer and cancel the timeout. */
  function clearPending(): void {
    pending = null;
    if (pendingTimer !== null) {
      clearTimeout(pendingTimer);
      pendingTimer = null;
    }
  }

  return (e: KeyboardEvent) => {
    const target = e.target as HTMLElement | null;
    const normalized = normalizeKeyEvent(e);
    if (normalized === null) return;

    if (isEditableTarget(normalized, target)) return;

    const bindings = { ...globalBindings, ...(getScopeBindings?.() ?? {}) };

    // Multi-key sequence handling
    if (pending !== null) {
      const secondMap = sequences[pending];
      if (secondMap && normalized in secondMap) {
        clearPending();
        e.preventDefault();
        e.stopPropagation();
        executeCommand(secondMap[normalized]);
        return;
      }
      clearPending();
    }
    if (normalized in sequences) {
      pending = normalized;
      pendingTimer = setTimeout(clearPending, SEQUENCE_TIMEOUT_MS);
      return;
    }

    // Single-key binding lookup
    if (normalized in bindings) {
      e.preventDefault();
      e.stopPropagation();
      executeCommand(bindings[normalized]);
    }
  };
}

/**
 * Extract key bindings from a command scope chain for a given keymap mode.
 *
 * Walks the scope chain and collects `keys[mode]` from every command,
 * producing a flat key→commandId binding table. Inner (deeper) scopes
 * shadow outer scopes for the same key.
 *
 * @param scope - The scope to start from (typically the focused scope).
 * @param mode - The keymap mode to extract bindings for.
 * @returns A flat BindingTable mapping canonical key strings to command IDs.
 */
export function extractScopeBindings(
  scope: {
    commands: Map<string, { id: string; keys?: Record<string, string> }>;
    parent: unknown;
  } | null,
  mode: KeymapMode,
): BindingTable {
  const result: BindingTable = {};
  let current = scope;
  while (current !== null) {
    for (const [, cmd] of current.commands) {
      const key = cmd.keys?.[mode];
      if (key && !(key in result)) {
        // Inner scopes win — only set if not already claimed
        result[key] = cmd.id;
      }
    }
    current = current.parent as typeof scope;
  }
  return result;
}
