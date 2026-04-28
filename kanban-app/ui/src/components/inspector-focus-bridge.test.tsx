import { describe, it, expect, vi } from "vitest";
import { render, act } from "@testing-library/react";

const TASK_SCHEMA = {
  entity: {
    name: "task",
    body_field: "body",
    fields: ["title", "body"],
  },
  fields: [
    {
      id: "f1",
      name: "title",
      type: { kind: "markdown", single_line: true },
      editor: "markdown",
      display: "text",
      section: "header",
    },
    {
      id: "f2",
      name: "body",
      type: { kind: "markdown", single_line: false },
      editor: "markdown",
      display: "markdown",
      icon: "file-text",
      section: "body",
    },
  ],
};

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const mockInvoke = vi.fn((...args: any[]) => {
  if (args[0] === "list_entity_types") return Promise.resolve(["task"]);
  if (args[0] === "get_entity_schema") return Promise.resolve(TASK_SCHEMA);
  if (args[0] === "get_ui_state")
    return Promise.resolve({
      palette_open: false,
      palette_mode: "command",
      keymap_mode: "cua",
      scope_chain: [],
      open_boards: [],
      windows: {},
      recent_boards: [],
    });
  if (args[0] === "update_entity_field")
    return Promise.resolve({ id: "test-id" });
  return Promise.resolve("ok");
});

// Preserve real exports (SERIALIZE_TO_IPC_FN, Resource, Channel, TauriEvent,
// …) so transitively-imported submodules like `window.js` / `dpi.js` can
// still resolve their re-exports. Only override `invoke` / `listen`.
vi.mock("@tauri-apps/api/core", async () => {
  const actual = await vi.importActual<typeof import("@tauri-apps/api/core")>(
    "@tauri-apps/api/core",
  );
  return {
    ...actual,
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    invoke: (...args: any[]) => mockInvoke(...args),
  };
});
vi.mock("@tauri-apps/api/event", async () => {
  const actual = await vi.importActual<typeof import("@tauri-apps/api/event")>(
    "@tauri-apps/api/event",
  );
  return {
    ...actual,
    listen: vi.fn(() => Promise.resolve(() => {})),
  };
});
// `window-container.tsx` calls `getCurrentWindow()` at module-load time.
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    label: "main",
    listen: vi.fn(() => Promise.resolve(() => {})),
  }),
}));
vi.mock("@tauri-apps/plugin-log", () => ({
  error: vi.fn(),
  warn: vi.fn(),
  info: vi.fn(),
  debug: vi.fn(),
  trace: vi.fn(),
  attachConsole: vi.fn(() => Promise.resolve()),
}));

import "@/components/fields/registrations";
import { InspectorFocusBridge } from "./inspector-focus-bridge";
import { UIStateProvider } from "@/lib/ui-state-context";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import {
  EntityFocusProvider,
  useEntityFocus,
} from "@/lib/entity-focus-context";

import { FieldUpdateProvider } from "@/lib/field-update-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import type { Entity } from "@/types/kanban";
import { useState } from "react";

function makeEntity(fields: Record<string, unknown> = {}): Entity {
  return {
    entity_type: "task",
    id: "test-id",
    moniker: "task:test-id",
    fields,
  };
}

/** Reads focusedMoniker and renders it as text for test assertions. */
function FocusMonitorDisplay() {
  const { focusedMoniker } = useEntityFocus();
  return <span data-testid="focus-monitor">{focusedMoniker ?? "null"}</span>;
}

/** Wrapper that can toggle showing/hiding the bridge to test unmount. */
function ToggleableBridge({
  entity,
  initialShow = true,
}: {
  entity: Entity;
  initialShow?: boolean;
}) {
  const [show, setShow] = useState(initialShow);
  return (
    <>
      {show && <InspectorFocusBridge entity={entity} />}
      <FocusMonitorDisplay />
      <button data-testid="toggle" onClick={() => setShow((s) => !s)} />
    </>
  );
}

function Providers({ children }: { children: React.ReactNode }) {
  return (
    <TooltipProvider>
      <SchemaProvider>
        <EntityStoreProvider
          entities={{ task: [makeEntity({ title: "T", body: "B" })] }}
        >
          <EntityFocusProvider>
            <FieldUpdateProvider>
              <UIStateProvider>{children}</UIStateProvider>
            </FieldUpdateProvider>
          </EntityFocusProvider>
        </EntityStoreProvider>
      </SchemaProvider>
    </TooltipProvider>
  );
}

async function renderBridge(entity: Entity) {
  const result = render(
    <Providers>
      <InspectorFocusBridge entity={entity} />
    </Providers>,
  );
  await act(async () => {
    await new Promise((r) => setTimeout(r, 50));
  });
  return result;
}

describe("InspectorFocusBridge", () => {
  it("renders EntityInspector inside a command scope", async () => {
    const { container } = await renderBridge(
      makeEntity({ title: "T", body: "B" }),
    );
    expect(
      container.querySelector('[data-testid="entity-inspector"]'),
    ).toBeTruthy();
  });

  it("first field is focused on mount", async () => {
    const { container } = await renderBridge(
      makeEntity({ title: "T", body: "B" }),
    );
    // After card `01KQ5QB6F4MTD35GBTARJH4JEW` the row's outer `<div>` is
    // plain — the focus-bearing element is the Field's `<FocusZone>`
    // (a descendant of the row).
    const titleRow = container.querySelector('[data-testid="field-row-title"]');
    const titleFocusZone = titleRow?.querySelector(
      "[data-moniker='field:task:test-id.title']",
    );
    expect(titleFocusZone?.hasAttribute("data-focused")).toBe(true);
    const bodyRow = container.querySelector('[data-testid="field-row-body"]');
    const bodyFocusZone = bodyRow?.querySelector(
      "[data-moniker='field:task:test-id.body']",
    );
    expect(bodyFocusZone?.hasAttribute("data-focused")).toBe(false);
  });

  it("claims entity focus on mount", async () => {
    const { getByTestId } = render(
      <Providers>
        <ToggleableBridge
          entity={makeEntity({ title: "T", body: "B" })}
          initialShow={true}
        />
      </Providers>,
    );
    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });
    // EntityInspector's mount effect sets focus to the first field moniker
    expect(getByTestId("focus-monitor").textContent).toBe(
      "field:task:test-id.title",
    );
  });

  it("restores previous focus on unmount", async () => {
    const { getByTestId } = render(
      <Providers>
        <ToggleableBridge
          entity={makeEntity({ title: "T", body: "B" })}
          initialShow={true}
        />
      </Providers>,
    );
    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });
    // Inspector is focused — mount effect set focus to the first field
    expect(getByTestId("focus-monitor").textContent).toBe(
      "field:task:test-id.title",
    );

    // Close the inspector
    await act(async () => {
      getByTestId("toggle").click();
      await new Promise((r) => setTimeout(r, 50));
    });
    // Focus restored to null (nothing was focused before)
    expect(getByTestId("focus-monitor").textContent).toBe("null");
  });

  it("renders all navigable fields", async () => {
    const { container } = await renderBridge(
      makeEntity({ title: "T", body: "B" }),
    );
    expect(
      container.querySelector('[data-testid="field-row-title"]'),
    ).toBeTruthy();
    expect(
      container.querySelector('[data-testid="field-row-body"]'),
    ).toBeTruthy();
  });

  it("renders entity FocusScope with the entity moniker", async () => {
    const { container } = await renderBridge(
      makeEntity({ title: "T", body: "B" }),
    );
    // FocusScope adds data-moniker attribute; the inspector's entity scope should be present
    expect(
      container.querySelector('[data-moniker="task:test-id"]'),
    ).toBeTruthy();
  });
});
