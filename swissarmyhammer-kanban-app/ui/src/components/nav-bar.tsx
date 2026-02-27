import { invoke } from "@tauri-apps/api/core";
import { Check, ChevronDown, FolderOpen } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import type { Board, OpenBoard, RecentBoard } from "@/types/kanban";

interface NavBarProps {
  board: Board | null;
  openBoards: OpenBoard[];
  recentBoards: RecentBoard[];
  onBoardChanged: () => void;
}

export function NavBar({
  board,
  openBoards,
  recentBoards,
  onBoardChanged,
}: NavBarProps) {
  const handleSwitchBoard = async (path: string) => {
    await invoke("set_active_board", { path });
    onBoardChanged();
  };

  const handleOpenRecent = async (path: string) => {
    await invoke("open_board", { path });
    onBoardChanged();
  };

  const handleOpenFolder = async () => {
    // For now, prompt user — folder dialog requires tauri-plugin-dialog
    const path = window.prompt("Enter board path:");
    if (path) {
      await invoke("open_board", { path });
      onBoardChanged();
    }
  };

  const summary = board?.summary;

  return (
    <header className="flex h-12 items-center border-b px-4 gap-3">
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button variant="ghost" className="gap-1 font-semibold">
            {board?.name ?? "No Board"}
            <ChevronDown className="h-4 w-4 opacity-50" />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="start" className="w-64">
          {openBoards.length > 0 && (
            <>
              <DropdownMenuLabel>Open</DropdownMenuLabel>
              {openBoards.map((ob) => (
                <DropdownMenuItem
                  key={ob.path}
                  onClick={() => handleSwitchBoard(ob.path)}
                >
                  {ob.is_active && <Check className="h-4 w-4" />}
                  <span className={ob.is_active ? "font-medium" : ""}>
                    {ob.path.split("/").pop() || ob.path}
                  </span>
                </DropdownMenuItem>
              ))}
            </>
          )}
          {recentBoards.length > 0 && (
            <>
              <DropdownMenuSeparator />
              <DropdownMenuLabel>Recent</DropdownMenuLabel>
              {recentBoards.slice(0, 5).map((rb) => (
                <DropdownMenuItem
                  key={rb.path}
                  onClick={() => handleOpenRecent(rb.path)}
                >
                  {rb.name}
                </DropdownMenuItem>
              ))}
            </>
          )}
          <DropdownMenuSeparator />
          <DropdownMenuItem onClick={handleOpenFolder}>
            <FolderOpen className="h-4 w-4" />
            Open Board...
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>

      <div className="flex items-center gap-2 ml-auto">
        {summary && (
          <>
            <Badge variant="secondary">
              {summary.total_tasks} tasks
            </Badge>
            <Badge variant="default">
              {summary.ready_tasks} ready
            </Badge>
            {summary.blocked_tasks > 0 && (
              <Badge variant="outline">
                {summary.blocked_tasks} blocked
              </Badge>
            )}
          </>
        )}
      </div>
    </header>
  );
}
