/**
 * Subscribes to Tauri `entity-field-changed` events and keeps `BoardData`
 * in sync for structural entity types (board, column).
 *
 * Background: the entity store maintained by `RustEngineContainer` is
 * patched in place by its own listener, but `BoardData` lives one level
 * up in `WindowContainer` and was not being updated by any listener.
 * That meant column renames, column reorders, and board renames were
 * invisible to consumers reading `useBoardData()` until a full board
 * refresh fired — which never happened for plain field changes.
 *
 * The most visible symptom: after dragging a column then pressing undo,
 * the columns reverted on disk but the board view stayed in its post-drag
 * order, because `BoardDataContext` still held the pre-undo column array.
 *
 * The hook applies `patchBoardData` whenever a structural field change
 * arrives. Non-structural types (`task`, `tag`, etc.) short-circuit via
 * the `null` branch in `patchBoardData` so this never triggers a board
 * refetch on task-level mutations.
 */
import { useEffect } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { patchBoardData } from "./board-data-patch";
import type { BoardData, Entity } from "@/types/kanban";

interface EntityFieldChangedEvent {
  kind: "entity-field-changed";
  entity_type: string;
  id: string;
  changes: Array<{ field: string; value: unknown }>;
  board_path?: string;
}

type SetBoardFn = React.Dispatch<React.SetStateAction<BoardData | null>>;

/**
 * Subscribe `setBoard` to `entity-field-changed` events.
 *
 * When the event targets a structural entity (board or column), the hook
 * applies the field patch to the in-memory `BoardData` so anything reading
 * `useBoardData()` re-renders against the new values immediately. The
 * existing flat per-type entity store (managed by `RustEngineContainer`)
 * stays untouched — this hook layers on top of it, it does not replace it.
 *
 * `activeBoardPathRef` mirrors the filter used by
 * `handleEntityFieldChanged` in rust-engine-container: events tagged with
 * a `board_path` that doesn't match this window's active board are
 * dropped, so secondary windows showing other boards don't get
 * cross-patched. When either side is undefined the event passes through.
 */
export function useBoardDataSync(
  setBoard: SetBoardFn,
  activeBoardPathRef?: React.RefObject<string | undefined>,
): void {
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    let cancelled = false;

    listen<EntityFieldChangedEvent>("entity-field-changed", (event) => {
      const payload = event.payload;
      if (!payload || !payload.changes || payload.changes.length === 0) return;
      const { entity_type, id, changes, board_path } = payload;
      if (entity_type !== "board" && entity_type !== "column") return;
      // Drop events from boards other than this window's active board.
      // Mirrors `isBoardMismatch` in rust-engine-container.
      const active = activeBoardPathRef?.current;
      if (board_path && active && board_path !== active) return;

      setBoard((prev) => {
        if (!prev) return prev;
        const existing: Entity | undefined =
          entity_type === "board"
            ? prev.board
            : prev.columns.find((c) => c.id === id);
        if (!existing) return prev;

        const patchedFields = { ...existing.fields };
        for (const { field, value } of changes) patchedFields[field] = value;
        const patchedEntity: Entity = { ...existing, fields: patchedFields };

        return patchBoardData(prev, entity_type, id, patchedEntity) ?? prev;
      });
    }).then((u) => {
      if (cancelled) {
        u();
      } else {
        unlisten = u;
      }
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [setBoard, activeBoardPathRef]);
}
