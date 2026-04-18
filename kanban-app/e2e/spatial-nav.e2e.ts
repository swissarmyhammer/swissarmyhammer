/**
 * Canonical spatial-nav E2E scenario.
 *
 * Boots a real `kanban-app` debug binary against a deterministic 3x3 board
 * fixture and walks the full spatial-nav loop end-to-end — board layer
 * registration, cardinal navigation against real DOM rects, inspector layer
 * push, in-layer navigation, and layer pop with focus restoration.
 *
 * This is the test we point at to say "the thing actually works."
 *
 * Each step asserts **both** sides of the contract where possible:
 *
 *   - DOM state: the `data-focused` attribute on the FocusScope wrapper
 *     (rendered by `focus-highlight.tsx`) and the `data-moniker` attribute
 *     on the same wrapper.
 *   - Rust state: the `__spatial_dump` debug-only Tauri command returns
 *     a `SpatialDump` struct (see `src/spatial.rs`) with `focused_moniker`,
 *     `entry_count`, and `layer_stack`.
 *
 * Divergence between the two halves is the bug the harness is hunting —
 * the React tree might say `task-1-1` is focused while Rust thinks nothing
 * is, or Rust could report 9 registered entries while the DOM shows only 6.
 * A single-sided assertion would silently miss either pathology.
 *
 * Navigation is driven through **synthesised key events** (`browser.keys`)
 * rather than direct `invoke("spatial_navigate")` calls. This exercises
 * the full keybinding → dispatch → `spatial_navigate` chain so a regression
 * anywhere in that pipeline fails here. If key synthesis turns out to be
 * flaky on a specific platform, switch to `invoke` dispatch and file a
 * follow-up rather than silently loosening the assertions.
 */

import type { BoardFixture } from "./setup.js";

/**
 * Shape of the `SpatialDump` struct returned by the debug-only
 * `__spatial_dump` Tauri command. Must stay in lockstep with
 * `kanban-app/src/spatial.rs::SpatialDump`.
 */
interface SpatialDump {
  focused_key: string | null;
  focused_moniker: string | null;
  entry_count: number;
  layer_stack: Array<{
    key: string;
    name: string;
    last_focused: string | null;
    entry_count_in_layer: number;
  }>;
}

/**
 * Minimal typing for the Tauri v2 IPC bridge exposed on `window` inside the
 * webview. We reach for the internals directly (rather than importing
 * `@tauri-apps/api/core`) so the snippet can be serialised by WebdriverIO
 * `executeAsync` without a module-resolution dance inside the driver's
 * evaluation context.
 */
interface TauriInternals {
  invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
}

/**
 * Invoke `__spatial_dump` from inside the webview context via
 * `browser.executeAsync`. The Tauri bridge (`window.__TAURI_INTERNALS__`)
 * is only reachable from the webview page, so we hop through the driver's
 * `executeAsync` to land on the right side of the IPC boundary.
 *
 * The snippet is serialised by WebdriverIO and evaluated inside the
 * webview, so it must be self-contained — no imports, no closures over
 * outer-scope variables. It calls the internals bridge directly rather
 * than importing `@tauri-apps/api` to avoid a module-resolution dance in
 * the driver's eval environment.
 */
async function spatialDump(): Promise<SpatialDump> {
  return await browser.executeAsync<SpatialDump, []>(function (done) {
    const tauri = (window as unknown as { __TAURI_INTERNALS__: TauriInternals })
      .__TAURI_INTERNALS__;
    tauri
      .invoke("__spatial_dump")
      .then((dump) => done(dump as SpatialDump))
      .catch((err) => {
        // Propagate as a rejected SpatialDump-shaped value so the
        // outer assertion fails with a readable error.
        done({
          focused_key: null,
          focused_moniker: String(err),
          entry_count: -1,
          layer_stack: [],
        });
      });
  });
}

/** The chainable element returned by `$()` — the async proxy WebdriverIO hands back. */
type FocusScopeElement = ReturnType<WebdriverIO.Browser["$"]>;

/**
 * Locate the focus-scope wrapper element for a given moniker. The element
 * is rendered by `FocusScope` / `FocusHighlight`, which sets `data-moniker`
 * unconditionally and `data-focused` when focused.
 */
async function findScope(moniker: string): Promise<FocusScopeElement> {
  const el = $(`[data-moniker="${moniker}"]`);
  await el.waitForExist({ timeout: 10_000 });
  return el;
}

/**
 * True iff the scope for `moniker` is currently rendering `data-focused`.
 * `FocusHighlight` omits the attribute when `focused` is false, so the
 * attribute's absence (getAttribute → null) is the unfocused state.
 */
async function isFocusedInDom(moniker: string): Promise<boolean> {
  const el = await findScope(moniker);
  const attr = await el.getAttribute("data-focused");
  return attr === "true";
}

/**
 * Assert that the spatial state agrees with the DOM about which moniker is
 * focused. Called at the end of every nav step so a divergence surfaces
 * immediately rather than cascading into later assertions.
 */
async function assertFocus(moniker: string): Promise<void> {
  await browser.waitUntil(
    async () => {
      const dump = await spatialDump();
      const dom = await isFocusedInDom(moniker);
      return dump.focused_moniker === moniker && dom;
    },
    {
      timeout: 5_000,
      timeoutMsg: `focus never converged on ${moniker}`,
    },
  );
}

describe("spatial-nav canonical scenario", () => {
  it("board → inspector → back reaches every expected state", async () => {
    const fixture = globalThis.__fixture as BoardFixture | undefined;
    if (!fixture) throw new Error("beforeSession must populate global __fixture");

    // ------------------------------------------------------------------
    // Step 1 — Cold boot. Root "window" layer should be the only layer.
    // ------------------------------------------------------------------
    await browser.waitUntil(
      async () => (await spatialDump()).layer_stack.length >= 1,
      { timeout: 30_000, timeoutMsg: "root layer never pushed" },
    );
    const cold = await spatialDump();
    expect(cold.layer_stack).toHaveLength(1);
    expect(cold.layer_stack[0].name).toBe("window");

    // ------------------------------------------------------------------
    // Step 2 — Bulk registration. ResizeObserver must have fired on every
    // FocusScope, so Rust sees at least nine entries (one per task card).
    // The ">=9" lower bound allows for chrome scopes (column headers,
    // board container) that may also register — the assertion is about
    // "at least the task cards made it", not "exactly nine".
    // ------------------------------------------------------------------
    await browser.waitUntil(
      async () => (await spatialDump()).entry_count >= 9,
      {
        timeout: 10_000,
        timeoutMsg: "expected at least 9 registered spatial entries",
      },
    );
    // Also confirm DOM: all nine task monikers must be present.
    for (const taskId of fixture.tasks) {
      const el = await findScope(`task:${taskId}`);
      expect(await el.isExisting()).toBe(true);
    }

    // ------------------------------------------------------------------
    // Step 3 — Click to focus. Click the top-left card (task-1-1). DOM
    // must surface `data-focused="true"`; Rust must report the same
    // focused_moniker.
    // ------------------------------------------------------------------
    const topLeft = await findScope("task:task-1-1");
    await topLeft.click();
    await assertFocus("task:task-1-1");

    // ------------------------------------------------------------------
    // Step 4 — Cardinal nav via synthesised key events.
    // Layout (row-major): task-<col>-<row>
    //   col-1: task-1-1, task-1-2, task-1-3
    //   col-2: task-2-1, task-2-2, task-2-3
    //   col-3: task-3-1, task-3-2, task-3-3
    //
    // Each key event drives the full keybindings → dispatch →
    // spatial_navigate chain. `browser.keys` forwards through the driver
    // the same way a real keystroke would.
    // ------------------------------------------------------------------
    await browser.keys(["ArrowRight"]);
    await assertFocus("task:task-2-1");

    await browser.keys(["ArrowDown"]);
    await assertFocus("task:task-2-2");

    await browser.keys(["ArrowLeft"]);
    await assertFocus("task:task-1-2");

    // Left again at the left edge clamps — no wrap, no throw.
    await browser.keys(["ArrowLeft"]);
    await assertFocus("task:task-1-2");

    // Remember this moniker; step 7 asserts it is restored after the
    // inspector closes. Using the DOM-visible moniker (not the Rust
    // spatial key, which is a ULID regenerated per mount) makes the
    // restore assertion readable.
    const preInspectorFocus = "task:task-1-2";

    // ------------------------------------------------------------------
    // Step 5 — Layer capture on inspector open. Double-click fires
    // `ui.inspect`, which mounts `InspectorsContainer` wrapped in
    // `<FocusLayer name="inspector">`. That layer push must reach Rust.
    // ------------------------------------------------------------------
    const focused = await findScope(preInspectorFocus);
    await focused.doubleClick();

    await browser.waitUntil(
      async () => {
        const dump = await spatialDump();
        return (
          dump.layer_stack.length === 2 &&
          dump.layer_stack[1].name === "inspector"
        );
      },
      {
        timeout: 10_000,
        timeoutMsg: "inspector layer never pushed",
      },
    );

    // Inspector panel must be present in the DOM. `SlidePanel`
    // (`ui/src/components/slide-panel.tsx`) renders with
    // `role="dialog" aria-modal="true"` — both an a11y affordance and
    // the stable selector this harness pivots on.
    const inspectorPanel = await $('[role="dialog"]');
    await inspectorPanel.waitForExist({ timeout: 5_000 });
    expect(await inspectorPanel.isExisting()).toBe(true);

    // First inspector field must have focus. We don't hard-code which
    // moniker that is (the inspector's field order is metadata-driven and
    // therefore unstable across schema edits); instead we tie the focused
    // entry to the active inspector layer by two independent signals:
    //
    //   (a) the focused moniker uses the `field:` prefix emitted by
    //       `fieldMoniker(type, id, field)` in `ui/src/lib/moniker.ts` —
    //       only inspector field rows carry that prefix, so any residual
    //       focus on a background task card (`task:…`) would fail here.
    //   (b) the focused moniker is not one of the fixture's task cards —
    //       a belt-and-braces check in case a future moniker convention
    //       changes the `field:` prefix.
    //
    // Together with `layer_stack[1].entry_count_in_layer > 0` (the
    // inspector actually mounted FocusScopes), these three assertions
    // catch the regression the spec warns about: "inspector layer pushes
    // correctly but focus fails to land inside that layer".
    const afterOpen = await spatialDump();
    expect(afterOpen.focused_key).not.toBeNull();
    expect(afterOpen.focused_moniker).not.toBeNull();
    const inspectorLayerKey = afterOpen.layer_stack[1].key;
    expect(afterOpen.layer_stack[1].entry_count_in_layer).toBeGreaterThan(0);
    expect(afterOpen.focused_moniker?.startsWith("field:")).toBe(true);
    const cardMonikers = new Set(fixture.tasks.map((t) => `task:${t}`));
    expect(cardMonikers.has(afterOpen.focused_moniker ?? "")).toBe(false);

    // ------------------------------------------------------------------
    // Step 6 — Nav stays inside the inspector layer. Several Down
    // keystrokes must cycle among inspector fields, never escaping to a
    // card in the background.
    //
    // We assert that after each Down the focused_moniker does not match
    // any `task:task-X-Y` card from the fixture. Background cards remain
    // registered (their layer is still in the stack) — the navigator is
    // filtered by *active* layer, so the guarantee is "never reaches a
    // background card". Six presses is enough to overshoot any realistic
    // inspector field count and exercise the clamp path at least once.
    // ------------------------------------------------------------------
    for (let i = 0; i < 6; i += 1) {
      await browser.keys(["ArrowDown"]);
      const dump = await spatialDump();
      expect(cardMonikers.has(dump.focused_moniker ?? "")).toBe(false);
    }

    // Bottom of the layer — one extra Down must clamp. Capture the
    // current focus, press Down once more, and verify nothing moved.
    // This distinguishes "never leaked" (the loop above) from "actively
    // held at the last field" (what the navigator is supposed to do when
    // there is nowhere further to go inside the active layer).
    const beforeClamp = await spatialDump();
    await browser.keys(["ArrowDown"]);
    const afterClamp = await spatialDump();
    expect(afterClamp.focused_moniker).toBe(beforeClamp.focused_moniker);
    expect(afterClamp.focused_key).toBe(beforeClamp.focused_key);

    // ------------------------------------------------------------------
    // Step 7 — Layer pop via Escape restores focus via last_focused.
    //
    // Escape dispatches `app.dismiss`, which closes the inspector and
    // unmounts the `<FocusLayer name="inspector">`. The Rust
    // `remove_layer` call restores focus to layer 0's `last_focused`,
    // which — per the double-click path — was `task:task-1-2`.
    // ------------------------------------------------------------------
    await browser.keys(["Escape"]);

    await browser.waitUntil(
      async () => (await spatialDump()).layer_stack.length === 1,
      {
        timeout: 10_000,
        timeoutMsg: "inspector layer never removed",
      },
    );

    // Inspector DOM must be gone.
    const dismissedInspector = await $('[role="dialog"]');
    expect(await dismissedInspector.isExisting()).toBe(false);

    // And focus must be back where it was before the inspector opened.
    await assertFocus(preInspectorFocus);

    // Bookkeeping sanity: the inspector layer's key must not reappear in
    // the surviving stack.
    const final = await spatialDump();
    expect(
      final.layer_stack.some((l) => l.key === inspectorLayerKey),
    ).toBe(false);
  });
});
