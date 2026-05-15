import { icons, LayoutGrid } from "lucide-react";
import { useViews } from "@/lib/views-context";
import { CommandScopeProvider, useDispatchCommand } from "@/lib/command-scope";
import { useContextMenu } from "@/lib/context-menu";
import { moniker } from "@/lib/moniker";
import { cn } from "@/lib/utils";
import { FocusScope } from "@/components/focus-scope";
import { Pressable } from "@/components/pressable";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { asSegment } from "@/types/spatial";
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
    <FocusScope
      moniker={asSegment("ui:left-nav")}
      // showFocus=false: viewport-spanning sidebar chrome; the inner view buttons own the visible focus signal.
      showFocus={false}
      role="navigation"
      className="flex flex-col items-center gap-1 py-2 pl-3 pr-1 border-r bg-muted/30 w-10 shrink-0"
    >
      {views.map((view) => (
        <ScopedViewButton
          key={view.id}
          view={view}
          isActive={activeView?.id === view.id}
        />
      ))}
    </FocusScope>
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
 * Migrates to `<Pressable asChild>` so the chrome leaf gains both
 * keyboard reachability (the inner `<FocusScope>` provided by Pressable)
 * AND scope-level CommandDefs that bind Enter (vim/cua) and Space (cua)
 * to the same `view.set` dispatch as a pointer click. Pre-migration the
 * site was a hand-rolled `<FocusScope view:{id}>` plus a manually-built
 * `view.activate` CommandDef; Pressable's `pressable.activate` Enter
 * binding subsumes the manual CommandDef so the dead weight is gone.
 *
 * The leaf moniker `ui:leftnav.view:{id}` follows the chrome-namespace
 * pattern (`ui:navbar.*` / `ui:perspective-bar.*`) — UI chrome is not
 * inspectable. The outer `CommandScopeProvider` keeps `view:{id}` in
 * the right-click scope chain so `entity.add:{type}` dynamics still
 * resolve for views that declare an `entity_type`.
 *
 * Left-click dispatches the canonical `view.set` command with the view id
 * in `args` (the palette fan-out that used to emit `view.switch:{id}` was
 * retired in 01KPZMXXEXKVE3RNPA4XJP0105). Right-click raises the native
 * context menu via `useContextMenu`. The menu never contains a
 * `Switch to <ViewName>` entry — view switching is palette-only — but
 * scope-dependent dynamics (e.g. `entity.add:{type}`) still surface for
 * views that declare an `entity_type`.
 */
function ViewButton({ view, isActive }: ScopedViewButtonProps) {
  const dispatch = useDispatchCommand("view.set");
  const handleContextMenu = useContextMenu();
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Pressable
          asChild
          moniker={asSegment(`ui:leftnav.view:${view.id}`)}
          ariaLabel={view.name}
          onPress={() => {
            dispatch({ args: { view_id: view.id } }).catch(console.error);
          }}
        >
          <button
            type="button"
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
        </Pressable>
      </TooltipTrigger>
      <TooltipContent side="right" sideOffset={8}>
        {view.name}
      </TooltipContent>
    </Tooltip>
  );
}
