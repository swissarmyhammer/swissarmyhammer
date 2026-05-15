/**
 * Centralised toast surfacing for command-dispatch failures.
 *
 * The `useDispatchCommand` hook re-throws backend errors as plain
 * Promise rejections so each call site can decide what to do — some
 * sites already catch the rejection to show in-place feedback (e.g.
 * `useAddTaskHandler` in `board-view.tsx` toasts a contextual
 * "Failed to add task: …" message). Sites that *don't* catch — the
 * keyboard handler, the native-menu listener, and the context-menu
 * listener — would otherwise let the error vanish into an unhandled
 * promise rejection. {@link reportDispatchError} is the single hook
 * those generic dispatch entry points use to surface the failure as a
 * visible toast that names the failing command and the backend's
 * message.
 *
 * Why a separate module instead of inlining in `command-scope.tsx`:
 * keeping the toast policy here lets the test pin its behaviour
 * (`dispatch-error.test.ts`) without spinning up the full
 * `useDispatchCommand` provider tree, and lets non-React callers
 * (e.g. menu listeners) reuse the same wording.
 */

import { toast } from "sonner";

/**
 * Convert a thrown backend-dispatch error into a user-visible toast.
 *
 * The backend wraps every command failure as `Command failed: <msg>`
 * (see `dispatch_command_internal` in `kanban-app/src/commands.rs`).
 * This helper strips that wrapper, names the command that failed, and
 * forwards the cleaned-up message to {@link toast.error}.
 *
 * @param cmdId - The command identifier the user (or a binding) tried
 *                to dispatch. Embedded in the toast title so the user
 *                can correlate the failure to the action they took
 *                (Ctrl+V → `entity.paste`, etc.).
 * @param err   - The value thrown / rejected by the dispatch call.
 *                `Error` instances and plain strings are both handled
 *                — Tauri's `invoke` typically rejects with a string
 *                payload, but a JS-side throw produces an `Error`.
 */
export function reportDispatchError(cmdId: string, err: unknown): void {
  const raw = err instanceof Error ? err.message : String(err);
  // Strip the backend's "Command failed: " prefix so the toast text
  // doesn't duplicate the framing we add ourselves below. The prefix
  // is added by the Tauri wrapper for telemetry and is not useful to
  // the end user.
  const cleaned = raw.replace(/^Command failed:\s*/, "");
  toast.error(`${cmdId} failed: ${cleaned}`);
}
