import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { KeymapProvider } from "@/lib/keymap-context";
import { AppModeProvider } from "@/lib/app-mode-context";
import { UndoStackProvider } from "@/lib/undo-context";
import { SchemaProvider } from "@/lib/schema-context";
import { FieldUpdateProvider } from "@/lib/field-update-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import { AppShell } from "@/components/app-shell";
import { NavBar } from "@/components/nav-bar";
import { ModeIndicator } from "@/components/mode-indicator";
import { BoardView } from "@/components/board-view";
import { EntityInspector } from "@/components/entity-inspector";
import { SlidePanel } from "@/components/slide-panel";
import type {
  BoardData, OpenBoard, Entity,
  BoardDataResponse, EntityListResponse,
} from "@/types/kanban";
import { entityFromBag, boardDataToBoardData } from "@/types/kanban";

const PANEL_WIDTH = 420;

/** A panel entry is just an entity reference — entity type + id. */
interface PanelEntry {
  entityType: string;
  entityId: string;
}

interface TagContextMenuPayload {
  action: string;
  tag_id: string;
  task_id: string | null;
}

function App() {
  const [board, setBoard] = useState<BoardData | null>(null);
  const [taskEntities, setTaskEntities] = useState<Entity[]>([]);
  const [tagEntities, setTagEntities] = useState<Entity[]>([]);
  const [openBoards, setOpenBoards] = useState<OpenBoard[]>([]);
  const [panelStack, setPanelStack] = useState<PanelEntry[]>([]);

  /** Open an inspector for any entity. Replaces the stack for primary entities, pushes for secondary. */
  const inspectEntity = useCallback((entityType: string, entityId: string) => {
    setPanelStack((prev) => {
      // Primary entity (task, column, board) replaces the stack
      if (entityType === "task" || entityType === "column" || entityType === "board") {
        return [{ entityType, entityId }];
      }
      // Secondary entities (tag) push onto the stack, replacing any existing entry of same type
      const filtered = prev.filter((e) => e.entityType !== entityType);
      return [...filtered, { entityType, entityId }];
    });
  }, []);

  const closeTopPanel = useCallback(() => {
    setPanelStack((prev) => prev.slice(0, -1));
  }, []);

  const closeAll = useCallback(() => {
    setPanelStack([]);
  }, []);

  const refresh = useCallback(async () => {
    try {
      const [boardData, openData, taskData] = await Promise.all([
        invoke<BoardDataResponse>("get_board_data"),
        invoke<OpenBoard[]>("list_open_boards"),
        invoke<EntityListResponse>("list_entities", { entityType: "task" }),
      ]);
      setBoard(boardDataToBoardData(boardData));
      setOpenBoards(openData);
      setTaskEntities(taskData.entities.map(entityFromBag));
      setTagEntities(boardData.tags.map(entityFromBag));
    } catch (e) {
      console.error("Failed to load board data:", e);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  useEffect(() => {
    const unlisten = listen("board-changed", () => {
      refresh();
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [refresh]);

  // Tag context menu listener
  useEffect(() => {
    const unlisten = listen<TagContextMenuPayload>("tag-context-menu", async (event) => {
      const { action, tag_id, task_id } = event.payload;
      if (action === "tag_edit") {
        // tag_id from context menu is the slug — resolve to ULID via tagEntities
        const tag = tagEntities.find((t) => (t.fields.tag_name as string) === tag_id);
        if (tag) inspectEntity("tag", tag.id);
      } else if (action === "tag_delete" && task_id) {
        try {
          await invoke("untag_task", { id: task_id, tag: tag_id });
          refresh();
        } catch (e) {
          console.error("Failed to remove tag:", e);
        }
      }
    });
    return () => { unlisten.then((fn) => fn()); };
  }, [tagEntities, inspectEntity, refresh]);

  const entityStore = useMemo(() => ({
    task: taskEntities,
    tag: tagEntities,
    column: board?.columns ?? [],
    swimlane: board?.swimlanes ?? [],
  }), [taskEntities, tagEntities, board]);

  return (
    <TooltipProvider delayDuration={400}>
    <SchemaProvider>
    <EntityStoreProvider entities={entityStore}>
    <FieldUpdateProvider onRefresh={refresh}>
    <KeymapProvider>
    <AppModeProvider>
    <UndoStackProvider>
    <AppShell>
    <div className="h-screen bg-background text-foreground flex flex-col">
      <NavBar
        board={board}
        openBoards={openBoards}
        onBoardChanged={refresh}
        onBoardInspect={() => inspectEntity("board", "board")}
      />
      {board ? (
        <>
          <BoardView
            board={board}
            tasks={taskEntities}
            onTaskClick={(taskId) => inspectEntity("task", taskId)}
            onColumnInspect={(colId) => inspectEntity("column", colId)}
            onTaskMoved={refresh}
          />

          {/* Backdrop — visible when any panel is open */}
          <div
            className={`fixed inset-0 bg-black/20 transition-opacity duration-200 ${
              panelStack.length > 0 ? "opacity-100" : "opacity-0 pointer-events-none"
            }`}
            onClick={closeAll}
          />

          {/* Render inspector panels from the stack */}
          {panelStack.map((entry, index) => {
            const rightOffset = (panelStack.length - 1 - index) * PANEL_WIDTH;
            const entities = (entityStore as Record<string, Entity[]>)[entry.entityType];
            const entity = entities?.find((e) => e.id === entry.entityId);
            // Board entity is special — it's in board.board, not a list
            const resolved = entity ?? (
              entry.entityType === "board" ? board?.board : undefined
            );
            if (!resolved) return null;
            return (
              <SlidePanel
                key={`${entry.entityType}-${entry.entityId}`}
                open={true}
                onClose={closeTopPanel}
                style={{ right: rightOffset }}
              >
                <EntityInspector entity={resolved} />
              </SlidePanel>
            );
          })}
        </>
      ) : (
        <main className="flex-1 flex items-center justify-center">
          <p className="text-muted-foreground">
            No board loaded. Open a board to get started.
          </p>
        </main>
      )}
      <ModeIndicator />
    </div>
    </AppShell>
    </UndoStackProvider>
    </AppModeProvider>
    </KeymapProvider>
    </FieldUpdateProvider>
    </EntityStoreProvider>
    </SchemaProvider>
    </TooltipProvider>
  );
}

export default App;
