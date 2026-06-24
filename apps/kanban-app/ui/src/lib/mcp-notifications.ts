/**
 * The webview's single seam onto the MCP **change-notification** planes.
 *
 * Prior work landed `subscribeCommandsChanged` in `mcp-transport.ts` for the
 * `notifications/commands/changed` plane. This module is the sibling for the
 * three data/state planes the UI consumes as a pure MCP client — the same
 * normalized stream an external AI agent receives (see
 * `swissarmyhammer-plugin::notify`):
 *
 *   - `notifications/store/changed` — the one generic data-change schema for
 *     entities (field-level `changes`), views, and perspectives (reload-item).
 *   - `notifications/store/undo_changed` — Undo/Redo control state.
 *   - `notifications/ui_state/changed` — ephemeral per-window UI state.
 *
 * The host re-broadcasts each MCP notification as a Tauri event whose name is
 * the notification `method`, exactly as it already does for
 * `notifications/commands/changed`. This module wraps those `listen(...)` calls
 * so React code never imports the Tauri event API for change events, and — for
 * the data plane — owns the **transaction-batching** contract.
 *
 * ## Transaction batching (the headline contract)
 *
 * A single command (or an undo of one) produces N `store/changed`
 * notifications that all share one `txn`. The webview must apply them as ONE
 * atomic state update so a multi-write command re-renders once, not N times.
 * {@link subscribeStoreChanged} buffers notifications keyed on `txn` and flushes
 * the buffer as a single batch:
 *
 *   - the buffer flushes on a microtask tick (so a synchronous burst of
 *     same-`txn` notifications coalesces into one batch), and
 *   - a notification carrying a *different* `txn` than the one currently
 *     buffered flushes the pending batch first, then starts a new one — so two
 *     transactions never merge even if they arrive in the same tick.
 *
 * Notifications with no `txn` (`txn: null` — a change made outside any
 * transaction) are not coalesced: each flushes on its own as a singleton
 * batch.
 */

import type { ChangeOp } from "@/types/mcp-notifications";

/** The Tauri event the host raises for `notifications/store/changed`. */
export const STORE_CHANGED_EVENT = "notifications/store/changed" as const;
/** The Tauri event the host raises for `notifications/store/undo_changed`. */
export const STORE_UNDO_CHANGED_EVENT =
  "notifications/store/undo_changed" as const;
/** The Tauri event the host raises for `notifications/ui_state/changed`. */
export const UI_STATE_CHANGED_EVENT = "notifications/ui_state/changed" as const;
/** The Tauri event the host raises for `notifications/focus/changed`. */
export const FOCUS_CHANGED_EVENT = "notifications/focus/changed" as const;
/** The Tauri event the host raises for `notifications/ui_state/drag_started`. */
export const DRAG_STARTED_EVENT =
  "notifications/ui_state/drag_started" as const;
/** The Tauri event the host raises for `notifications/ui_state/drag_cancelled`. */
export const DRAG_CANCELLED_EVENT =
  "notifications/ui_state/drag_cancelled" as const;
/** The Tauri event the host raises for `notifications/ui_state/drag_completed`. */
export const DRAG_COMPLETED_EVENT =
  "notifications/ui_state/drag_completed" as const;

/** A single field-level change inside a `store/changed` notification. */
export interface StoreFieldChange {
  /** The field that changed. */
  field: string;
  /** The field's new value; `null` signals removal of the field. */
  value: unknown;
}

/**
 * One `notifications/store/changed` notification's params.
 *
 * Mirrors `swissarmyhammer-plugin::notify::McpNotification::store_changed`:
 * `store` names the store ("task", "tag", "view", "perspective", …); `item`
 * is the item id; `op` is the change kind. `changes` is present for entity
 * stores (field-level diff) and absent for views/perspectives (reload-item).
 */
export interface StoreChanged {
  /** The store name — entity type, or "view" / "perspective". */
  store: string;
  /** The changed item's id. */
  item: string;
  /** The change kind. */
  op: ChangeOp;
  /** Field-level diff (entities only); absent → reload the item. */
  changes?: StoreFieldChange[];
  /** The transaction id grouping a command's writes; `null` outside a txn. */
  txn: string | null;
  /** Provenance: "user", "agent:<id>", "undo", "redo", "watcher". */
  origin: string;
}

/** One `notifications/store/undo_changed` notification's params. */
export interface UndoChanged {
  /** Whether an undo is currently possible. */
  can_undo: boolean;
  /** Whether a redo is currently possible. */
  can_redo: boolean;
  /** Label of the entry at the top of the undo stack, if any. */
  undo_label?: string | null;
  /** Label of the entry at the top of the redo stack, if any. */
  redo_label?: string | null;
}

/**
 * One `notifications/ui_state/changed` notification's params.
 *
 * Mirrors the declared `UiStateChanged` payload struct in
 * `swissarmyhammer-ui-state` (`crates/swissarmyhammer-ui-state/src/operations.rs`):
 * a `kind` discriminator naming which UI-state slice changed plus `state`, the
 * full per-window-keyed UI-state snapshot after the change. A consumer
 * self-selects the slice it cares about (the webview's `UIStateProvider` reads
 * only its own `windows[<label>]`). Provenance (`txn`/`origin`) is stamped on
 * top by the publish path but is not consumed here.
 */
export interface UiStateChanged {
  /**
   * Which UI-state slice changed — one discriminator per backend
   * `UiStateChange` variant (e.g. "palette_open", "keymap_mode",
   * "inspector_stack", "perspective_switch").
   */
  kind: string;
  /** The full UI-state snapshot after the change. */
  state: unknown;
}

/**
 * One `notifications/focus/changed` notification's params.
 *
 * Mirrors the declared `FocusChanged` payload struct in `swissarmyhammer-focus`
 * (`crates/swissarmyhammer-focus/src/operations.rs`), whose fields ARE the
 * kernel's `FocusChangedEvent`: the window the move belongs to plus the
 * fully-qualified monikers on either side of the transition (and the new
 * focus's trailing segment). Field names are snake_case to match Rust's serde
 * defaults. Provenance (`txn`/`origin`) is stamped on top by the publish path
 * but is not consumed here.
 *
 * Structurally identical to `FocusChangedPayload` in `@/types/spatial`; the
 * `SpatialFocusProvider` consumes this stream as that type. Kept as a local
 * declaration so this module stays decoupled from the spatial type graph,
 * mirroring the other `subscribe*` payload interfaces here.
 */
export interface FocusChanged {
  /** Window in which focus changed. */
  window_label: string;
  /** Previously focused fully-qualified moniker, or `null`. */
  prev_fq: string | null;
  /** Newly focused fully-qualified moniker, or `null` when focus is cleared. */
  next_fq: string | null;
  /** Trailing segment of the newly focused FQM, or `null`. */
  next_segment: string | null;
}

/**
 * One `notifications/ui_state/drag_started` notification's params.
 *
 * Mirrors the declared `DragStarted` payload struct in `swissarmyhammer-ui-state`
 * (`crates/swissarmyhammer-ui-state/src/operations.rs`) — the full cross-window
 * drag session wire shape, structurally the `DragSession` the drag context
 * consumes. The drag state machine lives in the `ui_state` service, so the
 * lifecycle is declared and published there. Provenance (`txn`/`origin`) is
 * stamped on top by the publish path but is not consumed here.
 */
export interface DragStarted {
  /** Unique session id (ULID) for the drag. */
  session_id: string;
  /** Filesystem path of the source board (empty for file drags). */
  source_board_path: string;
  /** Tauri window label of the source window. */
  source_window_label: string;
  /** The dragged task id (empty for file drags). */
  task_id: string;
  /** Serialized source entity field snapshot for ghost preview. */
  task_fields: Record<string, unknown>;
  /** Whether Alt/Option was held (copy mode). */
  copy_mode: boolean;
  /** When the session started (epoch millis). */
  started_at_ms: number;
  /** Discriminated-union drag source mirroring the Rust `DragSource` enum. */
  from: unknown;
}

/** One `notifications/ui_state/drag_cancelled` notification's params. */
export interface DragCancelled {
  /** The cancelled session's id (ULID). */
  session_id: string;
}

/** One `notifications/ui_state/drag_completed` notification's params. */
export interface DragCompleted {
  /** The completed session's id (ULID). */
  session_id: string;
  /** Whether the drop's side-effects (transfer / flush) succeeded. */
  success: boolean;
}

/** A batch of `store/changed` notifications that share one `txn`. */
export type StoreChangeBatch = StoreChanged[];

/**
 * Lazily import the Tauri event API.
 *
 * Kept lazy (mirroring {@link subscribeCommandsChanged} in `mcp-transport.ts`)
 * so this module's static graph does not pull in `@tauri-apps/api/event` for
 * importers that only need the types.
 */
function importListen() {
  return import("@tauri-apps/api/event").then((m) => m.listen);
}

/**
 * Subscribe to the `store/changed` plane with transaction batching.
 *
 * `onBatch` is invoked once per transaction with every same-`txn`
 * notification collected so far. Consumers apply the whole batch in a single
 * state update so one command (or one undo) re-renders exactly once. See the
 * module docs for the precise flush rules.
 *
 * @param onBatch - Receives one coalesced batch of same-`txn` notifications.
 * @returns A promise resolving to an unsubscribe function.
 */
export function subscribeStoreChanged(
  onBatch: (batch: StoreChangeBatch) => void,
): Promise<() => void> {
  const batcher = new TxnBatcher(onBatch);
  let disposed = false;
  return importListen()
    .then((listen) =>
      listen<StoreChanged>(STORE_CHANGED_EVENT, (event) => {
        batcher.push(event.payload);
      }),
    )
    .then((unlisten) => () => {
      disposed = true;
      batcher.dispose();
      unlisten();
    })
    .catch((err) => {
      // A transport hiccup must not crash the subscriber; degrade to a no-op
      // unsubscribe so the caller's cleanup stays uniform.
      console.error(`[mcp-notifications] ${STORE_CHANGED_EVENT} failed:`, err);
      return () => {
        disposed = true;
        batcher.dispose();
      };
    })
    .then((unsub) => (disposed ? (batcher.dispose(), () => {}) : unsub));
}

/**
 * Coalesces `store/changed` notifications into one batch per transaction.
 *
 * Exported for unit tests that drive the batching logic directly without the
 * Tauri event round-trip; production code goes through
 * {@link subscribeStoreChanged}.
 */
export class TxnBatcher {
  private pending: StoreChanged[] = [];
  /** The `txn` the pending buffer belongs to (`null` = an un-txned singleton). */
  private pendingTxn: string | null | undefined = undefined;
  private flushScheduled = false;
  private disposed = false;

  constructor(private readonly onBatch: (batch: StoreChangeBatch) => void) {}

  /** Feed one notification into the batcher. */
  push(note: StoreChanged): void {
    if (this.disposed) return;

    // An un-txned change is never coalesced: flush any pending batch, then
    // emit this one on its own immediately.
    if (note.txn == null) {
      this.flush();
      this.onBatch([note]);
      return;
    }

    // A different txn than the one buffered must not merge — flush first.
    if (this.pending.length > 0 && note.txn !== this.pendingTxn) {
      this.flush();
    }

    this.pending.push(note);
    this.pendingTxn = note.txn;
    this.scheduleFlush();
  }

  /** Flush the pending batch now, if any. */
  flush(): void {
    if (this.pending.length === 0) return;
    const batch = this.pending;
    this.pending = [];
    this.pendingTxn = undefined;
    this.onBatch(batch);
  }

  /** Drop any pending batch without emitting it. */
  dispose(): void {
    this.disposed = true;
    this.pending = [];
    this.pendingTxn = undefined;
  }

  /** Schedule a microtask flush so a synchronous burst coalesces. */
  private scheduleFlush(): void {
    if (this.flushScheduled) return;
    this.flushScheduled = true;
    queueMicrotask(() => {
      this.flushScheduled = false;
      if (!this.disposed) this.flush();
    });
  }
}

/**
 * Subscribe to the `store/undo_changed` plane (Undo/Redo control state).
 *
 * @param onChanged - Receives the new undo/redo availability + labels.
 * @returns A promise resolving to an unsubscribe function.
 */
export function subscribeUndoChanged(
  onChanged: (state: UndoChanged) => void,
): Promise<() => void> {
  return importListen()
    .then((listen) =>
      listen<UndoChanged>(STORE_UNDO_CHANGED_EVENT, (event) =>
        onChanged(event.payload),
      ),
    )
    .catch((err) => {
      console.error(
        `[mcp-notifications] ${STORE_UNDO_CHANGED_EVENT} failed:`,
        err,
      );
      return () => {};
    });
}

/**
 * Subscribe to the `ui_state/changed` plane (ephemeral per-window UI state).
 *
 * @param onChanged - Receives the changed UI-state key/value (+ window).
 * @returns A promise resolving to an unsubscribe function.
 */
export function subscribeUiStateChanged(
  onChanged: (change: UiStateChanged) => void,
): Promise<() => void> {
  return importListen()
    .then((listen) =>
      listen<UiStateChanged>(UI_STATE_CHANGED_EVENT, (event) =>
        onChanged(event.payload),
      ),
    )
    .catch((err) => {
      console.error(
        `[mcp-notifications] ${UI_STATE_CHANGED_EVENT} failed:`,
        err,
      );
      return () => {};
    });
}

/**
 * Subscribe to the `focus/changed` plane (per-window spatial focus moves).
 *
 * The webview consumes spatial focus as a pure MCP client: the host
 * re-broadcasts each `notifications/focus/changed` bridge notification as the
 * Tauri event named by its method, scoped to the originating window.
 *
 * This is the public, lazy seam for plugins and other MCP-client consumers of
 * the focus plane (mirroring {@link subscribeUiStateChanged}). The
 * `SpatialFocusProvider` does NOT use this helper — it `listen`s for
 * {@link FOCUS_CHANGED_EVENT} directly so its handler registers synchronously
 * on mount (the spatial test harness fires focus events immediately and a
 * deferred dynamic-import registration would miss them). Both target the same
 * event name, so a consumer here sees exactly what the provider sees.
 *
 * @param onChanged - Receives the changed focus payload (window + prev/next FQM).
 * @returns A promise resolving to an unsubscribe function.
 */
export function subscribeFocusChanged(
  onChanged: (change: FocusChanged) => void,
): Promise<() => void> {
  return importListen()
    .then((listen) =>
      listen<FocusChanged>(FOCUS_CHANGED_EVENT, (event) =>
        onChanged(event.payload),
      ),
    )
    .catch((err) => {
      console.error(`[mcp-notifications] ${FOCUS_CHANGED_EVENT} failed:`, err);
      return () => {};
    });
}

/**
 * Subscribe to a single bridge-forwarded notification plane by its method name,
 * forwarding each notification's `params` to `onPayload`.
 *
 * The shared seam behind the drag-lifecycle subscribers: each `listen`s for the
 * bridge event named by the notification `method`, never a legacy direct Tauri
 * event, and degrades a transport hiccup to a no-op unsubscribe so the caller's
 * cleanup stays uniform.
 */
function subscribeBridgeEvent<T>(
  eventName: string,
  onPayload: (payload: T) => void,
): Promise<() => void> {
  return importListen()
    .then((listen) =>
      listen<T>(eventName, (event) => onPayload(event.payload)),
    )
    .catch((err) => {
      console.error(`[mcp-notifications] ${eventName} failed:`, err);
      return () => {};
    });
}

/**
 * Subscribe to the `ui_state/drag_started` plane (a cross-window drag began).
 *
 * The drag state machine lives in the `ui_state` service; a plugin subscribes
 * with `this.ui_state.on("drag_started", …)`. The webview consumes it as a pure
 * MCP client — the host re-broadcasts the bridge notification as the Tauri event
 * named by its method.
 *
 * @param onStarted - Receives the started session payload.
 * @returns A promise resolving to an unsubscribe function.
 */
export function subscribeDragStarted(
  onStarted: (session: DragStarted) => void,
): Promise<() => void> {
  return subscribeBridgeEvent(DRAG_STARTED_EVENT, onStarted);
}

/**
 * Subscribe to the `ui_state/drag_cancelled` plane (the drag session cancelled).
 *
 * @param onCancelled - Receives the cancelled session id payload.
 * @returns A promise resolving to an unsubscribe function.
 */
export function subscribeDragCancelled(
  onCancelled: (payload: DragCancelled) => void,
): Promise<() => void> {
  return subscribeBridgeEvent(DRAG_CANCELLED_EVENT, onCancelled);
}

/**
 * Subscribe to the `ui_state/drag_completed` plane (the drag session dropped).
 *
 * @param onCompleted - Receives the completed session id + success payload.
 * @returns A promise resolving to an unsubscribe function.
 */
export function subscribeDragCompleted(
  onCompleted: (payload: DragCompleted) => void,
): Promise<() => void> {
  return subscribeBridgeEvent(DRAG_COMPLETED_EVENT, onCompleted);
}
