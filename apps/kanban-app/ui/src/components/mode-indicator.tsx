import { useSyncExternalStore, type ReactNode } from "react";
import { SparklesIcon, TriangleAlertIcon } from "lucide-react";
import { useAppMode, type AppMode } from "@/lib/app-mode-context";
import { useUIState } from "@/lib/ui-state-context";
import { aiStatus, subscribeAiStatus } from "@/ai/commands";
import type { ConversationStatus } from "@/ai/conversation";
import { Loader } from "@/components/ai-elements/loader";
import { cn } from "@/lib/utils";

/** Maps each mode to its vim-style display label. */
const MODE_LABELS: Record<AppMode, string> = {
  normal: "-- NORMAL --",
  command: "-- COMMAND --",
  search: "-- SEARCH --",
};

/**
 * Per-status presentation for the bottom-bar AI status indicator: a label,
 * an icon, and a tone class. The conversation hook reports one of these
 * three {@link ConversationStatus} values into the `ai/commands.ts` store.
 */
const AI_STATUS_PRESENTATION: Record<
  ConversationStatus,
  { label: string; icon: ReactNode; tone: string }
> = {
  idle: {
    label: "AI idle",
    icon: <SparklesIcon className="size-3" />,
    tone: "text-muted-foreground",
  },
  streaming: {
    label: "AI streaming",
    icon: <Loader size={12} />,
    tone: "text-foreground",
  },
  error: {
    label: "AI error",
    icon: <TriangleAlertIcon className="size-3" />,
    tone: "text-destructive",
  },
};

/**
 * The bottom bar's AI status indicator.
 *
 * Subscribes to the `ai/commands.ts` turn-status store — the single source
 * of truth `AiPanelConversation` writes the live ACP turn status into — and
 * renders `idle` / `streaming` / `error` so the user sees the agent's state
 * from anywhere in the app, not just inside the AI panel.
 */
function AiStatusIndicator(): ReactNode {
  // `useSyncExternalStore` keeps this in lockstep with the module-level
  // status store; the store notifies on every real status change.
  const status = useSyncExternalStore(subscribeAiStatus, aiStatus, aiStatus);
  const { label, icon, tone } = AI_STATUS_PRESENTATION[status];

  return (
    <span
      className={cn("flex items-center gap-1", tone)}
      data-testid="ai-status-indicator"
      data-ai-status={status}
    >
      {icon}
      <span>{label}</span>
    </span>
  );
}

/**
 * A vim-style mode indicator bar fixed at the bottom of the viewport.
 *
 * The bar always renders: its right slot carries the AI status indicator
 * (idle / streaming / error), which must be visible in every keymap mode.
 * The center vim-style mode label (`-- NORMAL --` etc.) is shown only when
 * the keymap is set to "vim"; the left slot is a placeholder (view name) for
 * future use.
 */
export function ModeIndicator() {
  const { mode } = useAppMode();
  const { keymap_mode } = useUIState();

  const showVimMode = keymap_mode === "vim";

  return (
    <div
      data-testid="mode-indicator"
      className="flex items-center justify-between px-3 py-0.5 font-mono text-xs
        bg-muted text-muted-foreground border-t border-border shrink-0"
    >
      {/* Left slot: view name (placeholder) */}
      <span data-testid="mode-indicator-left" className="min-w-0 truncate">
        &nbsp;
      </span>

      {/* Center: vim-style mode label — only meaningful in vim mode. */}
      {showVimMode ? (
        <span
          data-testid="mode-indicator-mode"
          className="font-bold tracking-wide"
        >
          {MODE_LABELS[mode]}
        </span>
      ) : (
        <span className="min-w-0 truncate">&nbsp;</span>
      )}

      {/* Right slot: the AI status indicator, shown in every keymap mode. */}
      <span
        data-testid="mode-indicator-right"
        className="flex min-w-0 items-center justify-end gap-1 truncate text-right"
      >
        <AiStatusIndicator />
      </span>
    </div>
  );
}
