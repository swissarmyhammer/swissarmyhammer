import { useAppMode, type AppMode } from "@/lib/app-mode-context";
import { useUIState } from "@/lib/ui-state-context";

/** Maps each mode to its vim-style display label. */
const MODE_LABELS: Record<AppMode, string> = {
  normal: "-- NORMAL --",
  command: "-- COMMAND --",
  search: "-- SEARCH --",
};

/**
 * A vim-style mode indicator bar fixed at the bottom of the viewport.
 *
 * The bar always renders. The center vim-style mode label (`-- NORMAL --`
 * etc.) is shown only when the keymap is set to "vim"; the left and right
 * slots are placeholders (view name, status) for future use.
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

      {/* Right slot: status (placeholder) */}
      <span
        data-testid="mode-indicator-right"
        className="flex min-w-0 items-center justify-end gap-1 truncate text-right"
      >
        &nbsp;
      </span>
    </div>
  );
}
