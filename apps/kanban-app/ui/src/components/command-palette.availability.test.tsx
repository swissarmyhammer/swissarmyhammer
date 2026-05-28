/**
 * Palette wiring via the metadata-driven Command registry (card
 * `01KS36XGKCQ36QM7P6MH3FHMBJ`).
 *
 * The palette sources its rows from `useCommandList` and evaluates
 * `available command` for every visible row on open. These tests mock the two
 * seams — the command list and the availability transport — and assert:
 *
 *   - all 20 registry commands render (no hardcoded list),
 *   - a command whose `available command` verdict is `false` renders
 *     grayed-out (`data-available="false"`), and
 *   - its `reason` is surfaced as the row's tooltip (`title`).
 *
 * Real timers are used — the suite runs in real Chromium where fake timers
 * deadlock `waitFor`. The availability fan-out resolves on microtasks, so the
 * assertions just `waitFor` the grayed-out row to settle.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

// Mock Tauri core — the palette still calls get_ui_state (keymap mode) and
// would call search_entities in search mode (unused here).
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn((cmd: string) => {
    if (cmd === "get_ui_state")
      return Promise.resolve({
        palette_open: false,
        palette_mode: "command",
        keymap_mode: "cua",
        scope_chain: [],
        open_boards: [],
        windows: {},
        recent_boards: [],
      });
    return Promise.resolve(null);
  }),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main", setFocus: vi.fn() }),
}));
vi.mock("@replit/codemirror-vim", async () => {
  const actual = await vi.importActual<typeof import("@replit/codemirror-vim")>(
    "@replit/codemirror-vim",
  );
  return {
    ...actual,
    getCM: vi.fn(() => ({ state: { vim: {} } }) as any),
    Vim: { ...actual.Vim, handleKey: vi.fn(), exitInsertMode: vi.fn() },
  };
});

// Drive the palette's command source: useCommandList returns a fixed registry.
import type { CommandMetadata } from "@/hooks/use-command-list";
const REGISTRY: CommandMetadata[] = Array.from({ length: 20 }, (_, i) => ({
  id: `cmd.${i}`,
  name: `Command ${i}`,
}));
vi.mock("@/hooks/use-command-list", () => ({
  useCommandList: () => ({ commands: REGISTRY, loading: false, refresh: vi.fn() }),
}));

// Drive availability: cmd.7 is unavailable with a reason; the rest available.
const UNAVAILABLE_ID = "cmd.7";
const UNAVAILABLE_REASON = "No selection to act on";
vi.mock("@/lib/mcp-transport", async () => {
  const actual =
    await vi.importActual<typeof import("@/lib/mcp-transport")>(
      "@/lib/mcp-transport",
    );
  return {
    ...actual,
    evaluateAvailableCommand: vi.fn((id: string) =>
      id === UNAVAILABLE_ID
        ? Promise.resolve({ available: false, reason: UNAVAILABLE_REASON })
        : Promise.resolve({ available: true }),
    ),
  };
});

import { CommandPalette } from "./command-palette";
import { CommandScopeProvider } from "@/lib/command-scope";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { UIStateProvider } from "@/lib/ui-state-context";

function renderPalette() {
  return render(
    <EntityFocusProvider>
      <UIStateProvider>
        <CommandScopeProvider commands={[]}>
          <CommandPalette open onClose={vi.fn()} />
        </CommandScopeProvider>
      </UIStateProvider>
    </EntityFocusProvider>,
  );
}

describe("CommandPalette registry + availability wiring", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders all 20 commands from useCommandList", async () => {
    renderPalette();
    await waitFor(() => {
      for (let i = 0; i < 20; i++) {
        expect(screen.getByText(`Command ${i}`)).toBeTruthy();
      }
    });
  });

  it("grays out a command whose availability is false and shows its reason as a tooltip", async () => {
    renderPalette();

    // The unavailable row settles to data-available="false" once the
    // batched `available command` fan-out resolves.
    await waitFor(() => {
      const row = screen.getByTestId(`command-item-${UNAVAILABLE_ID}`);
      expect(row.getAttribute("data-available")).toBe("false");
    });

    const row = screen.getByTestId(`command-item-${UNAVAILABLE_ID}`);
    expect(row.getAttribute("title")).toBe(UNAVAILABLE_REASON);
    expect(row.getAttribute("aria-disabled")).toBe("true");

    // A sibling command stays available (no false flicker).
    const available = screen.getByTestId("command-item-cmd.0");
    expect(available.getAttribute("data-available")).toBe("true");
    expect(available.getAttribute("title")).toBeNull();
  });
});
