import { Kanban, List, Calendar, Clock, LayoutGrid, Table2, Tag } from "lucide-react";
import { useViews } from "@/lib/views-context";
import { useExecuteCommand } from "@/lib/command-scope";
import { cn } from "@/lib/utils";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import type { ViewDef } from "@/types/kanban";

/** Map a view icon name (from YAML) to a Lucide icon component. */
function viewIcon(view: ViewDef) {
  switch (view.icon ?? view.kind) {
    case "kanban":
    case "board":
      return <Kanban className="h-4 w-4" />;
    case "list":
      return <List className="h-4 w-4" />;
    case "calendar":
      return <Calendar className="h-4 w-4" />;
    case "timeline":
      return <Clock className="h-4 w-4" />;
    case "table":
    case "grid":
      return <Table2 className="h-4 w-4" />;
    case "tag":
      return <Tag className="h-4 w-4" />;
    default:
      return <LayoutGrid className="h-4 w-4" />;
  }
}

export function LeftNav() {
  const { views, activeView } = useViews();
  const executeCommand = useExecuteCommand();

  if (views.length === 0) return null;

  return (
    <nav className="flex flex-col items-center gap-1 py-2 px-1 border-r bg-muted/30 w-10 shrink-0">
      {views.map((view) => {
        const isActive = activeView?.id === view.id;
        return (
          <Tooltip key={view.id}>
            <TooltipTrigger asChild>
              <button
                onClick={() => executeCommand(`nav.view.${view.id}`)}
                className={cn(
                  "flex items-center justify-center rounded-md p-1.5 transition-colors",
                  isActive
                    ? "bg-primary text-primary-foreground"
                    : "text-muted-foreground hover:bg-accent hover:text-accent-foreground",
                )}
              >
                {viewIcon(view)}
              </button>
            </TooltipTrigger>
            <TooltipContent side="right" sideOffset={8}>
              {view.name}
            </TooltipContent>
          </Tooltip>
        );
      })}
    </nav>
  );
}
