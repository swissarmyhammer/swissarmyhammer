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

/** One `notifications/ui_state/changed` notification's params. */
export interface UiStateChanged {
  /** The window the change is scoped to; absent for global state. */
  window?: string;
  /** The UI surface that changed (e.g. "palette_open", "keymap_mode"). */
  key: string;
  /** The surface's new value. */
  value: unknown;
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
