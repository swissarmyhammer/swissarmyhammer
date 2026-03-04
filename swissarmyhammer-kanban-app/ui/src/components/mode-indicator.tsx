import { useAppMode, type AppMode } from "@/lib/app-mode-context";

/** Maps each mode to its vim-style display label. */
const MODE_LABELS: Record<AppMode, string> = {
  normal: "-- NORMAL --",
  command: "-- COMMAND --",
  search: "-- SEARCH --",
};

/**
 * A vim-style mode indicator bar fixed at the bottom of the viewport.
 *
 * Displays the current app mode (normal, command, or search) in the center,
 * with placeholder slots on the left (view name) and right (sort/filter info)
 * for future use.
 */
export function ModeIndicator() {
  const { mode } = useAppMode();

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

      {/* Center: mode label */}
      <span data-testid="mode-indicator-mode" className="font-bold tracking-wide">
        {MODE_LABELS[mode]}
      </span>

      {/* Right slot: sort/filter info (placeholder) */}
      <span data-testid="mode-indicator-right" className="min-w-0 truncate text-right">
        &nbsp;
      </span>
    </div>
  );
}
