// nav-commands — builtin plugin owning the universal spatial-navigation
// commands (`nav.*`). It REPLACES the retired
// `crates/swissarmyhammer-focus/builtin/commands/nav.yaml` overlay (whose
// execution lived in React closures in `app-shell.tsx`) so the nav metadata
// reaches the OS menu THROUGH the CommandService catalogue, and nav execution
// is a real backend/plugin path — not a React closure, not a YAML merge.
//
// This mirrors the `file-commands` / `app-shell-commands` template:
//
//   1. A `Plugin` subclass carries `name` / `description` as descriptive class
//      props (plugin identity is the bundle directory name — `nav-commands`).
//   2. `load()` calls `ensureServices(this, ["commands", "focus"])` FIRST — so
//      the `commands` registry and the `focus` kernel are both live before any
//      registration — THEN `registerCommands`.
//   3. Each nav.yaml-ported registration carries the FULL UI metadata from the
//      source `nav.yaml` — `keys` + `menu:{path:["Navigation"],group,order}` —
//      1:1, so the OS menu renders identically to the YAML-driven version.
//      `nav.focus` (never in nav.yaml) carries neither: no keys, no menu.
//   4. The plugin holds NO business logic. Each directional / drill command
//      makes exactly ONE host-driven MCP call into the `focus` kernel; the
//      kernel pulls the live geometry + focus from the webview on demand (Card
//      F2), so NO snapshot crosses the wire.
//
// Backend routing — eight of the nine nav.yaml-ported commands route to the
// `focus` kernel (`crates/swissarmyhammer-focus/src/operations.rs`), host-driven:
//   nav.up/down/left/right/first/last → focus `navigate focus`
//     (this.focus.focus.focus.navigate) with `{ window, direction }`.
//   nav.drillIn                       → focus `drill_in layer`
//     (this.focus.focus.layer.drill_in) with `{ window }` — the kernel resolves
//     the focused scope (provider focus → kernel-slot fallback, like navigate).
//   nav.drillOut                      → focus `drill_out layer`
//     (this.focus.focus.layer.drill_out) with `{ window }`; same source
//     resolution, with a `moved` flag driving the dismiss fall-through.
//
// The ninth — `nav.jump` — has NO backend op: the webview registers a handler
// on the command bus (`webview-command-bus.ts`, Card B) that opens the
// `<JumpToOverlay>`. This plugin owns its id / name / keys / menu so it appears
// in the OS menu and palette, but its EXECUTION is presentation-only and runs
// in the webview (`useDispatchCommand` consults the bus before the backend).
//
// The tenth — `nav.focus` — is the programmatic focus-claim command (it was
// never in nav.yaml: no keys, no menu, not palette-visible). It routes to
//   focus `set focus` (this.focus.focus.focus.set) with `{ fq: args.fq, window }`
// — the same wire shape `focus-mcp.ts::setFocus` uses, minus the snapshot
// (the host has no geometry; a snapshot-less commit drops silently per the
// kernel's transient-unmount contract). In the webview the single execution
// leg is the command-bus handler `SpatialFocusProvider` registers
// (`registerWebviewCommandHandler("nav.focus", …)` in
// `spatial-focus-context.tsx`); `useDispatchCommand` consults the bus before
// the backend, and the handler runs `actions.focus(fq)` — which composes the
// live geometry snapshot via `buildSnapshotForFocused` and commits the
// snapshot-bearing `set focus` through `focus-mcp.ts::setFocus`. (Card G
// deleted the two legacy `nav.focus` scope defs the contexts used to carry.)
// This plugin def makes the catalogue the single registration owner.
//
// # Resolving the window
//
// The focus host-driven ops take an explicit `window` (the MCP wire has no
// ambient window). The plugin derives it from the `window:<label>` moniker the
// frontend always injects into the dispatch scope chain — `scopeId(ctx,
// "window")`, the TypeScript mirror of
// `commands_core::context.rs::window_label_from_scope`. A dispatch with no
// `window:` moniker yields `undefined`; the focus op then drops silently
// (matching the kernel's window-unknown contract).

import {
  CommandContext,
  Plugin,
  ensureServices,
  registerCommands,
  scopeId,
  unwrapResult,
} from "@swissarmyhammer/plugin";

/**
 * The dispatch surface for the `focus` operation tool — the spatial-nav kernel
 * ops the directional / drill commands route to.
 *
 * The dispatch Proxy turns a property path into an MCP `tools/call`:
 * `server.tool.noun.verb`. For the `focus` server the server name and the
 * single tool name are both `"focus"`; the noun/verb pairs come straight from
 * `crates/swissarmyhammer-focus/src/operations.rs`:
 *   `set focus`       → this.focus.focus.focus.set
 *   `navigate focus`  → this.focus.focus.focus.navigate
 *   `drill_in layer`  → this.focus.focus.layer.drill_in
 *   `drill_out layer` → this.focus.focus.layer.drill_out
 */
interface FocusDispatch {
  focus: {
    focus: {
      focus: {
        set(args: Record<string, unknown>): Promise<unknown>;
        navigate(args: Record<string, unknown>): Promise<unknown>;
      };
      layer: {
        drill_in(args: Record<string, unknown>): Promise<unknown>;
        drill_out(args: Record<string, unknown>): Promise<unknown>;
      };
    };
  };
}

/**
 * The dispatch surface for the `ui_state` server's `dismiss ui` op —
 * `this.ui_state.ui_state.ui.dismiss`, mirroring `app.dismiss` in
 * `builtin/plugins/app-shell-commands/commands/app.ts`. `nav.drillOut` falls
 * through to this when the kernel drill is a no-op (the focused scope has no
 * `parent_zone` — a layer-root edge — or there is no focus at all), closing
 * the topmost modal layer (palette → inspector). This preserves the
 * Escape-chain semantics the removed React `buildDrillCommands` had.
 */
interface UiStateDispatch {
  ui_state: {
    ui_state: {
      ui: {
        dismiss(args: Record<string, unknown>): Promise<unknown>;
      };
    };
  };
}

/** One directional nav command's identity + metadata + wire direction. */
interface NavDirSpec {
  id: string;
  name: string;
  keys: Record<string, string>;
  menu: { path: string[]; group: number; order: number };
  /** The `Direction` wire literal the `navigate focus` op accepts. */
  direction: "up" | "down" | "left" | "right" | "first" | "last";
}

/**
 * The six directional / first-last nav commands, as a data table.
 *
 * `keys` + `menu` are copied 1:1 from
 * `crates/swissarmyhammer-focus/builtin/commands/nav.yaml`; `direction` is the
 * lowercase `Direction` wire literal (`operations.rs::Navigate`). Holding the
 * variation as data keeps the six registrations a single `map`, not six
 * near-identical object literals.
 */
const NAV_DIRECTIONS: readonly NavDirSpec[] = [
  {
    id: "nav.up",
    name: "Navigate Up",
    keys: { vim: "k", cua: "ArrowUp", emacs: "Ctrl+p" },
    menu: { path: ["Navigation"], group: 0, order: 0 },
    direction: "up",
  },
  {
    id: "nav.down",
    name: "Navigate Down",
    keys: { vim: "j", cua: "ArrowDown", emacs: "Ctrl+n" },
    menu: { path: ["Navigation"], group: 0, order: 1 },
    direction: "down",
  },
  {
    id: "nav.left",
    name: "Navigate Left",
    keys: { vim: "h", cua: "ArrowLeft", emacs: "Ctrl+b" },
    menu: { path: ["Navigation"], group: 0, order: 2 },
    direction: "left",
  },
  {
    id: "nav.right",
    name: "Navigate Right",
    keys: { vim: "l", cua: "ArrowRight", emacs: "Ctrl+f" },
    menu: { path: ["Navigation"], group: 0, order: 3 },
    direction: "right",
  },
  {
    id: "nav.first",
    name: "Navigate to First",
    keys: { cua: "Home", emacs: "Alt+<" },
    menu: { path: ["Navigation"], group: 1, order: 0 },
    direction: "first",
  },
  {
    id: "nav.last",
    name: "Navigate to Last",
    keys: { vim: "Shift+G", cua: "End", emacs: "Alt+>" },
    menu: { path: ["Navigation"], group: 1, order: 1 },
    direction: "last",
  },
];

/**
 * The nav-commands builtin plugin.
 *
 * Registers the ten `nav.*` commands: the nine ported from `nav.yaml` (eight
 * route to the `focus` kernel host-driven, no snapshot; `nav.jump` is
 * webview-bus handled) plus the programmatic `nav.focus` (focus `set focus`).
 * Identity is the bundle directory name (`nav-commands`); `name` /
 * `description` are descriptive metadata only.
 */
export default class NavCommandsPlugin extends Plugin {
  /** Human-readable name — descriptive metadata only, not plugin identity. */
  readonly name = "Navigation Commands";

  /** One-line description — descriptive metadata only. */
  readonly description =
    "Builtin universal spatial-navigation commands (directional / first-last / drill / jump) routed to the focus kernel and the webview jump overlay.";

  /**
   * Activate the `commands` registry and the `focus` kernel, then register the
   * ten nav commands.
   */
  async load(): Promise<void> {
    await ensureServices(this, ["commands", "focus", "ui_state"]);

    const focus = this as unknown as FocusDispatch;
    const uiState = this as unknown as UiStateDispatch;

    const directional = NAV_DIRECTIONS.map((spec) => ({
      id: spec.id,
      name: spec.name,
      undoable: false,
      keys: spec.keys,
      menu: spec.menu,
      // Host-driven navigate: send only `{ window, direction }`. The kernel
      // resolves the current focus from `focus_by_window[window]` and PULLS
      // the live geometry from the webview (Card F2) — no snapshot, no
      // focused_fq on the wire. A dispatch with no resolvable window drops
      // silently (the op no-ops on an undefined window).
      execute: async (rawCtx: unknown) => {
        const ctx = (rawCtx ?? {}) as CommandContext;
        const window = scopeId(ctx, "window");
        return await focus.focus.focus.focus.navigate({
          window,
          direction: spec.direction,
        });
      },
    }));

    await registerCommands(this, [
      ...directional,

      // ─── nav.drillIn ────────────────────────────────────────────────────
      // nav.yaml: keys vim/cua/emacs all Enter, menu Navigation/2/0. Host-
      // driven drill: send ONLY `{ window }` and let the kernel resolve the
      // current focus the SAME way `navigate focus` does — provider focus, then
      // the kernel `focus_by_window` slot fallback. The focused scope IS the
      // scope being drilled into, so the kernel uses the resolved focus as both
      // the drill target and the no-op echo; the plugin no longer pre-resolves
      // focus client-side (the old `query focus` → `provider.focus` ONLY path
      // had no kernel-slot fallback, so it silently no-op'd whenever the UI
      // reported no focus while the kernel slot was set — drilling broke while
      // navigate kept working).
      {
        id: "nav.drillIn",
        name: "Drill In",
        undoable: false,
        keys: { vim: "Enter", cua: "Enter", emacs: "Enter" },
        menu: { path: ["Navigation"], group: 2, order: 0 },
        execute: async (rawCtx: unknown) => {
          const ctx = (rawCtx ?? {}) as CommandContext;
          const window = scopeId(ctx, "window");
          return await focus.focus.focus.layer.drill_in({ window });
        },
      },

      // ─── nav.drillOut ───────────────────────────────────────────────────
      // nav.yaml: keys vim/cua/emacs all Escape, menu Navigation/2/1.
      // Two-stage, porting the removed React `buildDrillCommands` contract:
      //   1. Drill out host-driven: send ONLY `{ window }` and let the kernel
      //      resolve the current focus (provider focus → kernel-slot fallback,
      //      symmetric with navigate) and drill out of it. If the focused scope
      //      has a `parent_zone`, the kernel commits focus to it, emits
      //      `focus-changed` (the UI moves), and reports `moved: true`.
      //   2. DISMISS fallthrough: if the kernel did NOT move focus
      //      (`moved: false` — no resolvable focus, or a layer-root edge with no
      //      `parent_zone`), there is nothing to drill out TO, so close the
      //      topmost modal layer (palette → inspector) via `ui_state`
      //      `dismiss ui` — the same Escape-chain behavior the old React closure
      //      had. The `moved` flag replaces the old client-side
      //      `next_fq === focusedFq` echo check, which required pre-resolving
      //      focus (the source of the drill regression).
      {
        id: "nav.drillOut",
        name: "Drill Out",
        undoable: false,
        keys: { vim: "Escape", cua: "Escape", emacs: "Escape" },
        menu: { path: ["Navigation"], group: 2, order: 1 },
        execute: async (rawCtx: unknown) => {
          const ctx = (rawCtx ?? {}) as CommandContext;
          const window = scopeId(ctx, "window");
          const scopeChain = ctx.scope_chain ?? [];
          const result = await focus.focus.focus.layer.drill_out({ window });
          const moved = unwrapResult<{ moved?: unknown }>(result).moved;
          // Focus moved to a parent zone — the kernel already committed focus +
          // emitted `focus-changed`; just surface the result.
          if (moved === true) {
            return result;
          }
          // No move (no resolvable focus, or a layer-root edge) → fall through
          // to dismiss the topmost modal layer, honouring the Escape chain.
          return await uiState.ui_state.ui_state.ui.dismiss({
            scope_chain: scopeChain,
          });
        },
      },

      // ─── nav.jump ───────────────────────────────────────────────────────
      // nav.yaml: keys vim:s / cua:Mod+G / emacs:Mod+G, menu Navigation/3/0.
      // NO backend op: this command's effect is presentation-only — open the
      // `<JumpToOverlay>` (AceJump-style scope picker). The webview registers a
      // handler on the command bus (Card B) keyed by this id; `useDispatchCommand`
      // runs that handler and skips the backend. The plugin owns the metadata
      // (id / name / keys / menu) so the command appears in the OS menu and
      // palette; it carries NO `execute` here because the effect lives in the
      // webview, not the host.
      {
        id: "nav.jump",
        name: "Jump To",
        undoable: false,
        keys: { vim: "s", cua: "Mod+G", emacs: "Mod+G" },
        menu: { path: ["Navigation"], group: 3, order: 0 },
        // Presentation-only: the webview bus handler (Card B) intercepts this
        // id in `useDispatchCommand` before the backend, so this host `execute`
        // is never reached in production. It exists as an inert no-op only to
        // satisfy the registration contract (every command carries an
        // `execute`) and to keep a direct host-side dispatch — e.g. in the
        // plugin e2e where no webview is mounted — a harmless success.
        execute: async () => {
          return { ok: true };
        },
      },

      // ─── nav.focus ──────────────────────────────────────────────────────
      // The programmatic focus-claim command — never in nav.yaml, so it
      // carries NO keys and NO menu placement (the Navigation submenu stays
      // at the nine nav.yaml entries), and it is not palette-visible (it
      // requires a target `args.fq`, like the programmatic `app.setFocus`).
      // Routes to the focus kernel's `set focus` op with `{ fq, window }` —
      // the wire shape `focus-mcp.ts::setFocus` uses, minus the snapshot:
      // the host has no geometry of its own, and the kernel drops a
      // snapshot-less commit silently (its transient-unmount contract). In
      // production the webview intercepts this id before the backend:
      // `SpatialFocusProvider` (`spatial-focus-context.tsx`) registers the
      // webview-bus handler (`registerWebviewCommandHandler("nav.focus", …)`)
      // that runs `actions.focus(fq)`, composing the live geometry snapshot
      // via `buildSnapshotForFocused` and committing the snapshot-bearing
      // `set focus` (Card G deleted the two legacy scope defs in
      // `entity-focus-context.tsx` / `spatial-focus-context.tsx`) — this
      // plugin def owns the catalogue registration.
      {
        id: "nav.focus",
        name: "Focus Scope",
        visible: false,
        undoable: false,
        execute: async (rawCtx: unknown) => {
          const ctx = (rawCtx ?? {}) as CommandContext;
          const window = scopeId(ctx, "window");
          return await focus.focus.focus.focus.set({
            fq: ctx.args?.fq,
            window,
          });
        },
      },
    ]);

    this.log.info(
      "nav-commands: registered 10 nav.* (up/down/left/right/first/last → focus navigate; drillIn/drillOut → focus drill; jump → webview bus; focus → focus set)",
    );
  }
}
