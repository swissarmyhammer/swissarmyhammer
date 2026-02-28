import { invoke } from "@tauri-apps/api/core";
import { Check, ChevronDown } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import type { Board, OpenBoard } from "@/types/kanban";

interface NavBarProps {
  board: Board | null;
  openBoards: OpenBoard[];
  onBoardChanged: () => void;
}

export function NavBar({
  board,
  openBoards,
  onBoardChanged,
}: NavBarProps) {
  const handleSwitchBoard = async (path: string) => {
    await invoke("set_active_board", { path });
    onBoardChanged();
  };

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
                    {(() => {
                      const parts = ob.path.split("/").filter(Boolean);
                      const last = parts[parts.length - 1];
                      return last === ".kanban" && parts.length > 1
                        ? parts[parts.length - 2]
                        : last || ob.path;
                    })()}
                  </span>
                </DropdownMenuItem>
              ))}
            </>
          )}
        </DropdownMenuContent>
      </DropdownMenu>

    </header>
  );
}
