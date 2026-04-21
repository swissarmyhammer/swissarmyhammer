import { forwardRef, useCallback } from "react";
import { icons, LayoutGrid } from "lucide-react";
import { useViews } from "@/lib/views-context";
import { useDispatchCommand } from "@/lib/command-scope";
import { useEntityFocus } from "@/lib/entity-focus-context";
import { FocusScope, useFocusScopeElementRef } from "@/components/focus-scope";
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
 * Props for the inner `<button>` element of a view switcher.
 *
 * Extends standard button attributes so `TooltipTrigger asChild` can
 * forward its own event/ref props through to the DOM node.
 */
interface ViewButtonElementProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  /** The view's moniker — used for `data-moniker` attribution. */
  viewMoniker: string;
  /** Whether this view is currently active (drives `data-active` + styling). */
  isActive: boolean;
  /** Icon content rendered inside the button. */
  children: React.ReactNode;
}

/**
 * The `<button>` element for a view switcher.
 *
 * Wired via `React.forwardRef` so Radix `TooltipTrigger asChild` can
 * forward its Slot ref, and internally composes that with the
 * enclosing `FocusScope`'s `elementRef` (read from
 * `FocusScopeElementRefContext`) so `ResizeObserver` can measure the
 * button's rect for spatial navigation.
 *
 * The button owns the `onClick` handler passed from the parent
 * (`ViewButton`) — that handler sets focus on the view moniker and
 * dispatches `view.switch:<id>`. Because `FocusScope` with
 * `renderContainer={false}` does not attach its own click handler,
 * setting focus explicitly is the consumer's responsibility.
 */
const ViewButtonElement = forwardRef<HTMLButtonElement, ViewButtonElementProps>(
  function ViewButtonElement(
    { viewMoniker, isActive, children, className, ...rest },
    forwardedRef,
  ) {
    const scopeElementRef = useFocusScopeElementRef();

    /**
     * Compose the forwarded ref (from `TooltipTrigger asChild`) with
     * the scope's `elementRef`. Both need to land on the same `<button>`
     * node — the forwarded ref lets Radix's positioning logic measure
     * the trigger, and the scope ref lets the spatial engine track the
     * same rect.
     */
    const refCallback = useCallback(
      (node: HTMLButtonElement | null) => {
        if (scopeElementRef) scopeElementRef.current = node;
        if (typeof forwardedRef === "function") forwardedRef(node);
        else if (forwardedRef) forwardedRef.current = node;
      },
      [scopeElementRef, forwardedRef],
    );

    return (
      <button
        ref={refCallback}
        data-moniker={viewMoniker}
        data-testid={`data-moniker:${viewMoniker}`}
        data-active={isActive ? "true" : "false"}
        className={cn(
          "flex items-center justify-center rounded-md p-1.5 transition-colors",
          isActive
            ? "bg-primary text-primary-foreground"
            : "text-muted-foreground hover:bg-accent hover:text-accent-foreground",
          className,
        )}
        {...rest}
      >
        {children}
      </button>
    );
  },
);

/**
 * One view-switcher button, wrapped in a `FocusScope` so the LeftNav
 * is reachable by spatial `h`/`l` navigation from any view body.
 *
 * The `FocusScope` uses `renderContainer={false}` because the button
 * is the DOM element that defines the scope's spatial footprint —
 * the scope attaches `elementRef` to the `<button>` itself via
 * `useFocusScopeElementRef()`, mirroring the pattern used for row
 * selectors and cells in `data-table.tsx`.
 *
 * `showFocusBar={false}` suppresses the scope's focus-bar highlight;
 * LeftNav already has its own `data-active` styling and an extra
 * focus bar would duplicate it. Clicks still set focus (through an
 * explicit `setFocus(mk)` call in the handler) and dispatch the
 * existing `view.switch:<id>` command verbatim.
 */
function ViewButton({ view }: { view: ViewDef }) {
  const { activeView } = useViews();
  const dispatch = useDispatchCommand();
  const { setFocus } = useEntityFocus();
  const isActive = activeView?.id === view.id;
  const mk = moniker("view", view.id);

  const handleClick = useCallback(() => {
    setFocus(mk);
    dispatch(`view.switch:${view.id}`).catch(console.error);
  }, [setFocus, mk, dispatch, view.id]);

  return (
    <FocusScope
      moniker={mk}
      commands={[]}
      renderContainer={false}
      showFocusBar={false}
    >
      <Tooltip>
        <TooltipTrigger asChild>
          <ViewButtonElement
            viewMoniker={mk}
            isActive={isActive}
            onClick={handleClick}
          >
            {viewIcon(view)}
          </ViewButtonElement>
        </TooltipTrigger>
        <TooltipContent side="right" sideOffset={8}>
          {view.name}
        </TooltipContent>
      </Tooltip>
    </FocusScope>
  );
}

/** Left-edge view switcher strip — one button per registered view. */
export function LeftNav() {
  const { views } = useViews();

  if (views.length === 0) return null;

  return (
    <nav className="flex flex-col items-center gap-1 py-2 px-1 border-r bg-muted/30 w-10 shrink-0">
      {views.map((view) => (
        <ViewButton key={view.id} view={view} />
      ))}
    </nav>
  );
}
