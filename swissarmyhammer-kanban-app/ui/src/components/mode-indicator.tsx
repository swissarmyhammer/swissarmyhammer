import { useAppMode, type AppMode } from "@/lib/app-mode-context";
import { useKeymap } from "@/lib/keymap-context";

/** Maps each mode to its vim-style display label. */
const MODE_LABELS: Record<AppMode, string> = {
  normal: "-- NORMAL --",
  command: "-- COMMAND --",
  search: "-- SEARCH --",
};

/**
 * A vim-style mode indicator bar fixed at the bottom of the viewport.
 *
 * Only visible when the keymap is set to "vim". Displays the current app
 * interaction mode (normal, command, or search) in the center, with
 * placeholder slots on the left (view name) and right (sort/filter info)
 * for future use.
 */
export function ModeIndicator() {
  const { mode } = useAppMode();
  const { mode: keymapMode } = useKeymap();

  // Only show the vim-style mode indicator in vim mode
  if (keymapMode !== "vim") return null;

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
