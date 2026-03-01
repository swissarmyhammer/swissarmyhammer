import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { KeymapProvider } from "@/lib/keymap-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import { NavBar } from "@/components/nav-bar";
import { BoardView } from "@/components/board-view";
import { TaskDetailPanel } from "@/components/task-detail-panel";
import { TagInspector } from "@/components/tag-inspector";
import type { Board, Tag, Task, OpenBoard } from "@/types/kanban";

const PANEL_WIDTH = 420;

interface TaskListResponse {
  tasks: Task[];
  count: number;
}

type PanelEntry =
  | { type: "task"; taskId: string }
  | { type: "tag"; tagId: string };

interface TagContextMenuPayload {
  action: string;
  tag_id: string;
  task_id: string | null;
}

function App() {
  const [board, setBoard] = useState<Board | null>(null);
  const [tasks, setTasks] = useState<Task[]>([]);
  const [openBoards, setOpenBoards] = useState<OpenBoard[]>([]);
  const [panelStack, setPanelStack] = useState<PanelEntry[]>([]);

  // Derive selected task from the stack
  const selectedTask = useMemo(() => {
    const taskEntry = panelStack.find((e): e is PanelEntry & { type: "task" } => e.type === "task");
    if (!taskEntry) return null;
    return tasks.find((t) => t.id === taskEntry.taskId) ?? null;
  }, [panelStack, tasks]);

  // Derive inspected tag from the stack
  const inspectedTag = useMemo((): Tag | null => {
    const tagEntry = panelStack.find((e): e is PanelEntry & { type: "tag" } => e.type === "tag");
    if (!tagEntry || !board) return null;
    return board.tags.find((t) => t.id === tagEntry.tagId) ?? null;
  }, [panelStack, board]);

  // Helper: open a task panel (replaces entire stack)
  const openTaskPanel = useCallback((taskId: string) => {
    setPanelStack([{ type: "task", taskId }]);
  }, []);

  // Helper: push/replace a tag panel onto the stack
  const openTagPanel = useCallback((tagId: string) => {
    setPanelStack((prev) => {
      // Remove any existing tag entry, then push new one
      const filtered = prev.filter((e) => e.type !== "tag");
      return [...filtered, { type: "tag", tagId }];
    });
  }, []);

  // Helper: pop the topmost panel
  const closeTopPanel = useCallback(() => {
    setPanelStack((prev) => prev.slice(0, -1));
  }, []);

  // No renameTagInStack needed — tag IDs are stable ULIDs

  // Helper: close all panels
  const closeAll = useCallback(() => {
    setPanelStack([]);
  }, []);

  const refresh = useCallback(async () => {
    try {
      const [boardData, openData, taskData] = await Promise.all([
        invoke<Board>("get_board", { path: null }),
        invoke<OpenBoard[]>("list_open_boards"),
        invoke<TaskListResponse>("list_tasks", { path: null }),
      ]);
      setBoard(boardData);
      setOpenBoards(openData);
      setTasks(taskData.tasks);
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

  // Global Escape handler — pops the panel stack
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key !== "Escape" || panelStack.length === 0) return;
      // Don't close if an editable field is focused
      const tag = (e.target as HTMLElement)?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA") return;
      // Don't close if a CodeMirror editor is focused
      if ((e.target as HTMLElement)?.closest?.(".cm-editor")) return;
      closeTopPanel();
    }
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [panelStack.length, closeTopPanel]);

  // Tag context menu listener
  useEffect(() => {
    const unlisten = listen<TagContextMenuPayload>("tag-context-menu", async (event) => {
      const { action, tag_id, task_id } = event.payload;
      if (action === "tag_edit") {
        // tag_id from context menu is the slug — resolve to ULID
        const tag = board?.tags.find((t) => t.name === tag_id);
        if (tag) openTagPanel(tag.id);
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
  }, [board, openTagPanel, refresh]);

  const handleUpdateTitle = useCallback(async (taskId: string, title: string) => {
    try {
      await invoke("update_task_title", { id: taskId, title });
      refresh();
    } catch (e) {
      console.error("Failed to update task title:", e);
    }
  }, [refresh]);

  const handleUpdateDescription = useCallback(async (taskId: string, description: string) => {
    try {
      await invoke("update_task_description", { id: taskId, description });
      refresh();
    } catch (e) {
      console.error("Failed to update task description:", e);
    }
  }, [refresh]);

  return (
    <TooltipProvider delayDuration={400}>
    <KeymapProvider>
    <div className="h-screen bg-background text-foreground flex flex-col">
      <NavBar
        board={board}
        openBoards={openBoards}
        onBoardChanged={refresh}
      />
      {board ? (
        <>
          <BoardView
            board={board}
            tasks={tasks}
            onTaskClick={(t) => openTaskPanel(t.id)}
            onUpdateTitle={handleUpdateTitle}
            onTaskMoved={refresh}
          />

          {/* Backdrop — visible when any panel is open */}
          <div
            className={`fixed inset-0 bg-black/20 transition-opacity duration-200 ${
              panelStack.length > 0 ? "opacity-100" : "opacity-0 pointer-events-none"
            }`}
            onClick={closeAll}
          />

          {/* Render panels from the stack */}
          {panelStack.map((entry, index) => {
            const rightOffset = (panelStack.length - 1 - index) * PANEL_WIDTH;
            if (entry.type === "task") {
              return (
                <TaskDetailPanel
                  key={`task-${entry.taskId}`}
                  task={selectedTask}
                  tags={board.tags}
                  onClose={closeTopPanel}
                  onUpdateTitle={handleUpdateTitle}
                  onUpdateDescription={handleUpdateDescription}
                  style={{ right: rightOffset }}
                />
              );
            }
            if (entry.type === "tag" && inspectedTag) {
              return (
                <TagInspector
                  key={`tag-${entry.tagId}`}
                  tag={inspectedTag}
                  onClose={closeTopPanel}
                  onRefresh={refresh}
                  style={{ right: rightOffset }}
                />
              );
            }
            return null;
          })}
        </>
      ) : (
        <main className="flex-1 flex items-center justify-center">
          <p className="text-muted-foreground">
            No board loaded. Open a board to get started.
          </p>
        </main>
      )}
    </div>
    </KeymapProvider>
    </TooltipProvider>
  );
}

export default App;
