import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { NavBar } from "@/components/nav-bar";
import { BoardView } from "@/components/board-view";
import { TaskDetailPanel } from "@/components/task-detail-panel";
import type { Board, Task, OpenBoard } from "@/types/kanban";

interface TaskListResponse {
  tasks: Task[];
  count: number;
}

function App() {
  const [board, setBoard] = useState<Board | null>(null);
  const [tasks, setTasks] = useState<Task[]>([]);
  const [openBoards, setOpenBoards] = useState<OpenBoard[]>([]);
  const [selectedTask, setSelectedTask] = useState<Task | null>(null);

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

  return (
    <div className="h-screen bg-background text-foreground flex flex-col">
      <NavBar
        board={board}
        openBoards={openBoards}
        onBoardChanged={refresh}
      />
      {board ? (
        <>
          <BoardView board={board} tasks={tasks} onTaskClick={setSelectedTask} onTaskMoved={refresh} />
          <TaskDetailPanel
            task={selectedTask}
            onClose={() => setSelectedTask(null)}
            onUpdateTitle={async (taskId, title) => {
              try {
                await invoke("update_task_title", { id: taskId, title });
                refresh();
              } catch (e) {
                console.error("Failed to update task title:", e);
              }
            }}
          />
        </>
      ) : (
        <main className="flex-1 flex items-center justify-center">
          <p className="text-muted-foreground">
            No board loaded. Open a board to get started.
          </p>
        </main>
      )}
    </div>
  );
}

export default App;
