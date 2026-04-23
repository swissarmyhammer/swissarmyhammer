import { icons, LayoutGrid } from "lucide-react";
import { useViews } from "@/lib/views-context";
import { CommandScopeProvider, useDispatchCommand } from "@/lib/command-scope";
import { useContextMenu } from "@/lib/context-menu";
import { moniker } from "@/lib/moniker";
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

/**
 * Left-nav sidebar listing every known view as an icon button.
 *
 * Each button is wrapped in its own {@link CommandScopeProvider} with a
 * `view:{id}` moniker so right-click on that specific button resolves a
 * scope chain the backend recognises. View switching is palette-only, so
 * the context menu never shows a "Switch to <ViewName>" entry; the
 * `view:{id}` moniker is still needed for other dynamics (e.g.
 * `entity.add:{type}` when the view declares an `entity_type`).
 */
export function LeftNav() {
  const { views, activeView } = useViews();

  if (views.length === 0) return null;

  return (
    <nav className="flex flex-col items-center gap-1 py-2 px-1 border-r bg-muted/30 w-10 shrink-0">
      {views.map((view) => (
        <ScopedViewButton
          key={view.id}
          view={view}
          isActive={activeView?.id === view.id}
        />
      ))}
    </nav>
  );
}

/** Props for a single view button rendered inside its own command scope. */
interface ScopedViewButtonProps {
  view: ViewDef;
  isActive: boolean;
}

/**
 * Wraps a single view button in a {@link CommandScopeProvider} with a
 * `view:{id}` moniker.
 *
 * Mirrors `ScopedPerspectiveTab` in `perspective-tab-bar.tsx`: the moniker
 * placed in the scope chain is what `useContextMenu` reads via
 * `CommandScopeContext`. The backend does not emit `view.switch:*` as a
 * context-menu entry, but other dynamic commands (notably
 * `entity.add:{type}` for views with an `entity_type`) still require the
 * `view:{id}` moniker to resolve their scope.
 */
function ScopedViewButton({ view, isActive }: ScopedViewButtonProps) {
  return (
    <CommandScopeProvider moniker={moniker("view", view.id)}>
      <ViewButton view={view} isActive={isActive} />
    </CommandScopeProvider>
  );
}

/**
 * The actual icon button for a single view.
 *
 * Must be rendered inside a {@link CommandScopeProvider} that supplies the
 * `view:{id}` moniker — {@link useContextMenu} reads that scope chain when
 * building the context-menu request to the backend.
 *
 * Left-click dispatches `view.switch:{id}` through the command pipeline;
 * right-click raises the native context menu via `useContextMenu`. The
 * menu never contains a `Switch to <ViewName>` entry — view switching is
 * palette-only — but scope-dependent dynamics (e.g. `entity.add:{type}`)
 * still surface for views that declare an `entity_type`.
 */
function ViewButton({ view, isActive }: ScopedViewButtonProps) {
  const dispatch = useDispatchCommand();
  const handleContextMenu = useContextMenu();
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          onClick={() => dispatch(`view.switch:${view.id}`)}
          onContextMenu={handleContextMenu}
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
}
