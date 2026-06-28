// App sub-domain — ports the nine commands from `app.yaml`. Despite the shared
// `app.*` id prefix these fan out to THREE backends by concern:
//   - app.quit / app.about / app.help            → `app` server (OS chrome)
//   - app.undo / app.redo                         → `store` server (undo stack)
//   - app.command / app.palette / app.search /
//     app.dismiss                                 → `ui_state` server (UI toggles)
// Each registration carries `app.yaml`'s metadata (keys / menu / visible /
// undoable) 1:1 and makes exactly one MCP call into its backend.
//
// # Key canonicalization (Card I sweep)
//
// Since Card I deleted `app-shell.tsx`'s static scope defs, this registry
// metadata is the ONLY key source for the webview hotkey path:
// `extractKeymapBindings` matches the declared string LITERALLY against
// `normalizeKeyEvent` output, which emits lowercase letters for unshifted
// chords (`Mod+f`, not `Mod+F`). So the affected unshifted-letter keys are
// declared in the canonical lowercase form `BINDING_TABLES`
// (`apps/kanban-app/ui/src/lib/keybindings.ts`) encodes:
//   - app.undo  cua `Mod+z`  (BINDING_TABLES.cua agrees)
//   - app.redo  vim `Mod+r`  (BINDING_TABLES.vim; non-Mac `Ctrl+r` normalizes
//                             to `Mod+r`, and `Ctrl+R` never appeared in the
//                             table on Mac where Ctrl stays distinct)
//   - app.search cua `Mod+f` (BINDING_TABLES.cua agrees). The emacs key is
//                             DROPPED, not lowercased: `BINDING_TABLES.emacs`
//                             binds `Mod+f` to `nav.right` (emacs forward-char),
//                             so claiming `Mod+f` for Find would steal it and
//                             re-open the first-id-wins nondeterminism (cards
//                             01KMT56FTBAP8PQ4QQND08MP97 /
//                             01KTQ6QZNB3VN4MAND7VPASM21). Emacs Find stays
//                             palette-only.
//   - app.quit  cua `Mod+q`  (no BINDING_TABLES entry — quit rides the native
//                             menu accelerator, which parses letters
//                             case-insensitively; lowercasing keeps the
//                             accelerator AND makes the chord live in the
//                             webview on non-Mac, the `file.closeBoard`
//                             precedent).
// The `app-shell-plugin-commands-mirror.spatial.node.test.ts` drift guard pins
// these against `BINDING_TABLES` (with an explicit allowlist for the
// menu-accelerator-only `app.quit`).

import { bindCommandRun } from "@swissarmyhammer/plugin";

import {
  type AppDispatch,
  type CommandContext,
  type CommandSpec,
  type StoreDispatch,
  type UiStateDispatch,
} from "./context.ts";

/** The three dispatch surfaces an `app.*` command's `run` routes through. */
interface AppDispatchBundle {
  app: AppDispatch;
  store: StoreDispatch;
  uiState: UiStateDispatch;
}

/** Which dispatch surface (and MCP server/tool) an `app.*` command routes to.
 * The value is the {@link AppDispatchBundle} key; each surface's MCP tool name
 * is derived from it by {@link DISPATCH_TOOL}. */
type AppDispatchKind = "app" | "store" | "uiState";

/** The MCP server/tool segment each dispatch surface's path uses: the dispatch
 * Proxy turns `bundle.<kind>.<server>.<tool>.<service>.<verb>` into a
 * `tools/call`, where the server name and its single tool name are identical
 * (`app` / `store` / `ui_state`), so both the server and tool proxy hops use
 * this one value. `app` / `store` match their bundle key; `ui_state` is the
 * snake_case form of `uiState`. */
const DISPATCH_TOOL: Record<AppDispatchKind, string> = {
  app: "app",
  store: "store",
  uiState: "ui_state",
};

/** One `app.*` registration: the static metadata literals (`id` / `name` /
 * `keys` / `menu` / `visible` / `undoable`) the catalogue and the keymap drift
 * guard read, plus the routing data (`dispatch` / `service` / `verb` /
 * `passScope`) that — through {@link appRun} — drives the single backend MCP
 * call. The nine commands differ ONLY in this metadata + routing data, so `run`
 * is derived (not hand-written per row) to keep them from drifting out of
 * lockstep. */
interface AppCommandSpec {
  id: string;
  name: string;
  keys?: Record<string, string>;
  menu?: Record<string, unknown>;
  visible?: boolean;
  undoable?: boolean;
  /** Which backend surface the command routes to. */
  dispatch: AppDispatchKind;
  /** The operation noun under the surface's tool (e.g. `about`, `stack`,
   * `search`) — the third proxy hop. */
  service: string;
  /** The operation verb on the noun (e.g. `show`, `undo`, `dismiss`) — the
   * fourth proxy hop. */
  verb: string;
  /** Whether the command threads the active `scope_chain` into the call (the
   * per-window `ui_state` ops resolve their target window from it; the `app` /
   * `store` ops take no scope). Absent ⇒ no scope argument. */
  passScope?: boolean;
}

/**
 * Build the `run` for an `AppCommandSpec` from its routing data: one code path
 * dispatching `bundle[dispatch][server][tool][service][verb](...)` (server and
 * tool are the same {@link DISPATCH_TOOL} value), threading the active
 * `scope_chain` only when `passScope` is set. This collapses what were nine
 * parallel `run` closures (differing solely by which surface, noun, verb, and
 * whether they pass a scope) into a single interpreter of the table's routing
 * fields.
 */
function appRun(
  spec: AppCommandSpec,
): (ctx: CommandContext, dispatch: AppDispatchBundle) => Promise<unknown> {
  const segment = DISPATCH_TOOL[spec.dispatch];
  return (ctx, dispatch) => {
    // Walk the dispatch Proxy: bundle key → server → tool → noun → verb. Each
    // hop is an index into the next Proxy level; the final index yields the
    // callable that turns into the MCP `tools/call`. The server and tool hops
    // share the one `segment` value (the server names its single tool after
    // itself).
    const surface = dispatch[spec.dispatch] as unknown as Record<
      string,
      Record<
        string,
        Record<string, Record<string, (args: Record<string, unknown>) => Promise<unknown>>>
      >
    >;
    const call = surface[segment][segment][spec.service][spec.verb];
    return call(spec.passScope ? { scope_chain: ctx.scope_chain ?? [] } : {});
  };
}

/**
 * The nine `app.*` commands, as a module-level data table.
 *
 * `id` / `name` / `keys` / `menu` / `visible` / `undoable` are the static
 * `app.yaml` metadata 1:1 — held as literals at module scope so the keymap
 * drift guard (`app-shell-plugin-commands-mirror.spatial.node.test.ts`) can
 * parse them from source (the `AI_COMMANDS` / `BOARD_COMMANDS` pattern). The
 * backend call is expressed as DATA (`dispatch` / `service` / `verb` /
 * `passScope`) interpreted by the single {@link appRun} code path, which
 * `appCommands` binds to an `execute` over the live dispatch surfaces.
 */
const APP_COMMANDS: readonly AppCommandSpec[] = [
  // ─── app.about ──────────────────────────────────────────────────────────
  // YAML: menu {path:[App], group:0, order:0}; no keys. Routes to app
  // `show about` (app.app.app.about.show).
  {
    id: "app.about",
    name: "About",
    menu: { path: ["App"], group: 0, order: 0 },
    dispatch: "app",
    service: "about",
    verb: "show",
  },

  // ─── app.help ───────────────────────────────────────────────────────────
  // YAML: keys vim:F1 / cua:F1. Routes to app `show help`
  // (app.app.app.help.show).
  {
    id: "app.help",
    name: "Help",
    keys: { vim: "F1", cua: "F1" },
    dispatch: "app",
    service: "help",
    verb: "show",
  },

  // ─── app.quit ───────────────────────────────────────────────────────────
  // YAML: keys cua:Mod+q (canonicalized lowercase — menu-accelerator-only, see
  // the file header) / vim:":q", menu {path:[App], group:2, order:0}. Routes
  // to app `quit app` (app.app.app.app.quit).
  {
    id: "app.quit",
    name: "Quit",
    keys: { cua: "Mod+q", vim: ":q" },
    menu: { path: ["App"], group: 2, order: 0 },
    dispatch: "app",
    service: "app",
    verb: "quit",
  },

  // ─── app.command ────────────────────────────────────────────────────────
  // YAML: keys vim:":" / cua:Mod+Shift+P / emacs:Mod+Shift+P. Routes to
  // ui_state `show command` (open palette in command mode) for the window.
  {
    id: "app.command",
    name: "Command Palette",
    keys: { vim: ":", cua: "Mod+Shift+P", emacs: "Mod+Shift+P" },
    dispatch: "uiState",
    service: "command",
    verb: "show",
    passScope: true,
  },

  // ─── app.palette ────────────────────────────────────────────────────────
  // YAML: visible:false; keys cua:Mod+Shift+P / vim:Mod+Shift+P /
  // emacs:Mod+Shift+P. Routes to ui_state `show palette` (open palette
  // without forcing a mode) for the window.
  {
    id: "app.palette",
    name: "Command Palette",
    visible: false,
    keys: { cua: "Mod+Shift+P", vim: "Mod+Shift+P", emacs: "Mod+Shift+P" },
    dispatch: "uiState",
    service: "palette",
    verb: "show",
    passScope: true,
  },

  // ─── app.search ─────────────────────────────────────────────────────────
  // YAML: keys vim:"/" / cua:Mod+f (canonicalized lowercase), menu {path:[Edit],
  // group:0, order:2}. The emacs key is dropped — see the file header for the
  // Mod+f / nav.right conflict. Routes to ui_state `show search` (open palette
  // in search mode) for the window.
  {
    id: "app.search",
    name: "Find",
    keys: { vim: "/", cua: "Mod+f" },
    menu: { path: ["Edit"], group: 0, order: 2 },
    dispatch: "uiState",
    service: "search",
    verb: "show",
    passScope: true,
  },

  // ─── app.dismiss ────────────────────────────────────────────────────────
  // Routes to ui_state `dismiss ui` (layered close — palette first, then
  // inspector) for the window.
  //
  // No longer keyed to Escape (card `01KTPDTH772HSEV5F7R1DKYDNJ`): Escape is
  // owned globally by `nav.drillOut`, which drills out one focus level and,
  // at a layer-root edge, falls through to this same `dismiss ui` op. Binding
  // Escape here too made `app.dismiss` a competing Escape owner that the
  // first-id-wins keymap layer could pick over `nav.drillOut`. The command id
  // survives — it is still dispatched programmatically (inspector backdrop
  // click, quick-capture) and discoverable in the palette — just unbound from
  // Escape.
  {
    id: "app.dismiss",
    name: "Dismiss",
    dispatch: "uiState",
    service: "ui",
    verb: "dismiss",
    passScope: true,
  },

  // ─── app.undo ───────────────────────────────────────────────────────────
  // YAML: undoable:false (undo is not itself an undoable action); keys
  // cua:Mod+z (canonicalized lowercase) / vim:u, menu {path:[Edit], group:0,
  // order:0}. Routes to the store server's `undo stack` — the one unified stack
  // that spans every store in the substrate, NOT the app shell.
  //
  // The emacs `Ctrl+/` binding moved here from `app-shell.tsx`'s deleted
  // `STATIC_GLOBAL_COMMANDS` (Card I): the registry is now the only key
  // source for the webview hotkey path, so the key must live on this
  // registration to keep emacs-mode undo working.
  {
    id: "app.undo",
    name: "Undo",
    undoable: false,
    keys: { cua: "Mod+z", vim: "u", emacs: "Ctrl+/" },
    menu: { path: ["Edit"], group: 0, order: 0 },
    dispatch: "store",
    service: "stack",
    verb: "undo",
  },

  // ─── app.redo ───────────────────────────────────────────────────────────
  // YAML: undoable:false; keys cua:Mod+Shift+Z / vim:Mod+r (canonicalized —
  // `BINDING_TABLES.vim` binds `Mod+r`; the legacy `Ctrl+R` literal was
  // unreachable, see the file header), menu {path:[Edit], group:0, order:1}.
  // Routes to the store server's `redo stack`.
  {
    id: "app.redo",
    name: "Redo",
    undoable: false,
    keys: { cua: "Mod+Shift+Z", vim: "Mod+r" },
    menu: { path: ["Edit"], group: 0, order: 1 },
    dispatch: "store",
    service: "stack",
    verb: "redo",
  },
];

/** Build the nine `app.*` command registrations.
 *
 * Each `APP_COMMANDS` row's routing data (`dispatch` / `service` / `verb` /
 * `passScope`) is interpreted by {@link appRun} into a `run`, which
 * `bindCommandRun` binds to an `execute` over the live dispatch surfaces. The
 * routing-only fields are stripped — they drive `appRun` but are not `register
 * command` metadata. */
export function appCommands(
  app: AppDispatch,
  store: StoreDispatch,
  uiState: UiStateDispatch,
): CommandSpec[] {
  const dispatch: AppDispatchBundle = { app, store, uiState };
  return APP_COMMANDS.map((spec) => {
    const {
      dispatch: _dispatch,
      service: _service,
      verb: _verb,
      passScope: _passScope,
      ...metadata
    } = spec;
    return bindCommandRun(
      { ...metadata, run: appRun(spec) },
      dispatch,
    ) as CommandSpec;
  });
}
