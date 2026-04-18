import { useRef, type ReactNode } from "react";
import { X } from "lucide-react";
import { useDispatchCommand } from "@/lib/command-scope";

interface SlidePanelProps {
  open: boolean;
  onClose: () => void;
  style?: React.CSSProperties;
  children: ReactNode;
}

/**
 * Generic slide-out panel shell — fixed to the right edge.
 *
 * Renders children inside a 420px panel with a close button.
 * Knows nothing about entities, fields, or tasks.
 */
export function SlidePanel({
  open,
  onClose: _onClose,
  style,
  children,
}: SlidePanelProps) {
  const panelRef = useRef<HTMLDivElement>(null);
  const dispatchClose = useDispatchCommand("ui.inspector.close");

  return (
    // `role="dialog"` + `aria-modal="true"` is both an a11y improvement
    // (assistive tech announces the panel as a modal dialog) and the stable
    // selector the E2E harness uses to wait for inspector mount. See
    // `kanban-app/e2e/spatial-nav.e2e.ts`.
    <div
      ref={panelRef}
      role="dialog"
      aria-modal="true"
      className={`fixed top-0 z-30 h-full w-[420px] max-w-[85vw] bg-background border-l border-border shadow-xl flex flex-col transition-transform duration-200 ease-out ${
        open ? "translate-x-0" : "translate-x-full"
      }`}
      style={style}
    >
      <div className="flex items-center justify-end px-3 pt-3">
        <button
          onClick={() => {
            dispatchClose().catch(console.error);
          }}
          className="shrink-0 p-1 rounded-md text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
        >
          <X className="h-4 w-4" />
        </button>
      </div>
      <div className="flex-1 min-h-0 overflow-y-auto px-5 pb-5">{children}</div>
    </div>
  );
}
