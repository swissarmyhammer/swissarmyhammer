/**
 * Tests for the `<Pressable>` primitive.
 *
 * Source of truth for acceptance of card `01KQM9BGN0HFQSC168YD9G82Z2`
 * (Add `<Pressable>` primitive: FocusScope leaf + button + Enter/Space
 * activation, then migrate every icon button).
 *
 * `<Pressable>` is the canonical primitive every actionable icon button
 * MUST use. It bundles three concerns:
 *
 *   1. Mounts a `<FocusScope>` leaf so the spatial-nav graph can
 *      navigate to it.
 *   2. Renders a `<button type="button">` (or, via `asChild`, an
 *      arbitrary host slot like `<TooltipTrigger asChild>`).
 *   3. Registers two scope-level CommandDefs so Enter (vim/cua) and
 *      Space (cua) on the focused leaf invoke the same `onPress`
 *      callback as the button's `onClick`.
 *
 * The test harness mirrors `app-shell.test.tsx`'s `renderShell` so the
 * focused-scope chain feeds the global keymap handler through
 * `extractScopeBindings` end-to-end. That is the only way to prove
 * Enter/Space pressed on a focused Pressable actually fires the
 * `onPress` callback — anything narrower would skip the integration
 * point that production traverses.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";
import type { ReactNode } from "react";

// ---------------------------------------------------------------------------
// Tauri API mocks — must come before component imports. Mirrors
// app-shell.test.tsx so the kernel simulator emits `focus-changed`
// when `spatial_focus(fq)` is invoked, which is what wires the
// React-side entity-focus store and lets `useFocusedScope()` populate
// the keymap-handler bindings.
// ---------------------------------------------------------------------------

const monikerToKey = new Map<string, string>();
const currentFocusKey: { key: string | null } = { key: null };
const listenCallbacks: Record<string, (event: unknown) => void> = {};

function defaultInvoke(cmd: string, args?: unknown): Promise<unknown> {
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
  if (cmd === "spatial_register_scope" || cmd === "spatial_register_scope") {
    const a = (args ?? {}) as { fq?: string; segment?: string };
    if (a.fq && a.segment) monikerToKey.set(a.segment, a.fq);
    return Promise.resolve(null);
  }
  if (cmd === "spatial_unregister_scope") {
    const a = (args ?? {}) as { fq?: string };
    if (a.fq) {
      for (const [m, k] of monikerToKey.entries()) {
        if (k === a.fq) {
          monikerToKey.delete(m);
          break;
        }
      }
    }
    return Promise.resolve(null);
  }
  if (cmd === "spatial_drill_in" || cmd === "spatial_drill_out") {
    const a = (args ?? {}) as { focusedFq?: string };
    return Promise.resolve(a.focusedFq ?? null);
  }
  if (cmd === "spatial_focus") {
    const a = (args ?? {}) as { fq?: string };
    const fq = a.fq ?? null;
    let moniker: string | null = null;
    for (const [s, k] of monikerToKey.entries()) {
      if (k === fq) {
        moniker = s;
        break;
      }
    }
    if (fq) {
      const prev = currentFocusKey.key;
      currentFocusKey.key = fq;
      queueMicrotask(() => {
        const cb = listenCallbacks["focus-changed"];
        if (cb) {
          cb({
            payload: {
              window_label: "main",
              prev_fq: prev,
              next_fq: fq,
              next_segment: moniker,
            },
          });
        }
      });
    }
    return Promise.resolve(null);
  }
  return Promise.resolve(null);
}

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn((cmd: string, args?: unknown) => defaultInvoke(cmd, args)),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((eventName: string, cb: (event: unknown) => void) => {
    listenCallbacks[eventName] = cb;
    return Promise.resolve(() => {});
  }),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main" }),
}));
vi.mock("@tauri-apps/plugin-log", () => ({
  error: vi.fn(),
  warn: vi.fn(),
  info: vi.fn(),
  debug: vi.fn(),
  trace: vi.fn(),
  attachConsole: vi.fn(() => Promise.resolve()),
}));

// Imports come after mocks so the mocks land before the modules see
// their dependencies.
import { Pressable } from "./pressable";
import { AppShell } from "./app-shell";
import { FocusLayer } from "./focus-layer";
import { UIStateProvider } from "@/lib/ui-state-context";
import { AppModeProvider } from "@/lib/app-mode-context";
import { UndoProvider } from "@/lib/undo-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import {
  TooltipProvider,
  Tooltip,
  TooltipTrigger,
  TooltipContent,
} from "@/components/ui/tooltip";
import { asSegment } from "@/types/spatial";
import { invoke } from "@tauri-apps/api/core";

const WINDOW_LAYER_NAME = asSegment("window");

/**
 * Render `<Pressable>` inside the production-shaped provider stack so
 * the global keymap handler's `extractScopeBindings(focusedScope)`
 * sees the Pressable's CommandDefs when its leaf is focused.
 */
async function renderPressable(children: ReactNode) {
  let result!: ReturnType<typeof render>;
  await act(async () => {
    result = render(
      <SpatialFocusProvider>
        <FocusLayer name={WINDOW_LAYER_NAME}>
          <EntityFocusProvider>
            <UIStateProvider>
              <AppModeProvider>
                <UndoProvider>
                  <AppShell>
                    <TooltipProvider delayDuration={100}>
                      {children}
                    </TooltipProvider>
                  </AppShell>
                </UndoProvider>
              </AppModeProvider>
            </UIStateProvider>
          </EntityFocusProvider>
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await Promise.resolve();
  });
  return result;
}

/** Wait for register effects scheduled in `useEffect` to flush. */
async function flushSetup() {
  await act(async () => {
    await Promise.resolve();
  });
}

/** Collect every `spatial_register_scope` invocation argument bag. */
function registerScopeArgs(): Array<Record<string, unknown>> {
  const mockInvoke = invoke as unknown as ReturnType<typeof vi.fn>;
  return mockInvoke.mock.calls
    .filter((c: unknown[]) => c[0] === "spatial_register_scope")
    .map((c: unknown[]) => c[1] as Record<string, unknown>);
}

/**
 * Drive a `focus-changed` event into the React tree as if the Rust
 * kernel had emitted one for the current window. Mirrors the helper
 * in `nav-bar.spatial-nav.test.tsx`.
 */
async function fireFocusChangedTo(fq: string, segment: string | null) {
  const cb = listenCallbacks["focus-changed"];
  if (!cb) throw new Error("focus-changed listener not registered yet");
  await act(async () => {
    cb({
      payload: {
        window_label: "main",
        prev_fq: currentFocusKey.key,
        next_fq: fq,
        next_segment: segment,
      },
    });
    currentFocusKey.key = fq;
    await Promise.resolve();
  });
}

describe("Pressable", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    monikerToKey.clear();
    currentFocusKey.key = null;
    for (const key of Object.keys(listenCallbacks)) {
      delete listenCallbacks[key];
    }
  });

  // -------------------------------------------------------------------------
  // Test 1 — clicking the rendered button calls onPress once.
  // -------------------------------------------------------------------------
  it("clicking the button calls onPress once", async () => {
    const onPress = vi.fn();
    await renderPressable(
      <Pressable
        moniker={asSegment("test:click")}
        ariaLabel="Click target"
        onPress={onPress}
      >
        <span>Hello</span>
      </Pressable>,
    );
    await flushSetup();

    const button = screen.getByRole("button", { name: "Click target" });
    fireEvent.click(button);

    expect(onPress).toHaveBeenCalledTimes(1);
  });

  // -------------------------------------------------------------------------
  // Test 2 — focusing the leaf and dispatching Enter calls onPress once.
  // -------------------------------------------------------------------------
  it("Enter on the focused leaf calls onPress once", async () => {
    const onPress = vi.fn();
    await renderPressable(
      <Pressable
        moniker={asSegment("test:enter")}
        ariaLabel="Enter target"
        onPress={onPress}
      >
        <span>Hello</span>
      </Pressable>,
    );
    await flushSetup();

    const leaf = registerScopeArgs().find((a) => a.segment === "test:enter");
    expect(leaf, "Pressable must register as a FocusScope leaf").toBeDefined();

    await fireFocusChangedTo(leaf!.fq as string, "test:enter");

    await act(async () => {
      fireEvent.keyDown(document, { key: "Enter", code: "Enter" });
    });

    expect(onPress).toHaveBeenCalledTimes(1);
  });

  // -------------------------------------------------------------------------
  // Test 3 — focusing the leaf and dispatching Space calls onPress once.
  // -------------------------------------------------------------------------
  it("Space on the focused leaf calls onPress once", async () => {
    const onPress = vi.fn();
    await renderPressable(
      <Pressable
        moniker={asSegment("test:space")}
        ariaLabel="Space target"
        onPress={onPress}
      >
        <span>Hello</span>
      </Pressable>,
    );
    await flushSetup();

    const leaf = registerScopeArgs().find((a) => a.segment === "test:space");
    expect(leaf).toBeDefined();

    await fireFocusChangedTo(leaf!.fq as string, "test:space");

    await act(async () => {
      // Browsers emit `e.key === " "` (a literal space) for the
      // spacebar; `normalizeKeyEvent` canonicalises that to `"Space"`
      // before scope binding lookup.
      fireEvent.keyDown(document, { key: " ", code: "Space" });
    });

    expect(onPress).toHaveBeenCalledTimes(1);
  });

  // -------------------------------------------------------------------------
  // Test 4 — disabled={true}: clicking and Enter both no-op.
  // -------------------------------------------------------------------------
  it("disabled={true} suppresses both click and Enter activation", async () => {
    const onPress = vi.fn();
    await renderPressable(
      <Pressable
        moniker={asSegment("test:disabled")}
        ariaLabel="Disabled target"
        onPress={onPress}
        disabled
      >
        <span>Hello</span>
      </Pressable>,
    );
    await flushSetup();

    const button = screen.getByRole("button", { name: "Disabled target" });
    fireEvent.click(button);

    expect(onPress).not.toHaveBeenCalled();

    const leaf = registerScopeArgs().find((a) => a.segment === "test:disabled");
    expect(leaf).toBeDefined();

    await fireFocusChangedTo(leaf!.fq as string, "test:disabled");

    await act(async () => {
      fireEvent.keyDown(document, { key: "Enter", code: "Enter" });
    });

    expect(onPress).not.toHaveBeenCalled();
  });

  // -------------------------------------------------------------------------
  // Test 5 — asChild={true} composes with TooltipTrigger asChild without
  // creating a double <button>. Click and Enter both still fire onPress.
  // -------------------------------------------------------------------------
  it("asChild composes with TooltipTrigger asChild as a single <button>", async () => {
    const onPress = vi.fn();
    await renderPressable(
      <Tooltip>
        <TooltipTrigger asChild>
          <Pressable
            asChild
            moniker={asSegment("test:aschild")}
            ariaLabel="AsChild target"
            onPress={onPress}
          >
            <button type="button">
              <span>Hello</span>
            </button>
          </Pressable>
        </TooltipTrigger>
        <TooltipContent>tip</TooltipContent>
      </Tooltip>,
    );
    await flushSetup();

    // Exactly one button with the aria-label exists — no double-button
    // from a wrapper button + slotted button.
    const buttons = screen.getAllByRole("button", { name: "AsChild target" });
    expect(buttons).toHaveLength(1);

    fireEvent.click(buttons[0]);
    expect(onPress).toHaveBeenCalledTimes(1);

    onPress.mockClear();

    const leaf = registerScopeArgs().find((a) => a.segment === "test:aschild");
    expect(leaf).toBeDefined();

    await fireFocusChangedTo(leaf!.fq as string, "test:aschild");

    await act(async () => {
      fireEvent.keyDown(document, { key: "Enter", code: "Enter" });
    });

    expect(onPress).toHaveBeenCalledTimes(1);
  });

  // -------------------------------------------------------------------------
  // Test 6 — the Pressable's leaf registration carries the supplied
  // moniker segment (proves the FocusScope wires through correctly).
  // -------------------------------------------------------------------------
  it("registers as spatial_register_scope with the supplied moniker segment", async () => {
    const onPress = vi.fn();
    await renderPressable(
      <Pressable
        moniker={asSegment("test:register")}
        ariaLabel="Register target"
        onPress={onPress}
      >
        <span>Hello</span>
      </Pressable>,
    );
    await flushSetup();

    const leaf = registerScopeArgs().find((a) => a.segment === "test:register");
    expect(
      leaf,
      "Pressable must register a FocusScope leaf with its segment",
    ).toBeDefined();
    expect(typeof leaf!.fq).toBe("string");
  });
});
