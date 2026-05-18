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
 * # Streaming state gates `ai.cancel`
 *
 * `ai.cancel` is available only while the conversation is streaming. The
 * conversation reports its turn status here via {@link setAiStreaming}; the
 * window-layer command builder reads {@link aiStreaming} (and re-renders via
 * {@link subscribeAiStreaming}) so the command's `available` flag tracks the
 * live conversation. The same flag is mirrored into the backend `UIState` so
 * `commands_for_scope` keeps the palette entry hidden when idle.
 */

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

/** Whether the AI conversation is currently streaming a turn. */
let streaming = false;

/** Subscribers notified whenever {@link streaming} changes. */
const streamingSubscribers = new Set<() => void>();

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

/** Whether the AI conversation is currently streaming a turn. */
export function aiStreaming(): boolean {
  return streaming;
}

/**
 * Report the AI conversation's streaming status.
 *
 * Called by `AiPanelConversation` whenever the ACP turn status crosses the
 * streaming boundary. Notifies every {@link subscribeAiStreaming} subscriber
 * on a real change so the window-layer command builder rebuilds `ai.cancel`
 * with the fresh `available` flag. A no-op when the value is unchanged.
 *
 * @param next - The new streaming flag.
 */
export function setAiStreaming(next: boolean): void {
  if (streaming === next) return;
  streaming = next;
  for (const notify of streamingSubscribers) notify();
}

/**
 * Subscribe to {@link aiStreaming} changes.
 *
 * @param onChange - Invoked after every real streaming-flag change.
 * @returns An unsubscribe function.
 */
export function subscribeAiStreaming(onChange: () => void): () => void {
  streamingSubscribers.add(onChange);
  return () => {
    streamingSubscribers.delete(onChange);
  };
}

/**
 * Reset the registry to its initial state.
 *
 * Test-only — clears every handler, the streaming flag, and all subscribers
 * so one test's registrations never leak into the next.
 */
export function resetAiCommandsForTest(): void {
  for (const key of Object.keys(handlers)) {
    delete (handlers as Record<string, unknown>)[key];
  }
  streaming = false;
  streamingSubscribers.clear();
}
