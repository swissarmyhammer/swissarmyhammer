/**
 * Keybinding layer for the Tauri kanban app.
 *
 * Maps keyboard events to command IDs based on the active keymap mode
 * (vim / cua / emacs). Supports modifier combos, vim-style multi-key
 * sequences with a 500ms timeout, and skips events originating from
 * CodeMirror 6 editors.
 */

import type { KeymapMode } from "./keymap-context";
export type { KeymapMode };

/* ---------- types ---------- */

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
    "Mod+Shift+P": "app.palette",
    "u": "app.undo",
    "Mod+r": "app.redo",
    "Escape": "app.dismiss",
  },
  cua: {
    "Mod+Shift+P": "app.palette",
    "Mod+z": "app.undo",
    "Mod+Shift+Z": "app.redo",
    "Escape": "app.dismiss",
  },
  emacs: {
    "Mod+Shift+P": "app.palette",
    "Escape": "app.dismiss",
  },
};

/**
 * Multi-key sequence tables per mode. Only vim uses these currently.
 * Keyed by first key, then second key, value is command ID.
 */
const SEQUENCE_TABLES: Record<KeymapMode, SequenceTable> = {
  vim: {
    g: { g: "board.firstColumn" },
    d: { d: "task.archive" },
    z: { o: "task.toggleCollapse" },
  },
  cua: {},
  emacs: {},
};

/* ---------- normalizeKeyEvent ---------- */

/** Set of keys that are modifiers themselves and should not produce a binding. */
const MODIFIER_KEYS = new Set(["Meta", "Control", "Shift", "Alt"]);

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
 * by the key. Letter keys are uppercased when Shift is held.
 *
 * @param e - The keyboard event to normalize.
 * @returns A canonical string like "Mod+Shift+P", "Escape", ":", or null
 *          if the event is a lone modifier press.
 */
export function normalizeKeyEvent(e: KeyboardEvent): string | null {
  // Ignore lone modifier presses
  if (MODIFIER_KEYS.has(e.key)) return null;

  const mac = isMac();
  const mod = mac ? e.metaKey : e.ctrlKey;

  const parts: string[] = [];

  if (mod) parts.push("Mod");
  if (e.altKey) parts.push("Alt");

  // Only add Shift modifier for letter keys (where we uppercase the letter).
  // For punctuation produced by Shift (like ":" from Shift+;), the e.key
  // already IS the shifted character, so adding "Shift" would be redundant
  // and break binding lookups.
  let key = e.key;
  if (e.shiftKey && key.length === 1 && /[a-z]/.test(key)) {
    parts.push("Shift");
    key = key.toUpperCase();
  } else if (e.shiftKey && key.length === 1 && /[A-Z]/.test(key)) {
    // Already uppercase letter — add Shift (e.g. Mod+Shift+P)
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
 *        `useExecuteCommand()` in the command scope.
 * @returns A function suitable for `addEventListener("keydown", ...)`.
 */
export function createKeyHandler(
  mode: KeymapMode,
  executeCommand: (id: string) => Promise<boolean>,
): (e: KeyboardEvent) => void {
  const bindings = BINDING_TABLES[mode];
  const sequences = SEQUENCE_TABLES[mode];

  /** Pending first key of a multi-key sequence, or null. */
  let pending: string | null = null;
  /** Timer handle for clearing the pending buffer. */
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

    console.debug(`[keybindings] mode=${mode} key="${normalized}" target=${target?.tagName ?? "null"}`);

    // Skip non-modifier single-character keys when focus is in any editable
    // context (CodeMirror editors, inputs, textareas, contenteditable).
    // Modifier combos (Mod+Shift+P, Mod+Z, etc.) pass through because they
    // don't interfere with text editing and are also handled by the native
    // menu bar as a fallback.
    const hasModifier = normalized.includes("Mod") || normalized.includes("Alt");
    if (!hasModifier && target) {
      const tag = target.tagName;
      if (
        tag === "INPUT" ||
        tag === "TEXTAREA" ||
        tag === "SELECT" ||
        target.closest?.(".cm-editor") ||
        target.closest?.("[contenteditable]")
      ) {
        console.debug(`[keybindings] SKIPPED: editable context (${tag})`);
        return;
      }
    }

    // --- Multi-key sequence handling ---

    // If we have a pending first key, check for completion
    if (pending !== null) {
      const secondMap = sequences[pending];
      if (secondMap && normalized in secondMap) {
        const commandId = secondMap[normalized];
        console.debug(`[keybindings] SEQUENCE MATCH: "${pending}" + "${normalized}" → ${commandId}`);
        clearPending();
        e.preventDefault();
        e.stopPropagation();
        executeCommand(commandId);
        return;
      }
      // No match for the second key; clear and fall through
      console.debug(`[keybindings] SEQUENCE BROKEN: pending="${pending}" key="${normalized}"`);
      clearPending();
    }

    // Check if this key starts a multi-key sequence
    if (normalized in sequences) {
      // Only start a sequence if this key is not also a single-key binding,
      // OR if there are actual sequence completions for it.
      // We prefer sequences over single-key bindings when ambiguous.
      pending = normalized;
      pendingTimer = setTimeout(clearPending, SEQUENCE_TIMEOUT_MS);
      // Do not fire yet; wait for second key or timeout
      return;
    }

    // --- Single-key binding lookup ---

    if (normalized in bindings) {
      const cmdId = bindings[normalized];
      console.debug(`[keybindings] MATCH: "${normalized}" → ${cmdId}`);
      e.preventDefault();
      e.stopPropagation();
      executeCommand(cmdId);
      return;
    }

    console.debug(`[keybindings] NO MATCH for "${normalized}"`)
  };
}
