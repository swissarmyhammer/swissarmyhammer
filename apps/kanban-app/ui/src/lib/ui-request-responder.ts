/**
 * The host→UI request responder bus — the webview half of the generic
 * host→UI request/reply channel.
 *
 * Host→UI traffic was otherwise fire-and-forget; the host now has a reply path
 * (see `apps/kanban-app/src/ui_request.rs`). The host:
 *
 *   1. emits a `ui/request` Tauri event `{ request_id, kind, params }` to a
 *      specific window and awaits a reply correlated by `request_id`;
 *   2. this bus's listener dispatches on `kind` to the responder registered
 *      for it, computes a result, and replies via
 *      `invoke("ui_request_reply", { requestId, result })`.
 *
 * # Why a module-level registry keyed by kind
 *
 * A responder answers a category of host question (e.g. `focus.geometry` — a
 * live layout read the host cannot do itself). Like the webview command bus
 * (`webview-command-bus.ts`), the live closure lives deep in a React subtree
 * the listener cannot reach directly, so a component registers its responder
 * here on mount against the `kind` it answers. The ownership-guarded cleanup
 * is the same StrictMode / HMR double-mount guard used there.
 *
 * # Reply invariant — always reply
 *
 * Every `ui/request` produces exactly one `ui_request_reply`, even for a
 * `kind` no responder is registered for (the reply value is `null`). The host
 * await would otherwise hang until its timeout for any unhandled kind; a
 * prompt `null` lets the host distinguish "no answer" from "slow answer".
 */

/**
 * A host→UI responder: answers a `kind` of host question.
 *
 * Receives the request `params` (the host's question payload) and returns the
 * answer, synchronously or as a promise. The return value is sent back to the
 * host verbatim as the reply `result`.
 */
export type UiResponder = (params: unknown) => unknown | Promise<unknown>;

/**
 * The shape the host emits on the `ui/request` Tauri event.
 *
 * Snake_case mirrors the host-side `serde_json::Value` payload built in
 * `ui_request.rs` (emitted as-is, no serde rename).
 */
export interface UiRequest {
  /** Correlation id the reply must echo back. */
  request_id: string;
  /** Which responder answers this request. */
  kind: string;
  /** The host's question payload, passed to the responder. */
  params: unknown;
  /**
   * The window the host targeted with `emit_to`. The bus answers ONLY when
   * this matches the current window's label. In a multi-window app every
   * window's global `listen` receives the event regardless of `emit_to`
   * target, so without this guard a non-target window can reply first (often
   * `null`) and win the host's request correlation. Absent (legacy/host
   * without the field) → answer unconditionally.
   */
  window?: string;
}

/** The Tauri event the host raises to ask the webview a question. */
export const UI_REQUEST_EVENT = "ui/request" as const;

/**
 * The live responder set, keyed by request `kind`.
 *
 * A slot is present only while the owning component is mounted and has
 * registered it; an absent slot means the kind is unhandled and the bus
 * replies `null`.
 */
const responders = new Map<string, UiResponder>();

/**
 * Register (or replace) the responder for a request `kind`.
 *
 * Called by the component that can answer this kind as it mounts.
 *
 * @param kind - The request kind this responder answers.
 * @param responder - The function that computes the reply from the params.
 * @returns A cleanup that clears the slot only if this call still owns it —
 *   call it on unmount so a stale closure never lingers, and so a later
 *   registration of the same kind (a StrictMode / HMR remount) is not wiped by
 *   an older cleanup.
 */
export function registerUiResponder(
  kind: string,
  responder: UiResponder,
): () => void {
  responders.set(kind, responder);
  return () => {
    if (responders.get(kind) === responder) {
      responders.delete(kind);
    }
  };
}

/**
 * The Tauri `invoke` signature this bus depends on.
 *
 * Narrowed to the one command the bus calls so {@link handleUiRequest} can be
 * unit-tested with a stub instead of the real `@tauri-apps/api/core` import.
 */
type InvokeFn = (
  cmd: string,
  args: Record<string, unknown>,
) => Promise<unknown>;

/**
 * Handle one host `ui/request`: dispatch by `kind`, then reply by `request_id`.
 *
 * Looks up the responder for `request.kind`, awaits its result (or uses `null`
 * when no responder is registered), and replies via the `ui_request_reply`
 * command. Exported so tests can drive the dispatch/reply logic without the
 * Tauri event round-trip; production wires it to the real `invoke` in
 * {@link initUiResponders}.
 *
 * @param request - The decoded `ui/request` payload.
 * @param invoke - The Tauri `invoke` used to send the reply.
 */
export async function handleUiRequest(
  request: UiRequest,
  invoke: InvokeFn,
  currentWindow?: string,
): Promise<void> {
  // Window scoping: the host targets one window via `emit_to`, but every
  // window's global `listen` receives the event. Answer ONLY when the request
  // targets THIS window, so a non-target window can't reply first (with null)
  // and win the correlation. When the request carries no `window` (legacy) or
  // we don't know our own label, fall through and answer.
  if (
    request.window !== undefined &&
    currentWindow !== undefined &&
    request.window !== currentWindow
  ) {
    return;
  }
  const responder = responders.get(request.kind);
  let result: unknown = null;
  if (responder) {
    result = await responder(request.params);
  } else {
    console.warn(
      `[ui-request-responder] no responder for kind "${request.kind}"; replying null`,
    );
  }
  // camelCase `requestId` maps to the Rust `request_id` param (Tauri's arg
  // convention — see `get_entity_schema`'s `entityType`).
  await invoke("ui_request_reply", {
    requestId: request.request_id,
    result,
  });
}

/**
 * Wire the host→UI request channel: listen for `ui/request` and reply.
 *
 * Call once at app start. Lazily imports the Tauri event + core APIs (mirroring
 * `mcp-notifications.ts`) so this module's static graph stays free of them for
 * importers that only register responders.
 *
 * @returns A promise resolving to an unlisten function.
 */
export async function initUiResponders(): Promise<() => void> {
  const [{ listen }, { invoke }, { getCurrentWindow }] = await Promise.all([
    import("@tauri-apps/api/event"),
    import("@tauri-apps/api/core"),
    import("@tauri-apps/api/window"),
  ]);
  // This window's own label, used to ignore `ui/request` events the host
  // targeted at OTHER windows but that this window's global listener still
  // receives (see `handleUiRequest`'s window guard).
  const currentWindow = getCurrentWindow().label;
  return listen<UiRequest>(UI_REQUEST_EVENT, (event) => {
    void handleUiRequest(event.payload, invoke, currentWindow).catch((err) => {
      console.error(
        `[ui-request-responder] failed handling ${UI_REQUEST_EVENT}:`,
        err,
      );
    });
  });
}

/**
 * Reset the bus to its initial state.
 *
 * Test-only — clears every registration so one test's responders never leak
 * into the next.
 */
export function resetUiRespondersForTest(): void {
  responders.clear();
}
