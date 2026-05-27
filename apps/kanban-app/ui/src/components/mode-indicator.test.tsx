/**
 * Component tests for {@link ModeIndicator} — the app's bottom bar.
 *
 * Two contracts are pinned here:
 *
 *   - the vim-style mode label still renders only in vim keymap mode;
 *   - the AI status indicator (idle / streaming / error) renders in the bar,
 *     sourced from the `ai/commands.ts` turn-status store, regardless of the
 *     active keymap — `AiPanelConversation` reports the ACP turn status into
 *     that store and the bottom bar reflects it.
 *
 * Source of truth for kanban task `01KRRQ3SPXBY1ZNRJHFGB09R3Z` — "AI panel
 * CM6 composer and bottom-bar AI status".
 */
import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { act, screen } from "@testing-library/react";
import { renderInAct } from "@/test/act-render";
import { resetAiCommandsForTest, setAiStatus } from "@/ai/commands";
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

describe("ModeIndicator — bottom-bar AI status", () => {
  beforeEach(() => {
    resetAiCommandsForTest();
  });

  afterEach(() => {
    resetAiCommandsForTest();
  });

  it("shows the AI status while a prompt is streaming, in the CUA keymap", async () => {
    await renderModeIndicator();

    // Idle before any turn — the indicator reports idle.
    const idle = await screen.findByTestId("ai-status-indicator");
    expect(idle.textContent?.toLowerCase()).toContain("idle");

    // A prompt turn starts — the conversation reports `streaming` into the
    // `ai/commands.ts` store; the bottom bar repaints.
    await act(async () => {
      setAiStatus("streaming");
    });
    expect(
      screen.getByTestId("ai-status-indicator").textContent?.toLowerCase(),
    ).toContain("streaming");
  });

  it("returns to idle after the turn ends", async () => {
    await renderModeIndicator();

    await act(async () => {
      setAiStatus("streaming");
    });
    expect(
      screen.getByTestId("ai-status-indicator").textContent?.toLowerCase(),
    ).toContain("streaming");

    // The turn ends — status falls back to idle.
    await act(async () => {
      setAiStatus("idle");
    });
    expect(
      screen.getByTestId("ai-status-indicator").textContent?.toLowerCase(),
    ).toContain("idle");
  });

  it("shows the error status when the turn fails", async () => {
    await renderModeIndicator();

    await act(async () => {
      setAiStatus("error");
    });
    expect(
      screen.getByTestId("ai-status-indicator").textContent?.toLowerCase(),
    ).toContain("error");
  });

  it("renders the bottom bar even outside vim mode so the AI status is visible", async () => {
    // The keymap defaults to `cua`; the bar must still render because it now
    // carries the AI status, not just the vim mode label.
    await renderModeIndicator();

    await act(async () => {
      setAiStatus("streaming");
    });
    // The bar host is present.
    expect(screen.getByTestId("mode-indicator")).not.toBeNull();
    // …but the vim-only mode label is absent in CUA mode.
    expect(screen.queryByTestId("mode-indicator-mode")).toBeNull();
  });
});
