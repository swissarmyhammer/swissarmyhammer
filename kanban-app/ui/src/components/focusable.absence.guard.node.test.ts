/**
 * Absence guard for the deprecated `<Focusable>` re-export shim.
 *
 * Background: the spatial-nav kernel originally exposed four React peers
 * — `<Focusable>` (leaf), `<FocusZone>` (container), `<FocusLayer>`
 * (modal boundary), and a composite `<FocusScope>`. Card
 * `01KQ5PP55SAAVJ0V3HDJ1DGNBY` collapsed that into three peers
 * (`<FocusScope>` became the leaf primitive; `<Focusable>` aliased to
 * it as a transitional re-export). Card `01KQ5PSMYE3Q60SV8270S6K819`
 * physically deleted the shim once every per-component card had
 * migrated its imports.
 *
 * Parent task `01KQSDP4ZJY5ERAJ68TFPVFRRE` then folded `<FocusZone>`
 * into `<FocusScope>` as well — leaving exactly two React peers:
 * `<FocusLayer>` (modal boundary) and `<FocusScope>` (the unified
 * spatial primitive — leaf when childless, container when not).
 *
 * This guard pins the deletion against a future regression where
 * someone re-adds `kanban-app/ui/src/components/focusable.tsx` (or a
 * test re-creates it). The canonical primitive name is `<FocusScope>`;
 * resurrecting the alias would dilute that single source of truth and
 * reintroduce the multi-name confusion the architecture-fix card
 * eliminated.
 *
 * Node-only because it reads the file system from disk; lives under
 * the `*.node.test.ts` suffix recognised by `vite.config.ts`.
 */
import { describe, it, expect } from "vitest";
import { existsSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));

describe("<Focusable> deprecated re-export shim", () => {
  it("focusable.tsx does not exist (deleted; use <FocusScope> directly)", () => {
    const path = resolve(__dirname, "focusable.tsx");
    expect(existsSync(path)).toBe(false);
  });

  it("focusable.test.tsx does not exist (deleted with the shim)", () => {
    const path = resolve(__dirname, "focusable.test.tsx");
    expect(existsSync(path)).toBe(false);
  });
});
