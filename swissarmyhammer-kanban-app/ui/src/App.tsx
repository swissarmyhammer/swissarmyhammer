import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { NavBar } from "@/components/nav-bar";
import type { Board, OpenBoard } from "@/types/kanban";

function App() {
  const [board, setBoard] = useState<Board | null>(null);
  const [openBoards, setOpenBoards] = useState<OpenBoard[]>([]);

  const refresh = useCallback(async () => {
    try {
      const [boardData, openData] = await Promise.all([
        invoke<Board>("get_board", { path: null }),
        invoke<OpenBoard[]>("list_open_boards"),
      ]);
      setBoard(boardData);
      setOpenBoards(openData);
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
    <div className="min-h-screen bg-background text-foreground flex flex-col">
      <NavBar
        board={board}
        openBoards={openBoards}
        onBoardChanged={refresh}
      />
      <main className="flex-1 p-4">
        {board ? (
          <p className="text-muted-foreground">
            Board loaded: {board.name}
          </p>
        ) : (
          <p className="text-muted-foreground">
            No board loaded. Open a board to get started.
          </p>
        )}
      </main>
    </div>
  );
}

export default App;
