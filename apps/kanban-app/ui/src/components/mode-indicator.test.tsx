/**
 * Component tests for {@link ModeIndicator} — the app's bottom bar.
 *
 * One contract is pinned here: the bar itself always renders, and the
 * vim-style mode label inside it renders only in vim keymap mode.
 */
import { describe, it, expect } from "vitest";
import { screen } from "@testing-library/react";
import { renderInAct } from "@/test/act-render";
import { ModeIndicator } from "./mode-indicator";

/**
 * Render a bare {@link ModeIndicator}.
 *
 * `useAppMode` and `useUIState` both expose default context values
 * (`mode: "normal"`, `keymap_mode: "cua"`), so the bar renders standalone
 * with no provider stack — and `ModeIndicator` itself touches no Tauri API,
 * so no `invoke` mock is needed. These tests therefore run in the default
 * CUA keymap.
 */
async function renderModeIndicator() {
  return renderInAct(<ModeIndicator />);
}

describe("ModeIndicator — bottom bar", () => {
  it("renders the bottom bar but hides the vim mode label outside vim mode", async () => {
    await renderModeIndicator();

    // The bar host is present.
    expect(screen.getByTestId("mode-indicator")).not.toBeNull();
    // …but the vim-only mode label is absent in CUA mode.
    expect(screen.queryByTestId("mode-indicator-mode")).toBeNull();
  });
});
