/**
 * The AI panel command registry — the seam between the window-layer `ai.*`
 * commands and the AI panel React tree.
 *
 * # Why a module-level registry
 *
 * The `ai.toggle` / `ai.focus` / `ai.newChat` / `ai.model` / `ai.cancel`
 * commands are registered at the **window layer** (`AppShell`'s global command
 * scope) so their keybindings fire app-wide — even when focus is on a board
 * card outside the AI panel. But the behaviour they drive lives deep in the AI
 * panel tree: the open-state in `AiPanelContainer`, the conversation
 * (`newConversation` / `cancel` / `status`) in `AiPanelConversation`, the
 * model selection in `AiPanelContainer`, and the prompt-input focus in the
 * `AiPanel` View.
 *
 * A module-level registry bridges the two: the AI panel components register
 * their live handlers here on mount, and the window-layer command `execute`
 * closures call into it. This mirrors `perspective-tab-bar.tsx`'s
 * `triggerStartRename` module bus — the established pattern for a
 * window-layer command whose effect lives in a sibling subtree.
 *
 * # Turn status — gates `ai.cancel` and feeds the bottom bar
 *
 * The conversation reports its full ACP turn status here via
 * {@link setAiStatus} — `idle`, `streaming`, or `error`. Two consumers read
 * it back:
 *
 * - `ai.cancel` is available only while the conversation is streaming. That
 *   gate is owned entirely frontend-side: the window-layer command builder
 *   (`app-shell.tsx`'s `buildAiCommands`) reads {@link aiStreaming} (derived
 *   from the status) and re-renders via {@link subscribeAiStreaming}, so the
 *   command's `available` flag tracks the live conversation — governing both
 *   the React-scope palette (`collectAvailableCommands`) and the keybinding
 *   (`resolveCommand` no-ops a blocked command). The registry-driven palette
 *   entry from the `ai-commands` builtin plugin carries no `available`
 *   callback (the plugin isolate has no synchronous view of this webview-only
 *   flag), so it shows as always-available there; the frontend gate is the
 *   authoritative one. (Historical note: this flag used to be mirrored into
 *   the backend `UIState.ai_streaming` for a Rust `AiCancelCmd::available()`
 *   check, but that command was retired in the ai.yaml → plugin migration.)
 * - The app's bottom bar (`ModeIndicator`) reads {@link aiStatus} and
 *   re-renders via {@link subscribeAiStatus} to show `idle` / `streaming` /
 *   `error` next to the keymap mode.
 *
 * The status store is the single source of truth; the streaming boolean is a
 * derived view of it (`status === "streaming"`). Both share one subscriber
 * set, so a status change notifies streaming subscribers and vice versa.
 */

import type { ConversationStatus } from "@/ai/conversation";

/**
 * The live handlers the AI panel components register.
 *
 * Each is `undefined` until the owning component mounts and registers it. A
 * command `execute` that fires before its handler is registered is a silent
 * no-op — the AI panel simply is not mounted yet.
 */
export interface AiCommandHandlers {
  /** Show/hide the AI panel — flips `AiPanelContainer`'s open-state. */
  toggle?: () => void;
  /** Move keyboard focus into the AI panel's prompt input. */
  focus?: () => void;
  /** Start a fresh stateless ACP session, clearing the conversation. */
  newChat?: () => void;
  /** Apply a model id as the per-board AI model selection. */
  setModel?: (modelId: string) => void;
  /** Cancel the in-flight generation (a no-op when none is running). */
  cancel?: () => void;
}

/** The current handler set. Mutated in place by {@link registerAiCommandHandlers}. */
const handlers: AiCommandHandlers = {};

/**
 * The AI conversation's current turn status — `idle`, `streaming`, or
 * `error`. The single source of truth for both the `ai.cancel` availability
 * gate and the bottom-bar AI status indicator. Defaults to `idle`: no
 * conversation has run a turn yet.
 */
let status: ConversationStatus = "idle";

/**
 * Subscribers notified whenever {@link status} changes.
 *
 * Shared by {@link subscribeAiStatus} and {@link subscribeAiStreaming}: the
 * streaming boolean is a derived view of the status, so a status change is
 * also a streaming change as far as subscribers are concerned.
 */
const statusSubscribers = new Set<() => void>();

/**
 * Register (or update) the AI panel command handlers.
 *
 * Called by the AI panel components as they mount. Only the keys present in
 * `partial` are replaced, so `AiPanelContainer` and `AiPanelConversation` can
 * each register the subset of handlers they own without clobbering the other.
 *
 * @param partial - The handler(s) to install.
 * @returns A cleanup function that clears exactly the handlers this call
 *   installed — call it on unmount so a stale closure never lingers.
 */
export function registerAiCommandHandlers(
  partial: AiCommandHandlers,
): () => void {
  const keys = Object.keys(partial) as (keyof AiCommandHandlers)[];
  for (const key of keys) {
    // The assignment is type-correct per key; the loop just erases the
    // per-key narrowing, so one localized cast keeps the call sites clean.
    (handlers as Record<string, unknown>)[key] = partial[key];
  }
  return () => {
    for (const key of keys) {
      // Only clear a slot this call still owns — a later registration of the
      // same key (e.g. a remount) must not be wiped by an older cleanup.
      if (handlers[key] === partial[key]) {
        delete handlers[key];
      }
    }
  };
}

/** Run the `ai.toggle` handler, if the AI panel is mounted. */
export function triggerAiToggle(): void {
  handlers.toggle?.();
}

/** Run the `ai.focus` handler, if the AI panel is mounted. */
export function triggerAiFocus(): void {
  handlers.focus?.();
}

/** Run the `ai.newChat` handler, if the AI panel is mounted. */
export function triggerAiNewChat(): void {
  handlers.newChat?.();
}

/** Run the `ai.cancel` handler, if the AI panel is mounted. */
export function triggerAiCancel(): void {
  handlers.cancel?.();
}

/**
 * Run the `ai.model` handler with the chosen model id.
 *
 * A no-op when no `model` arg is supplied or the panel is not mounted.
 *
 * @param modelId - The model id to select.
 */
export function triggerAiModel(modelId: string | undefined): void {
  if (modelId) {
    handlers.setModel?.(modelId);
  }
}

/**
 * The AI conversation's current turn status — `idle`, `streaming`, or
 * `error`.
 *
 * Read by the bottom bar (`ModeIndicator`) to render the AI status indicator.
 * `useSyncExternalStore(subscribeAiStatus, aiStatus, aiStatus)` keeps a React
 * component in sync with it.
 */
export function aiStatus(): ConversationStatus {
  return status;
}

/** Whether the AI conversation is currently streaming a turn. */
export function aiStreaming(): boolean {
  return status === "streaming";
}

/**
 * Report the AI conversation's turn status.
 *
 * Called by `AiPanelConversation` whenever the ACP turn status changes.
 * Notifies every {@link subscribeAiStatus} / {@link subscribeAiStreaming}
 * subscriber on a real change so the window-layer command builder rebuilds
 * `ai.cancel` with the fresh `available` flag and the bottom bar repaints the
 * AI status indicator. A no-op when the status is unchanged.
 *
 * @param next - The new turn status.
 */
export function setAiStatus(next: ConversationStatus): void {
  if (status === next) return;
  status = next;
  for (const notify of statusSubscribers) notify();
}

/**
 * Report the AI conversation's streaming status as a boolean.
 *
 * A back-compat shim over {@link setAiStatus}: `true` maps to `streaming`,
 * `false` to `idle`. Callers that have the full {@link ConversationStatus}
 * (notably `AiPanelConversation`) should call {@link setAiStatus} directly so
 * the `error` state is not flattened away.
 *
 * @param next - `true` for streaming, `false` for idle.
 */
export function setAiStreaming(next: boolean): void {
  setAiStatus(next ? "streaming" : "idle");
}

/**
 * Subscribe to {@link aiStatus} changes.
 *
 * @param onChange - Invoked after every real status change.
 * @returns An unsubscribe function.
 */
export function subscribeAiStatus(onChange: () => void): () => void {
  statusSubscribers.add(onChange);
  return () => {
    statusSubscribers.delete(onChange);
  };
}

/**
 * Subscribe to {@link aiStreaming} changes.
 *
 * The streaming boolean is derived from the status, so this is an alias of
 * {@link subscribeAiStatus} — both register into the one subscriber set.
 *
 * @param onChange - Invoked after every real status change.
 * @returns An unsubscribe function.
 */
export function subscribeAiStreaming(onChange: () => void): () => void {
  return subscribeAiStatus(onChange);
}

/**
 * Reset the registry to its initial state.
 *
 * Test-only — clears every handler, the turn status, and all subscribers so
 * one test's registrations never leak into the next.
 */
export function resetAiCommandsForTest(): void {
  for (const key of Object.keys(handlers)) {
    delete (handlers as Record<string, unknown>)[key];
  }
  status = "idle";
  statusSubscribers.clear();
}
