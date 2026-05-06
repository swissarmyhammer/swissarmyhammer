import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, fireEvent, waitFor, act } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";

// Capture focus-changed listeners so the kernel-emit simulation below can
// fire them when the test invokes `spatial_focus`. The default invoke
// implementation is replaced per-test where needed.
type ListenCallback = (event: { payload: unknown }) => void;
const focusListeners: ListenCallback[] = [];

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve()),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((event: string, cb: ListenCallback) => {
    if (event === "focus-changed") focusListeners.push(cb);
    return Promise.resolve(() => {
      const idx = focusListeners.indexOf(cb);
      if (idx >= 0) focusListeners.splice(idx, 1);
    });
  }),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main" }),
}));

/**
 * Default invoke implementation that emits a synthetic `focus-changed`
 * event when `spatial_focus({fq})` is called. This mirrors the real
 * kernel's emit-after-write contract so tests that fire a click and
 * then read the entity-focus store see the post-emit state.
 */
function emitFocusChangedDefault() {
  (invoke as ReturnType<typeof vi.fn>).mockImplementation(
    (cmd: string, args?: unknown) => {
      if (cmd === "spatial_focus") {
        const a = (args ?? {}) as { fq?: string };
        const fq = a.fq ?? null;
        // Sync emit so click-then-read tests see the post-emit state in
        // the same tick. Wrap in `act()` so React flushes the resulting
        // store update.
        if (fq && focusListeners.length > 0) {
          act(() => {
            for (const h of focusListeners) {
              h({
                payload: {
                  window_label: "main",
                  prev_fq: null,
                  next_fq: fq,
                  next_segment: null,
                },
              });
            }
          });
        }
      }
      return Promise.resolve();
    },
  );
}

import {
  EntityFocusProvider,
  useEntityFocus,
  useIsDirectFocus,
  useIsFocused,
} from "@/lib/entity-focus-context";
import { FocusScope, useParentFocusScope } from "./focus-scope";
import { CommandScopeProvider } from "@/lib/command-scope";
import {
  asSegment,
  fqLastSegment,
  type FullyQualifiedMoniker,
} from "@/types/spatial";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { FocusLayer } from "./focus-layer";

/**
 * Shape returned by the backend `list_commands_for_scope`.
 * Used to build mock responses for context menu tests.
 */
interface ResolvedCommand {
  id: string;
  name: string;
  target?: string;
  group: string;
  context_menu: boolean;
  keys?: { vim?: string; cua?: string; emacs?: string };
  available: boolean;
}

/**
 * Helper: configure invoke mock to return the given commands when
 * `list_commands_for_scope` is called, and resolve for everything else.
 */
function mockListCommands(commands: ResolvedCommand[]) {
  (invoke as ReturnType<typeof vi.fn>).mockImplementation(
    (cmd: string, args?: unknown) => {
      if (cmd === "list_commands_for_scope") return Promise.resolve(commands);
      if (cmd === "spatial_focus") {
        const a = (args ?? {}) as { fq?: string };
        const fq = a.fq ?? null;
        if (fq && focusListeners.length > 0) {
          act(() => {
            for (const h of focusListeners) {
              h({
                payload: {
                  window_label: "main",
                  prev_fq: null,
                  next_fq: fq,
                  next_segment: null,
                },
              });
            }
          });
        }
      }
      return Promise.resolve();
    },
  );
}

/** Helper to read focus state from inside the provider.
 *
 * The entity-focus store holds the focused `FullyQualifiedMoniker`; for
 * test assertions that compare against a relative segment we read the
 * trailing segment via `fqLastSegment`. */
function FocusReader() {
  const { focusedFq } = useEntityFocus();
  const segment = focusedFq ? fqLastSegment(focusedFq) : null;
  return <div data-testid="focus-reader">{segment ?? "null"}</div>;
}

function renderWithFocus(ui: React.ReactElement) {
  return render(
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <EntityFocusProvider>
          <FocusReader />
          {ui}
        </EntityFocusProvider>
      </FocusLayer>
    </SpatialFocusProvider>,
  );
}

describe("FocusScope", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    focusListeners.length = 0;
    emitFocusChangedDefault();
  });

  it("click sets entity focus to moniker", () => {
    const { getByTestId, getByText } = renderWithFocus(
      <FocusScope moniker={asSegment("task:abc")} commands={[]}>
        <span>card</span>
      </FocusScope>,
    );
    fireEvent.click(getByText("card"));
    expect(getByTestId("focus-reader").textContent).toBe("task:abc");
  });

  it("right-click sets entity focus and calls show_context_menu", async () => {
    mockListCommands([
      {
        id: "entity.inspect",
        name: "Inspect",
        group: "entity",
        context_menu: true,
        available: true,
      },
    ]);
    const execute = vi.fn();
    const { getByTestId, getByText } = renderWithFocus(
      <FocusScope
        moniker={asSegment("task:abc")}
        commands={[
          { id: "entity.inspect", name: "Inspect", contextMenu: true, execute },
        ]}
      >
        <span>card</span>
      </FocusScope>,
    );
    fireEvent.contextMenu(getByText("card"));
    expect(getByTestId("focus-reader").textContent).toBe("task:abc");
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("show_context_menu", {
        items: [
          expect.objectContaining({
            cmd: "entity.inspect",
            name: "Inspect",
            separator: false,
          }),
        ],
      });
    });
  });

  it("clicking input inside does not change entity focus", () => {
    const { getByTestId, getByRole } = renderWithFocus(
      <FocusScope moniker={asSegment("task:abc")} commands={[]}>
        <input type="text" />
      </FocusScope>,
    );
    fireEvent.click(getByRole("textbox"));
    expect(getByTestId("focus-reader").textContent).toBe("null");
  });

  it("nested FocusScope: inner click sets inner moniker", () => {
    const { getByTestId, getByText } = renderWithFocus(
      <FocusScope moniker={asSegment("task:abc")} commands={[]}>
        <span>card</span>
        <FocusScope moniker={asSegment("tag:xyz")} commands={[]}>
          <span>tag</span>
        </FocusScope>
      </FocusScope>,
    );
    fireEvent.click(getByText("tag"));
    expect(getByTestId("focus-reader").textContent).toBe("tag:xyz");
  });

  it("nested FocusScope: inner right-click stops propagation", async () => {
    // Backend returns both inner and outer commands (scope chain walks up on backend)
    mockListCommands([
      {
        id: "inner.cmd",
        name: "Inner",
        group: "inner",
        context_menu: true,
        available: true,
      },
      {
        id: "outer.cmd",
        name: "Outer",
        group: "outer",
        context_menu: true,
        available: true,
      },
    ]);
    const outerExec = vi.fn();
    const innerExec = vi.fn();
    const { getByText } = renderWithFocus(
      <FocusScope
        moniker={asSegment("task:abc")}
        commands={[
          {
            id: "outer.cmd",
            name: "Outer",
            contextMenu: true,
            execute: outerExec,
          },
        ]}
      >
        <span>card</span>
        <FocusScope
          moniker={asSegment("tag:xyz")}
          commands={[
            {
              id: "inner.cmd",
              name: "Inner",
              contextMenu: true,
              execute: innerExec,
            },
          ]}
        >
          <span>tag</span>
        </FocusScope>
      </FocusScope>,
    );
    fireEvent.contextMenu(getByText("tag"));
    // show_context_menu should be called exactly once (inner scope handles it, stopPropagation prevents outer)
    await waitFor(() => {
      const ctxCalls = (invoke as ReturnType<typeof vi.fn>).mock.calls.filter(
        (c: unknown[]) => c[0] === "show_context_menu",
      );
      expect(ctxCalls).toHaveLength(1);
      const call = ctxCalls[0];
      // Inner scope should show both inner and outer commands (scope chain walks up on backend)
      const items = call[1].items;
      expect(
        items.find((i: { cmd: string }) => i.cmd === "inner.cmd"),
      ).toBeTruthy();
      expect(
        items.find((i: { cmd: string }) => i.cmd === "outer.cmd"),
      ).toBeTruthy();
    });
  });

  it("nested FocusScope: same command ID without target shadows — inner wins", async () => {
    // Backend handles shadowing: only inner command returned
    mockListCommands([
      {
        id: "entity.inspect",
        name: "Inspect tag",
        group: "entity",
        context_menu: true,
        available: true,
      },
    ]);
    const outerExec = vi.fn();
    const innerExec = vi.fn();
    const { getByText } = renderWithFocus(
      <FocusScope
        moniker={asSegment("task:abc")}
        commands={[
          {
            id: "entity.inspect",
            name: "Inspect task",
            contextMenu: true,
            execute: outerExec,
          },
        ]}
      >
        <span>card</span>
        <FocusScope
          moniker={asSegment("tag:xyz")}
          commands={[
            {
              id: "entity.inspect",
              name: "Inspect tag",
              contextMenu: true,
              execute: innerExec,
            },
          ]}
        >
          <span>tag</span>
        </FocusScope>
      </FocusScope>,
    );
    fireEvent.contextMenu(getByText("tag"));
    await waitFor(() => {
      const ctxCalls = (invoke as ReturnType<typeof vi.fn>).mock.calls.filter(
        (c: unknown[]) => c[0] === "show_context_menu",
      );
      expect(ctxCalls).toHaveLength(1);
      const call = ctxCalls[0];
      const items = call[1].items;
      // No target -> shadow by id alone: inner "Inspect tag" shadows outer "Inspect task"
      expect(items).toHaveLength(1);
      expect(items[0]).toEqual(
        expect.objectContaining({
          cmd: "entity.inspect",
          name: "Inspect tag",
          separator: false,
        }),
      );
    });
  });

  it("nested FocusScope: same command ID with different targets accumulates both", async () => {
    // Backend returns both commands with different targets
    mockListCommands([
      {
        id: "entity.inspect",
        name: "Inspect tag",
        target: "tag:xyz",
        group: "entity",
        context_menu: true,
        available: true,
      },
      {
        id: "entity.inspect",
        name: "Inspect task",
        target: "task:abc",
        group: "entity",
        context_menu: true,
        available: true,
      },
    ]);
    const outerExec = vi.fn();
    const innerExec = vi.fn();
    const { getByText } = renderWithFocus(
      <FocusScope
        moniker={asSegment("task:abc")}
        commands={[
          {
            id: "entity.inspect",
            name: "Inspect task",
            target: "task:abc",
            contextMenu: true,
            execute: outerExec,
          },
        ]}
      >
        <span>card</span>
        <FocusScope
          moniker={asSegment("tag:xyz")}
          commands={[
            {
              id: "entity.inspect",
              name: "Inspect tag",
              target: "tag:xyz",
              contextMenu: true,
              execute: innerExec,
            },
          ]}
        >
          <span>tag</span>
        </FocusScope>
      </FocusScope>,
    );
    fireEvent.contextMenu(getByText("tag"));
    await waitFor(() => {
      const ctxCalls = (invoke as ReturnType<typeof vi.fn>).mock.calls.filter(
        (c: unknown[]) => c[0] === "show_context_menu",
      );
      expect(ctxCalls).toHaveLength(1);
      const call = ctxCalls[0];
      const items = call[1].items;
      // Different targets -> both accumulate
      const commandItems = items.filter(
        (i: { separator: boolean }) => !i.separator,
      );
      expect(commandItems).toHaveLength(2);
      expect(
        commandItems.find((i: { name: string }) => i.name === "Inspect tag"),
      ).toBeTruthy();
      expect(
        commandItems.find((i: { name: string }) => i.name === "Inspect task"),
      ).toBeTruthy();
    });
  });

  it("nested FocusScope: same command ID with same target shadows — inner wins", async () => {
    // Backend handles shadowing: only inner command returned
    mockListCommands([
      {
        id: "entity.inspect",
        name: "Inspect task inner",
        target: "task:abc",
        group: "entity",
        context_menu: true,
        available: true,
      },
    ]);
    const outerExec = vi.fn();
    const innerExec = vi.fn();
    const { getByText } = renderWithFocus(
      <FocusScope
        moniker={asSegment("task:abc")}
        commands={[
          {
            id: "entity.inspect",
            name: "Inspect task outer",
            target: "task:abc",
            contextMenu: true,
            execute: outerExec,
          },
        ]}
      >
        <span>card</span>
        <FocusScope
          moniker={asSegment("tag:xyz")}
          commands={[
            {
              id: "entity.inspect",
              name: "Inspect task inner",
              target: "task:abc",
              contextMenu: true,
              execute: innerExec,
            },
          ]}
        >
          <span>tag</span>
        </FocusScope>
      </FocusScope>,
    );
    fireEvent.contextMenu(getByText("tag"));
    await waitFor(() => {
      const ctxCalls = (invoke as ReturnType<typeof vi.fn>).mock.calls.filter(
        (c: unknown[]) => c[0] === "show_context_menu",
      );
      expect(ctxCalls).toHaveLength(1);
      const call = ctxCalls[0];
      const items = call[1].items;
      // Same target -> shadow: inner wins
      expect(items).toHaveLength(1);
      expect(items[0].name).toBe("Inspect task inner");
    });
  });

  it("nested FocusScope: unavailable inner command blocks same (id, target) from parent", async () => {
    // Backend returns empty list when inner blocks outer
    mockListCommands([]);
    const outerExec = vi.fn();
    const { getByText } = renderWithFocus(
      <FocusScope
        moniker={asSegment("task:abc")}
        commands={[
          {
            id: "entity.inspect",
            name: "Inspect task",
            contextMenu: true,
            execute: outerExec,
          },
        ]}
      >
        <span>card</span>
        <FocusScope
          moniker={asSegment("tag:xyz")}
          commands={[
            {
              id: "entity.inspect",
              name: "Inspect tag",
              contextMenu: true,
              available: false,
            },
          ]}
        >
          <span>tag</span>
        </FocusScope>
      </FocusScope>,
    );
    fireEvent.contextMenu(getByText("tag"));
    // Allow the async list_commands_for_scope call to settle
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "list_commands_for_scope",
        expect.anything(),
      );
    });
    // Backend returns empty: no context menu shown.
    expect(invoke).not.toHaveBeenCalledWith(
      "show_context_menu",
      expect.anything(),
    );
  });

  // Double-click → `ui.inspect` is no longer a `<FocusScope>` concern.
  // It moved to the `<Inspectable>` wrapper component (see
  // `inspectable.tsx`); the unit tests for the dispatch contract live
  // alongside it in `inspectable.spatial.test.tsx`. `<FocusScope>` is
  // a pure spatial primitive — it owns click → spatial focus and
  // right-click → context menu, nothing more.

  it("data-focused attribute set when focused", () => {
    const { container, getByText } = renderWithFocus(
      <FocusScope moniker={asSegment("task:abc")} commands={[]}>
        <span>card</span>
      </FocusScope>,
    );
    fireEvent.click(getByText("card"));
    const scopeDiv = container.querySelector("[data-segment='task:abc']");
    expect(scopeDiv?.hasAttribute("data-focused")).toBe(true);
  });

  it("data-focused attribute absent when not focused", () => {
    const { container } = renderWithFocus(
      <FocusScope moniker={asSegment("task:abc")} commands={[]}>
        <span>card</span>
      </FocusScope>,
    );
    const scopeDiv = container.querySelector("[data-segment='task:abc']");
    expect(scopeDiv?.hasAttribute("data-focused")).toBe(false);
  });

  it("data-moniker attribute always set", () => {
    const { container } = renderWithFocus(
      <FocusScope moniker={asSegment("task:abc")} commands={[]}>
        <span>card</span>
      </FocusScope>,
    );
    const scopeDiv = container.querySelector("[data-segment='task:abc']");
    expect(scopeDiv).not.toBeNull();
    // After path-monikers, `data-moniker` carries the FQM and `data-segment`
    // carries the relative segment. The legacy contract was that
    // `data-moniker` was the segment; tests asserting the segment shape now
    // read `data-segment`.
    expect(scopeDiv?.getAttribute("data-segment")).toBe("task:abc");
    expect(scopeDiv?.getAttribute("data-moniker")).toMatch(/task:abc$/);
  });

  it("commands are provided to CommandScopeProvider", async () => {
    mockListCommands([
      {
        id: "entity.inspect",
        name: "Inspect",
        group: "entity",
        context_menu: true,
        available: true,
      },
    ]);
    const execute = vi.fn();
    const { getByText } = renderWithFocus(
      <FocusScope
        moniker={asSegment("task:abc")}
        commands={[
          { id: "entity.inspect", name: "Inspect", contextMenu: true, execute },
        ]}
      >
        <span>card</span>
      </FocusScope>,
    );
    fireEvent.contextMenu(getByText("card"));
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("show_context_menu", {
        items: [
          expect.objectContaining({
            cmd: "entity.inspect",
            name: "Inspect",
            separator: false,
          }),
        ],
      });
    });
  });

  it("scope is registered on mount and deregistered on unmount", () => {
    // The registry is a ref, so we use a probe that reads getScope imperatively.
    let probeGetScope: ((m: string) => unknown) | null = null;
    function ScopeProbe() {
      const { getScope } = useEntityFocus();
      probeGetScope = getScope;
      return null;
    }

    const { unmount } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asSegment("window")}>
          <EntityFocusProvider>
            <ScopeProbe />
            <FocusScope moniker={asSegment("task:abc")} commands={[]}>
              <span>card</span>
            </FocusScope>
          </EntityFocusProvider>
        </FocusLayer>
      </SpatialFocusProvider>,
    );

    // The entity-focus registry is keyed by the composed FQM (the kernel
    // identifier), not the bare segment — `<FocusLayer name="window">`
    // wraps the scope so its key is `/window/task:abc`.
    const scopeFq = "/window/task:abc";
    // After mount + effects, scope should be registered in the ref
    expect(probeGetScope!(scopeFq)).not.toBeNull();
    unmount();
    // After unmount, the cleanup should have deregistered
    expect(probeGetScope!(scopeFq)).toBeNull();
  });

  it("showFocus=false still fires context menu (handleEvents defaults true)", async () => {
    mockListCommands([
      {
        id: "tag.inspect",
        name: "Inspect tag",
        group: "tag",
        context_menu: true,
        available: true,
      },
    ]);
    const execute = vi.fn();
    const { getByText } = renderWithFocus(
      <FocusScope
        moniker={asSegment("tag:xyz")}
        showFocus={false}
        commands={[
          {
            id: "tag.inspect",
            name: "Inspect tag",
            contextMenu: true,
            execute,
          },
        ]}
      >
        <span>tag pill</span>
      </FocusScope>,
    );
    fireEvent.contextMenu(getByText("tag pill"));
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("show_context_menu", {
        items: [
          expect.objectContaining({
            cmd: "tag.inspect",
            name: "Inspect tag",
          }),
        ],
      });
    });
  });

  it("handleEvents=false suppresses context menu even with showFocus=true", async () => {
    mockListCommands([
      {
        id: "tag.inspect",
        name: "Inspect tag",
        group: "tag",
        context_menu: true,
        available: true,
      },
    ]);
    const execute = vi.fn();
    const { getByText } = renderWithFocus(
      <FocusScope
        moniker={asSegment("tag:xyz")}
        showFocus={true}
        handleEvents={false}
        commands={[
          {
            id: "tag.inspect",
            name: "Inspect tag",
            contextMenu: true,
            execute,
          },
        ]}
      >
        <span>tag pill</span>
      </FocusScope>,
    );
    fireEvent.contextMenu(getByText("tag pill"));
    // Give time for any async calls to settle
    await new Promise((r) => setTimeout(r, 50));
    expect(invoke).not.toHaveBeenCalledWith(
      "show_context_menu",
      expect.anything(),
    );
  });

  describe("useParentFocusScope", () => {
    /** Helper that reads useParentFocusScope and renders the value. */
    function ParentScopeReader() {
      const parentMoniker = useParentFocusScope();
      const segment = parentMoniker ? fqLastSegment(parentMoniker) : null;
      return <span data-testid="parent-scope">{segment ?? "null"}</span>;
    }

    it("returns parent FocusScope moniker", () => {
      const { getByTestId } = render(
        <SpatialFocusProvider>
          <FocusLayer name={asSegment("window")}>
            <EntityFocusProvider>
              <FocusScope moniker={asSegment("column:col1")} commands={[]}>
                <ParentScopeReader />
              </FocusScope>
            </EntityFocusProvider>
          </FocusLayer>
        </SpatialFocusProvider>,
      );
      expect(getByTestId("parent-scope").textContent).toBe("column:col1");
    });

    it("skips CommandScopeProvider, returns grandparent FocusScope moniker", () => {
      const { getByTestId } = render(
        <SpatialFocusProvider>
          <FocusLayer name={asSegment("window")}>
            <EntityFocusProvider>
              <FocusScope moniker={asSegment("column:col1")} commands={[]}>
                <CommandScopeProvider
                  commands={[]}
                  moniker={asSegment("inner-cmd")}
                >
                  <ParentScopeReader />
                </CommandScopeProvider>
              </FocusScope>
            </EntityFocusProvider>
          </FocusLayer>
        </SpatialFocusProvider>,
      );
      // CommandScopeProvider is NOT a FocusScope, so context still shows the FocusScope ancestor
      expect(getByTestId("parent-scope").textContent).toBe("column:col1");
    });

    it("returns null at root", () => {
      const { getByTestId } = render(
        <SpatialFocusProvider>
          <FocusLayer name={asSegment("window")}>
            <EntityFocusProvider>
              <ParentScopeReader />
            </EntityFocusProvider>
          </FocusLayer>
        </SpatialFocusProvider>,
      );
      expect(getByTestId("parent-scope").textContent).toBe("null");
    });
  });

  it("useIsFocused ancestor: column gets data-focused when card inside is focused", () => {
    /** Column component that reads useIsFocused. */
    function ColumnWithFocus({
      moniker,
      children,
    }: {
      moniker: string;
      children: React.ReactNode;
    }) {
      const focused = useIsFocused(moniker);
      return (
        <div data-testid={`col-${moniker}`} data-focused={focused || undefined}>
          {children}
        </div>
      );
    }

    const { getByTestId, getByText } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asSegment("window")}>
          <EntityFocusProvider>
            <FocusScope moniker={asSegment("column:col1")} commands={[]}>
              <ColumnWithFocus moniker={asSegment("column:col1")}>
                <FocusScope moniker={asSegment("task:abc")} commands={[]}>
                  <span>card</span>
                </FocusScope>
              </ColumnWithFocus>
            </FocusScope>
          </EntityFocusProvider>
        </FocusLayer>
      </SpatialFocusProvider>,
    );

    // Click the card to focus task:abc
    fireEvent.click(getByText("card"));

    // The column should also show as focused via ancestor walk
    expect(getByTestId("col-column:col1").hasAttribute("data-focused")).toBe(
      true,
    );
  });

  /**
   * Selective-rerender regression test.
   *
   * FocusScope subscribes to its own moniker's focus slot via useIsDirectFocus,
   * so moving focus from A to B must wake exactly the A scope and the B scope —
   * NOT the three unrelated scopes in the tree. This is what takes per-arrow-
   * key renders in a 12k-cell grid from 12k down to exactly 2.
   *
   * Each FocusScope owns a counter child whose render function calls
   * `useIsDirectFocus(moniker)` itself, so the counter IS the subscribed
   * consumer — its render count equals the number of times the focus slot
   * for that moniker was notified. Passing the counter via `children`
   * instead of a distinct subscribed component would measure nothing,
   * because React reuses identical `children` element references across
   * the parent's re-renders (the classic memoization-via-children trick).
   */
  it("FocusScope re-renders exactly when its own moniker's focus state flips", () => {
    const monikers = [
      asSegment("scope:a"),
      asSegment("scope:b"),
      asSegment("scope:c"),
      asSegment("scope:d"),
      asSegment("scope:e"),
    ] as const;

    // After wrapping in `<FocusLayer name="window">`, each scope registers
    // under its composed FQM (`/window/scope:a`). The counters subscribe
    // and the SetFocus buttons write the same FQMs so the subscription
    // keys line up across all three sites.
    const fqs = monikers.map(
      (m) => `/window/${m}` as FullyQualifiedMoniker,
    ) as readonly FullyQualifiedMoniker[];

    const counts: Record<string, number> = Object.fromEntries(
      fqs.map((fq) => [fq, 0]),
    );

    /**
     * Standalone subscribed counter — calls `useIsDirectFocus` itself so
     * the subscription graph exactly mirrors what FocusScope does. Every
     * render increments the per-moniker counter.
     */
    function SubscribedCounter({ moniker }: { moniker: string }) {
      const focused = useIsDirectFocus(moniker);
      counts[moniker] += 1;
      return (
        <span data-testid={`counter-${moniker}`}>{focused ? "yes" : "no"}</span>
      );
    }

    /** Helper button to set focus via the hot-path `setFocus`. */
    function SetFocus({ moniker }: { moniker: FullyQualifiedMoniker | null }) {
      const { setFocus } = useEntityFocus();
      return (
        <button
          data-testid={`set-${moniker ?? "null"}`}
          onClick={() => setFocus(moniker)}
        />
      );
    }

    const { getByTestId } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asSegment("window")}>
          <EntityFocusProvider>
            {monikers.map((m) => (
              <FocusScope key={m} moniker={m}>
                <span>scope</span>
              </FocusScope>
            ))}
            {fqs.map((fq) => (
              <SubscribedCounter key={`sub-${fq}`} moniker={fq} />
            ))}
            <SetFocus moniker={fqs[0]} />
            <SetFocus moniker={fqs[1]} />
          </EntityFocusProvider>
        </FocusLayer>
      </SpatialFocusProvider>,
    );

    // Baseline: capture mount-time render counts so the assertion below
    // measures only the additional renders triggered by focus moves.
    // (SubscribedCounter is the fixture that uses useIsFocused here;
    // FocusScope uses the same underlying store via useIsDirectFocus, so
    // the selective-wake behavior is identical — this counter proves it.)
    const base = { ...counts };

    fireEvent.click(getByTestId(`set-${fqs[0]}`));
    // Only scope:a's subscription slot fired.
    expect(counts[fqs[0]]).toBe(base[fqs[0]] + 1);
    expect(counts[fqs[1]]).toBe(base[fqs[1]]);
    expect(counts[fqs[2]]).toBe(base[fqs[2]]);
    expect(counts[fqs[3]]).toBe(base[fqs[3]]);
    expect(counts[fqs[4]]).toBe(base[fqs[4]]);

    fireEvent.click(getByTestId(`set-${fqs[1]}`));
    // Moving focus A -> B wakes A (lost) and B (gained), nothing else.
    expect(counts[fqs[0]]).toBe(base[fqs[0]] + 2);
    expect(counts[fqs[1]]).toBe(base[fqs[1]] + 1);
    expect(counts[fqs[2]]).toBe(base[fqs[2]]);
    expect(counts[fqs[3]]).toBe(base[fqs[3]]);
    expect(counts[fqs[4]]).toBe(base[fqs[4]]);
  });

  /**
   * Composition tests — verify FocusScope is the leaf primitive,
   * forwards `navOverride` through to the registration call, and routes
   * click to the primitive's `spatial_focus` invoke. Every test in this
   * file mounts the primitive inside the spatial provider stack
   * (`SpatialFocusProvider` + `FocusLayer`) — the no-spatial-context
   * fallback path was removed in card `01KQPVA127YMJ8D7NB6M824595`, so
   * mounting `<FocusScope>` without `<FocusLayer>` now throws.
   */
  describe("spatial-context registration", () => {
    it("registers via spatial_register_scope when wrapped in <FocusLayer>", async () => {
      const { container } = render(
        <SpatialFocusProvider>
          <FocusLayer name={asSegment("window")}>
            <EntityFocusProvider>
              <FocusScope moniker={asSegment("task:abc")}>
                <span>card</span>
              </FocusScope>
            </EntityFocusProvider>
          </FocusLayer>
        </SpatialFocusProvider>,
      );

      // After parent task `01KQSDP4ZJY5ERAJ68TFPVFRRE` collapsed the
      // legacy split primitives into a single `<FocusScope>`, the only
      // registration command is `spatial_register_scope`. A scope with
      // no children behaves as a leaf; a scope with children behaves as
      // a navigable container.
      await waitFor(() => {
        expect(invoke).toHaveBeenCalledWith(
          "spatial_register_scope",
          expect.objectContaining({ segment: "task:abc" }),
        );
      });

      // The primitive's div carries data-moniker
      const node = container.querySelector("[data-segment='task:abc']");
      expect(node).not.toBeNull();
    });

    it("registers via spatial_register_scope regardless of moniker prefix", async () => {
      // Pre-collapse this test asserted that `column:` monikers
      // registered as zones while `task:` monikers registered as leaves.
      // Under the unified primitive there is one registration command
      // (`spatial_register_scope`); structural shape is determined by
      // whether the scope has child scopes, not by the moniker prefix or
      // a kind discriminator.
      const { container } = render(
        <SpatialFocusProvider>
          <FocusLayer name={asSegment("window")}>
            <EntityFocusProvider>
              <FocusScope moniker={asSegment("column:doing")}>
                <span>body</span>
              </FocusScope>
            </EntityFocusProvider>
          </FocusLayer>
        </SpatialFocusProvider>,
      );

      await waitFor(() => {
        expect(invoke).toHaveBeenCalledWith(
          "spatial_register_scope",
          expect.objectContaining({ segment: "column:doing" }),
        );
      });

      const node = container.querySelector("[data-segment='column:doing']");
      expect(node).not.toBeNull();
    });

    it("forwards navOverride to the primitive registration", async () => {
      const navOverride = { left: null };

      render(
        <SpatialFocusProvider>
          <FocusLayer name={asSegment("window")}>
            <EntityFocusProvider>
              <FocusScope
                moniker={asSegment("task:abc")}
                navOverride={navOverride}
              >
                <span>card</span>
              </FocusScope>
            </EntityFocusProvider>
          </FocusLayer>
        </SpatialFocusProvider>,
      );

      await waitFor(() => {
        expect(invoke).toHaveBeenCalledWith(
          "spatial_register_scope",
          expect.objectContaining({
            segment: "task:abc",
            overrides: { left: null },
          }),
        );
      });
    });

    it("click invokes spatial_focus with the primitive's key", async () => {
      const { getByText } = render(
        <SpatialFocusProvider>
          <FocusLayer name={asSegment("window")}>
            <EntityFocusProvider>
              <FocusScope moniker={asSegment("task:abc")}>
                <span>card</span>
              </FocusScope>
            </EntityFocusProvider>
          </FocusLayer>
        </SpatialFocusProvider>,
      );

      // Wait for the primitive to register so we can pull its key out
      // of the register call args.
      await waitFor(() => {
        expect(invoke).toHaveBeenCalledWith(
          "spatial_register_scope",
          expect.anything(),
        );
      });
      const registerCall = (invoke as ReturnType<typeof vi.fn>).mock.calls.find(
        (c) => c[0] === "spatial_register_scope",
      );
      const registeredKey = (registerCall![1] as { fq: string }).fq;

      // Click the rendered leaf — the primitive's onClick fires
      // `spatial_focus` with the key it minted on mount.
      fireEvent.click(getByText("card"));

      await waitFor(() => {
        expect(invoke).toHaveBeenCalledWith("spatial_focus", {
          fq: registeredKey,
        });
      });
    });

    it("throws when <FocusScope> is mounted without <FocusLayer>", () => {
      // Mounting a `<FocusScope>` outside the spatial provider stack is a
      // setup bug, not a supported mode. The primitive must surface the
      // missing `<FocusLayer>` ancestor as a clear error rather than
      // silently rendering a plain `<div>` (the legacy fallback). Suppress
      // React's auto-logging of the thrown error so the test output stays
      // readable; we still assert the throw via `expect(...).toThrow`.
      const consoleError = vi
        .spyOn(console, "error")
        .mockImplementation(() => {});
      try {
        expect(() =>
          render(
            <EntityFocusProvider>
              <FocusScope moniker={asSegment("task:abc")}>
                <span>card</span>
              </FocusScope>
            </EntityFocusProvider>,
          ),
        ).toThrow(/FocusLayer/);
      } finally {
        consoleError.mockRestore();
      }
    });

    /**
     * Layout regression guard for the FocusScope chrome composition.
     *
     * Earlier revisions wrapped children in an internal body div whose
     * default block layout broke the flex chain when consumers passed
     * `<FocusScope className="flex …">`. The collapse landed the chrome
     * (right-click / double-click / scrollIntoView) on the same `<div>`
     * the spatial primitive registers with, so the consumer's
     * `className` lands on a single element that hosts the children —
     * they become direct layout children of that element.
     *
     * The assertion below pins that contract: when the consumer asks for
     * `flex flex-row`, the children must be direct DOM children of the
     * single `<div>` (`data-moniker='task:row'`). A regression that
     * re-introduces an inner wrapper would push the children one layer
     * deeper and this test would catch it before any call site re-grew
     * its `outer-flex + inner-flex` workaround.
     */
    it("flex className lays children as direct flex items (no inner wrapper)", async () => {
      const { container } = render(
        <SpatialFocusProvider>
          <FocusLayer name={asSegment("window")}>
            <EntityFocusProvider>
              <FocusScope
                moniker={asSegment("task:row")}
                className="flex flex-row"
              >
                <span data-testid="child-a">a</span>
                <span data-testid="child-b">b</span>
              </FocusScope>
            </EntityFocusProvider>
          </FocusLayer>
        </SpatialFocusProvider>,
      );

      // Wait for the primitive to mount so the div carries its className
      // (registration is async but the JSX render is sync).
      await waitFor(() => {
        expect(invoke).toHaveBeenCalledWith(
          "spatial_register_scope",
          expect.objectContaining({ segment: "task:row" }),
        );
      });

      const node = container.querySelector(
        "[data-segment='task:row']",
      ) as HTMLElement | null;
      expect(node).not.toBeNull();
      // Consumer's className lands on the primitive's root div.
      expect(node!.className).toContain("flex");
      expect(node!.className).toContain("flex-row");

      // Both children must be DIRECT DOM children of the scope div — no
      // intervening wrapper element. If a future refactor re-introduces
      // an inner div, the children's parentElement would be that wrapper
      // instead of the scope, and the assertions below would fail.
      const childA = container.querySelector(
        '[data-testid="child-a"]',
      ) as HTMLElement | null;
      const childB = container.querySelector(
        '[data-testid="child-b"]',
      ) as HTMLElement | null;
      expect(childA?.parentElement).toBe(node);
      expect(childB?.parentElement).toBe(node);
    });
  });
});
