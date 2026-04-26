/**
 * Spatial-nav integration tests for `<PerspectiveContainer>`.
 *
 * Mounts the container inside the production-shaped provider stack
 * (`<SpatialFocusProvider>` + `<FocusLayer name="window">`) so the
 * conditional `<PerspectiveSpatialZone>` lights up its
 * `<FocusZone moniker={asMoniker("ui:perspective")}>` branch. The Tauri
 * `invoke` boundary is mocked so we can inspect the
 * `spatial_register_zone` calls the zone makes on mount.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, act } from "@testing-library/react";
import type { PerspectiveDef } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Tauri API mocks — must be set before any module that imports them.
// ---------------------------------------------------------------------------

const mockInvoke = vi.fn((..._args: unknown[]) => Promise.resolve());

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    label: "main",
    listen: vi.fn(() => Promise.resolve(() => {})),
  }),
}));

// ---------------------------------------------------------------------------
// Mock perspective-context — control the active perspective from each test.
// ---------------------------------------------------------------------------

const mockUsePerspectives = vi.hoisted(() =>
  vi.fn(() => ({
    perspectives: [] as PerspectiveDef[],
    activePerspective: null as PerspectiveDef | null,
    setActivePerspectiveId: vi.fn(),
    refresh: vi.fn(),
  })),
);

vi.mock("@/lib/perspective-context", () => ({
  usePerspectives: () => mockUsePerspectives(),
}));

// Mock ui-state-context for transitive dependencies.
vi.mock("@/lib/ui-state-context", () => ({
  useUIState: () => ({ windows: {} }),
  UIStateProvider: ({ children }: { children: unknown }) => children,
}));

// `PerspectiveContainer` calls `useRefreshEntities`; stub it so the test
// does not need a live rust-engine provider tree.
vi.mock("@/components/rust-engine-container", () => ({
  useRefreshEntities: () => () => Promise.resolve({ entities: {} }),
}));

// `PerspectiveContainer` reads the active board path via `useActiveBoardPath`
// from `@/lib/command-scope`; stub it so no board context is required.
vi.mock("@/lib/command-scope", async () => {
  const actual = await vi.importActual<typeof import("@/lib/command-scope")>(
    "@/lib/command-scope",
  );
  return {
    ...actual,
    useActiveBoardPath: () => undefined,
  };
});

// Imports come after mocks
import { PerspectiveContainer } from "./perspective-container";
import { FocusLayer } from "./focus-layer";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { asLayerName } from "@/types/spatial";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Wait for register effects scheduled in `useEffect` to flush. */
async function flushSetup() {
  await act(async () => {
    await Promise.resolve();
  });
}

/** Render PerspectiveContainer wrapped in the production-shaped spatial-nav stack. */
function renderWithSpatialStack(children: React.ReactNode = null) {
  return render(
    <SpatialFocusProvider>
      <FocusLayer name={asLayerName("window")}>
        <EntityFocusProvider>
          <PerspectiveContainer>{children}</PerspectiveContainer>
        </EntityFocusProvider>
      </FocusLayer>
    </SpatialFocusProvider>,
  );
}

/** Collect every `spatial_register_zone` call in order. */
function registerZoneCalls(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_register_zone")
    .map((c) => c[1] as Record<string, unknown>);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("PerspectiveContainer (spatial-nav)", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockUsePerspectives.mockReturnValue({
      perspectives: [],
      activePerspective: null,
      setActivePerspectiveId: vi.fn(),
      refresh: vi.fn(),
    });
  });

  it("registers a ui:perspective zone when wrapped in SpatialFocusProvider + FocusLayer", async () => {
    const { unmount } = renderWithSpatialStack(
      <span data-testid="child">child</span>,
    );
    await flushSetup();

    const calls = registerZoneCalls();
    const perspectiveZone = calls.find((c) => c.moniker === "ui:perspective");
    expect(perspectiveZone).toBeTruthy();
    expect(perspectiveZone?.parentZone).toBeNull();
    expect(perspectiveZone?.layerKey).toBeTruthy();

    unmount();
  });

  it("emits a wrapper element with data-moniker='ui:perspective'", async () => {
    const { container, unmount } = renderWithSpatialStack(
      <span data-testid="child">child</span>,
    );
    await flushSetup();

    const node = container.querySelector("[data-moniker='ui:perspective']");
    expect(node).not.toBeNull();

    unmount();
  });

  it("preserves the flex chain className on the perspective zone wrapper", async () => {
    const { container, unmount } = renderWithSpatialStack(
      <span data-testid="child">child</span>,
    );
    await flushSetup();

    const node = container.querySelector(
      "[data-moniker='ui:perspective']",
    ) as HTMLElement;
    expect(node).not.toBeNull();
    // The zone wraps the view chain — the flex chain must stay intact.
    expect(node.className).toContain("flex");
    expect(node.className).toContain("flex-col");
    expect(node.className).toContain("flex-1");
    expect(node.className).toContain("min-h-0");
    expect(node.className).toContain("min-w-0");

    unmount();
  });

  it("renders children inside the ui:perspective zone wrapper", async () => {
    const { container, unmount } = renderWithSpatialStack(
      <span data-testid="child">child</span>,
    );
    await flushSetup();

    const zone = container.querySelector(
      "[data-moniker='ui:perspective']",
    ) as HTMLElement;
    expect(zone).not.toBeNull();
    expect(zone.querySelector('[data-testid="child"]')).not.toBeNull();

    unmount();
  });

  it("does not wrap in FocusZone when no SpatialFocusProvider is present", () => {
    // The narrow provider tree (used by the existing test suite) must keep
    // the zone wrapper out of the DOM so it doesn't disrupt layout assertions.
    const { container } = render(
      <EntityFocusProvider>
        <PerspectiveContainer>
          <span data-testid="child">child</span>
        </PerspectiveContainer>
      </EntityFocusProvider>,
    );
    expect(
      container.querySelector("[data-moniker='ui:perspective']"),
    ).toBeNull();
  });
});
