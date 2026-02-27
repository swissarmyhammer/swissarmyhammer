import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { NavBar } from "@/components/nav-bar";
import type { Board, OpenBoard, RecentBoard } from "@/types/kanban";

function App() {
  const [board, setBoard] = useState<Board | null>(null);
  const [openBoards, setOpenBoards] = useState<OpenBoard[]>([]);
  const [recentBoards, setRecentBoards] = useState<RecentBoard[]>([]);

  const refresh = useCallback(async () => {
    try {
      const [boardData, openData, recentData] = await Promise.all([
        invoke<Board>("get_board", { path: null }),
        invoke<OpenBoard[]>("list_open_boards"),
        invoke<RecentBoard[]>("get_recent_boards"),
      ]);
      setBoard(boardData);
      setOpenBoards(openData);
      setRecentBoards(recentData);
    } catch (e) {
      console.error("Failed to load board data:", e);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  return (
    <div className="min-h-screen bg-background text-foreground flex flex-col">
      <NavBar
        board={board}
        openBoards={openBoards}
        recentBoards={recentBoards}
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
