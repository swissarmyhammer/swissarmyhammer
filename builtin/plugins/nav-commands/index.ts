// nav-commands — builtin plugin owning the nine universal spatial-navigation
// commands (`nav.*`). It REPLACES the retired
// `crates/swissarmyhammer-focus/builtin/commands/nav.yaml` overlay (whose
// execution lived in React closures in `app-shell.tsx`) so the nav metadata
// reaches the OS menu THROUGH the CommandService catalogue, and nav execution
// is a real backend/plugin path — not a React closure, not a YAML merge.
//
// This mirrors the `file-commands` / `ui-commands` template:
//
//   1. A `Plugin` subclass carries `name` / `description` as descriptive class
//      props (plugin identity is the bundle directory name — `nav-commands`).
//   2. `load()` calls `ensureServices(this, ["commands", "focus"])` FIRST — so
//      the `commands` registry and the `focus` kernel are both live before any
//      registration — THEN `registerCommands`.
//   3. Each registration carries the FULL UI metadata from the source
//      `nav.yaml` — `keys` + `menu:{path:["Navigation"],group,order}` — 1:1, so
//      the OS menu renders identically to the YAML-driven version.
//   4. The plugin holds NO business logic. Each directional / drill command
//      makes exactly ONE host-driven MCP call into the `focus` kernel; the
//      kernel pulls the live geometry + focus from the webview on demand (Card
//      F2), so NO snapshot crosses the wire.
//
// Backend routing — eight of the nine nav.* commands route to the `focus`
// kernel (`crates/swissarmyhammer-focus/src/operations.rs`), host-driven:
//   nav.up/down/left/right/first/last → focus `navigate focus`
//     (this.focus.focus.focus.navigate) with `{ window, direction }`.
//   nav.drillIn                       → focus `drill_in layer`
//     (this.focus.focus.layer.drill_in) with `{ window, fq, focused_fq }`.
//   nav.drillOut                      → focus `drill_out layer`
//     (this.focus.focus.layer.drill_out) with `{ window, fq, focused_fq }`.
//
// The ninth — `nav.jump` — has NO backend op: the webview registers a handler
// on the command bus (`webview-command-bus.ts`, Card B) that opens the
// `<JumpToOverlay>`. This plugin owns its id / name / keys / menu so it appears
// in the OS menu and palette, but its EXECUTION is presentation-only and runs
// in the webview (`useDispatchCommand` consults the bus before the backend).
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
 *   `navigate focus`  → this.focus.focus.focus.navigate
 *   `query focus`     → this.focus.focus.focus.query
 *   `drill_in layer`  → this.focus.focus.layer.drill_in
 *   `drill_out layer` → this.focus.focus.layer.drill_out
 */
interface FocusDispatch {
  focus: {
    focus: {
      focus: {
        navigate(args: Record<string, unknown>): Promise<unknown>;
        query(args: Record<string, unknown>): Promise<unknown>;
      };
      layer: {
        drill_in(args: Record<string, unknown>): Promise<unknown>;
        drill_out(args: Record<string, unknown>): Promise<unknown>;
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
 * Registers the nine `nav.*` commands ported from `nav.yaml`. Eight route to
 * the `focus` kernel (host-driven, no snapshot); `nav.jump` is webview-bus
 * handled. Identity is the bundle directory name (`nav-commands`); `name` /
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
   * nine nav commands.
   */
  async load(): Promise<void> {
    await ensureServices(this, ["commands", "focus"]);

    const focus = this as unknown as FocusDispatch;

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
      // driven drill: pull the focused FQM for the window, then drill into it.
      // The kernel needs `fq` (the scope being drilled into) — which is the
      // focused scope — so we resolve focus first, then thread it as both `fq`
      // and `focused_fq` and let the kernel pull the snapshot for `window`.
      {
        id: "nav.drillIn",
        name: "Drill In",
        undoable: false,
        keys: { vim: "Enter", cua: "Enter", emacs: "Enter" },
        menu: { path: ["Navigation"], group: 2, order: 0 },
        execute: async (rawCtx: unknown) => {
          const ctx = (rawCtx ?? {}) as CommandContext;
          const window = scopeId(ctx, "window");
          const fq = await this.focusedFq(focus, window);
          if (fq === undefined) return { ok: true, next_fq: null };
          return await focus.focus.focus.layer.drill_in({
            window,
            fq,
            focused_fq: fq,
          });
        },
      },

      // ─── nav.drillOut ───────────────────────────────────────────────────
      // nav.yaml: keys vim/cua/emacs all Escape, menu Navigation/2/1. Same
      // host-driven shape as nav.drillIn.
      {
        id: "nav.drillOut",
        name: "Drill Out",
        undoable: false,
        keys: { vim: "Escape", cua: "Escape", emacs: "Escape" },
        menu: { path: ["Navigation"], group: 2, order: 1 },
        execute: async (rawCtx: unknown) => {
          const ctx = (rawCtx ?? {}) as CommandContext;
          const window = scopeId(ctx, "window");
          const fq = await this.focusedFq(focus, window);
          if (fq === undefined) return { ok: true, next_fq: null };
          return await focus.focus.focus.layer.drill_out({
            window,
            fq,
            focused_fq: fq,
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
    ]);

    this.log.info(
      "nav-commands: registered 9 nav.* (up/down/left/right/first/last → focus navigate; drillIn/drillOut → focus drill; jump → webview bus)",
    );
  }

  /**
   * Pull the FQM currently focused in `window` from the focus kernel.
   *
   * Drill needs the focused scope as its `fq`; the host-driven contract has
   * the kernel resolve focus from `focus_by_window`, but it does not echo that
   * FQM back into `fq`, so the plugin pulls it explicitly via `query focus`.
   * Returns `undefined` when the window is unknown or has no focused slot — the
   * caller then no-ops (nothing to drill from).
   */
  private async focusedFq(
    focus: FocusDispatch,
    window: string | undefined,
  ): Promise<string | undefined> {
    if (window === undefined) return undefined;
    // In-process op tools answer with a CallToolResult whose `content[0].text`
    // carries the op's JSON payload (`{ ok, focus }`); `unwrapResult` parses it.
    const result = await focus.focus.focus.focus.query({ window });
    const fq = unwrapResult<{ focus?: unknown }>(result).focus;
    return typeof fq === "string" ? fq : undefined;
  }
}
