import { icons, LayoutGrid } from "lucide-react";
import { useViews } from "@/lib/views-context";
import { useExecuteCommand } from "@/lib/command-scope";
import { cn } from "@/lib/utils";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import type { ViewDef } from "@/types/kanban";

/** Convert kebab-case icon name to PascalCase key for lucide-react lookup. */
function kebabToPascal(s: string): string {
  return s.replace(/(^|-)([a-z])/g, (_, _dash, c: string) => c.toUpperCase());
}

/** Resolve a view's icon from its YAML `icon` property via dynamic lucide lookup. */
function viewIcon(view: ViewDef) {
  const name = view.icon ?? view.kind;
  if (name) {
    const key = kebabToPascal(name);
    const Icon = icons[key as keyof typeof icons];
    if (Icon) return <Icon className="h-4 w-4" />;
  }
  return <LayoutGrid className="h-4 w-4" />;
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
