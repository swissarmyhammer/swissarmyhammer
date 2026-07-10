/**
 * Subscribes to MCP `notifications/store/changed` and keeps `BoardData`
 * in sync for structural entity types (board, column).
 *
 * Background: the entity store maintained by `RustEngineContainer` is
 * patched in place by its own store-change reducer, but `BoardData` lives
 * one level up in `WindowContainer` and was not being updated by any
 * listener. That meant column renames, column reorders, and board renames
 * were invisible to consumers reading `useBoardData()` until a full board
 * refresh fired — which never happened for plain field changes.
 *
 * The most visible symptom: after dragging a column then pressing undo,
 * the columns reverted on disk but the board view stayed in its post-drag
 * order, because `BoardDataContext` still held the pre-undo column array.
 *
 * The hook applies `patchBoardData` whenever a structural field change
 * arrives in the MCP store-change stream. Non-structural stores (`task`,
 * `tag`, etc.) and reload-item stores (`view`, `perspective`) are ignored,
 * so this never patches `BoardData` on task-level mutations. A whole
 * transaction's changes are applied as one batch, matching the atomic
 * re-render contract.
 */
import { useEffect } from "react";
import {
  subscribeStoreChanged,
  type StoreChanged,
} from "./mcp-notifications";
import { patchBoardData } from "./board-data-patch";
import type { BoardData, Entity } from "@/types/kanban";

type SetBoardFn = React.Dispatch<React.SetStateAction<BoardData | null>>;

/** Apply one structural `store/changed` patch to in-memory `BoardData`. */
function patchStructural(
  prev: BoardData | null,
  note: StoreChanged,
): BoardData | null {
  if (!prev) return prev;
  const { store: entity_type, item: id, changes } = note;
  if (!changes || changes.length === 0) return prev;

  const existing: Entity | undefined =
    entity_type === "board"
      ? prev.board
      : prev.columns.find((c) => c.id === id);
  if (!existing) return prev;

  const patchedFields = { ...existing.fields };
  for (const { field, value } of changes) patchedFields[field] = value;
  const patchedEntity: Entity = { ...existing, fields: patchedFields };

  return patchBoardData(prev, entity_type, id, patchedEntity) ?? prev;
}

/**
 * Subscribe `setBoard` to the MCP `store/changed` plane.
 *
 * When a notification targets a structural store (board or column), the hook
 * applies the field patch to the in-memory `BoardData` so anything reading
 * `useBoardData()` re-renders against the new values immediately. The
 * existing flat per-type entity store (managed by `RustEngineContainer`)
 * stays untouched — this hook layers on top of it, it does not replace it.
 *
 * The whole transaction batch is folded in a single `setBoard` so a
 * multi-column move (or its undo) re-renders the board once, not per column.
 *
 * `activeBoardPathRef` is retained for API compatibility; cross-board
 * filtering now happens upstream (each window's host scopes its own
 * notification stream), so it is no longer read here.
 */
export function useBoardDataSync(
  setBoard: SetBoardFn,
  activeBoardPathRef?: React.RefObject<string | undefined>,
): void {
  void activeBoardPathRef;
  useEffect(() => {
    let disposed = false;

    const unsubPromise = subscribeStoreChanged((batch) => {
      const structural = batch.filter(
        (n) => n.store === "board" || n.store === "column",
      );
      if (structural.length === 0) return;

      setBoard((prev) => {
        let next = prev;
        for (const note of structural) next = patchStructural(next, note);
        return next;
      });
    });

    return () => {
      disposed = true;
      unsubPromise.then((unsub) => {
        if (disposed) unsub();
      });
    };
  }, [setBoard]);
}
