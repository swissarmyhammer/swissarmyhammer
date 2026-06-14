/**
 * Unit tests for `viewIcon` — the dumb metadata-driven lookup behind the
 * left-nav view buttons.
 *
 * Regression context ("all views show the board icon"): every non-board
 * view rendered lucide's `LayoutGrid` (the documented fallback) because the
 * views the frontend received carried no `icon`. The contract pinned here:
 * the icon comes ONLY from the view's metadata-declared `icon` property —
 * the view `kind` is a renderer hint, never an icon name — and an
 * unresolvable/missing icon yields `null` so the caller applies the single
 * documented fallback.
 *
 * Node-only because it needs no browser APIs; lives under the
 * `*.node.test.ts` suffix recognized by `vite.config.ts`.
 */
import { describe, it, expect } from "vitest";
import { icons } from "lucide-react";
import { viewIcon } from "./view-icon";
import type { ViewDef } from "@/types/kanban";

/** Build a minimal ViewDef with the given icon/kind. */
function view(icon: string | undefined, kind: string): ViewDef {
  return { id: "01TESTVIEW", name: "Test", icon, kind };
}

describe("viewIcon", () => {
  it("resolves a declared lucide icon name", () => {
    expect(viewIcon(view("table", "grid"))).toBe(icons.Table);
  });

  it("resolves the builtin board view's 'kanban' icon", () => {
    expect(viewIcon(view("kanban", "board"))).toBe(icons.Kanban);
  });

  it("resolves kebab-case icon names", () => {
    expect(viewIcon(view("arrow-up-down", "grid"))).toBe(icons.ArrowUpDown);
  });

  it("returns null for an unknown icon name", () => {
    expect(viewIcon(view("no-such-icon", "grid"))).toBeNull();
  });

  it("returns null when the view declares no icon", () => {
    // "unknown" is the serde fallthrough kind of a degenerate view file —
    // it must not accidentally resolve to anything.
    expect(viewIcon(view(undefined, "unknown"))).toBeNull();
  });

  it("never borrows the view kind as an icon name", () => {
    // Regression: the old lookup fell back to `view.kind`, relying on kind
    // strings accidentally matching lucide names. "list" IS a real lucide
    // icon, so a kind-fallback would resolve it — the metadata contract
    // says an icon-less view gets the caller's documented fallback instead.
    expect(viewIcon(view(undefined, "list"))).toBeNull();
  });
});
