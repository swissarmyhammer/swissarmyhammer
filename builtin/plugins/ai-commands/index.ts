// ai-commands — builtin plugin porting `ai.yaml` (the five AI-panel
// window-layer commands) to the TypeScript plugin SDK. This is the port that
// retires the LAST non-nav YAML command source
// (`crates/swissarmyhammer-kanban/builtin/commands/ai.yaml`), bringing the AI
// panel into the same builtin-command-plugin model as the other seven bundles.
//
// ───────────────────────────────────────────────────────────────────────────
// Why every `execute` is a webview-reactive no-op
// ───────────────────────────────────────────────────────────────────────────
//
// The AI panel's open-state, conversation, ACP session, and per-board model
// selection all live entirely in the React tree (per board, in `localStorage`
// / `useConversation`). There is NO backend store — and deliberately no `ai`
// MCP server — for any of it. The frontend owns the live behaviour:
// `apps/kanban-app/ui/src/components/app-shell.tsx` registers a webview
// command-bus handler for each of the five `ai.*` ids (Card I — the
// `registerWebviewCommandHandler` seam), routing the dispatch through the
// `ai/commands.ts` module bus (`triggerAiToggle`, `triggerAiFocus`, …) into
// the mounted AI panel.
//
// This bundle therefore matches the grid/board/UI-surface bundles: the
// command is registered through the `CommandService` so it carries its full
// palette / menu / keybinding metadata in the one unified command registry,
// but its backend `execute` is a deliberate no-op returning `null` — the
// webview bus intercepts the id in `useDispatchCommand` and runs the real
// effect before any dispatch would reach a backend. No `ai` server is
// required, and none is created.
//
// ───────────────────────────────────────────────────────────────────────────
// `ai.cancel` availability — event-driven cached flag
// ───────────────────────────────────────────────────────────────────────────
//
// `ai.cancel` stops an in-flight generation, so it is meaningful only while the
// conversation is streaming. The AI panel's conversation lifecycle lives
// entirely in the webview (`useConversation` → `aiStreaming()` in
// `ai/commands.ts`), which the plugin isolate cannot read synchronously. So the
// plugin gates the REGISTRY palette entry with the event-driven cached-flag
// pattern (command-service.md): it subscribes to the `aiStreaming` notification
// the `ui_state` server declares, caches the flag, and returns it from a
// SYNCHRONOUS `available` callback on `ai.cancel`.
//
// The publish point is the `ai_set_streaming` Tauri command, which builds the
// `notifications/ui_state/ai_streaming` notification (from the ui-state crate's
// `AiStreamingChanged` payload) and publishes it onto the `NotificationBridge`
// of the host that answers the streaming window's palette — the window's
// per-board host when it has a board open (the AI panel mounts in a board
// window), else the global host. It resolves that host from the calling
// window's label the SAME way `command_tool_call` routes the `available command`
// query, so the publish and the availability read hit the same isolate's cached
// flag, and `this.ui_state.on("aiStreaming", …)` delivers each change here.
//
// The webview bus handler for `ai.cancel` (registered by `AppShell`) ALSO gates
// at dispatch time on `aiStreaming()` — that remains the authoritative
// execution gate (keybinding + palette dispatch both funnel through it). This
// plugin gate governs the registry palette's enabled/disabled rendering, so a
// user sees "Stop AI Generation" greyed-out while idle, matching the dispatch
// behaviour.
//
// Backend routing — 5 commands, no backend (webview-reactive no-ops):
//   ai.toggle   → no-op (frontend `triggerAiToggle`)
//   ai.focus    → no-op (frontend `triggerAiFocus`)
//   ai.newChat  → no-op (frontend `triggerAiNewChat`)
//   ai.model    → no-op (frontend `triggerAiModel`, reads the `model` arg)
//   ai.cancel   → no-op (frontend `triggerAiCancel`); gated by the cached flag.

import {
  Plugin,
  ensureServices,
  registerCommands,
  type Availability,
} from "@swissarmyhammer/plugin";

/**
 * The cached AI-streaming flag, updated by the `aiStreaming` subscription and
 * read synchronously by `ai.cancel`'s `available` callback.
 *
 * Module-level rather than an instance field because the `AI_COMMANDS` data
 * table (which the frontend drift guard parses from source) is module-level and
 * `available` closes over this flag from there. Each plugin runs in its OWN
 * isolate, so this module-level state is per-plugin-instance — there is exactly
 * one `AiCommandsPlugin` per isolate, so there is no cross-instance sharing to
 * worry about. Defaults to `false`: a freshly-loaded plugin is not mid-stream.
 */
let cachedStreaming = false;

/**
 * The five `ai.*` window-layer command registrations, as a module-level data
 * table (the same hoisted-table structure as `nav-commands` / `grid-commands`,
 * which lets the frontend drift guard
 * `ai-plugin-commands-mirror.spatial.node.test.ts` parse it from source).
 *
 * `keys` use the canonical lowercase form the webview's `normalizeKeyEvent`
 * emits for an unshifted letter chord (`Mod+j`, not `Mod+J`). Since Card I
 * removed the React-side `buildAiCommands` scope defs, this registry metadata
 * is the ONLY key source for the webview hotkey path — `extractKeymapBindings`
 * reads the strings literally, so an uppercase unshifted letter would be
 * unreachable from a real keydown. The native menu accelerator (ai.toggle's
 * View-menu entry) parses letters case-insensitively, so the lowercase form
 * serves both sides. The drift guard pins this against `BINDING_TABLES`.
 */
const AI_COMMANDS = [
  // ─── ai.toggle ──────────────────────────────────────────────────────────
  // ai.yaml: keys cua/vim/emacs Mod+J (canonicalized to Mod+j, see above);
  // menu {path:[View], group:0, order:0}; undoable:false. Show/hide the AI
  // panel.
  {
    id: "ai.toggle",
    name: "Toggle AI Panel",
    undoable: false,
    keys: { cua: "Mod+j", vim: "Mod+j", emacs: "Mod+j" },
    menu: { path: ["View"], group: 0, order: 0 },
    execute: () => null,
  },

  // ─── ai.focus ───────────────────────────────────────────────────────────
  // ai.yaml: keys cua/vim/emacs Mod+I (canonicalized to Mod+i); undoable:
  // false. Move keyboard focus into the AI panel's prompt input.
  {
    id: "ai.focus",
    name: "Focus AI Panel",
    undoable: false,
    keys: { cua: "Mod+i", vim: "Mod+i", emacs: "Mod+i" },
    execute: () => null,
  },

  // ─── ai.newChat ─────────────────────────────────────────────────────────
  // ai.yaml: keys cua/vim/emacs Mod+Shift+J (already canonical — a shifted
  // letter keeps its uppercase); undoable:false. Start a fresh stateless AI
  // chat, clearing the current conversation.
  {
    id: "ai.newChat",
    name: "New AI Chat",
    undoable: false,
    keys: { cua: "Mod+Shift+J", vim: "Mod+Shift+J", emacs: "Mod+Shift+J" },
    execute: () => null,
  },

  // ─── ai.model ───────────────────────────────────────────────────────────
  // ai.yaml: undoable:false; param model(from:args, shape:enum,
  // options_from:"ai.models"). The palette renders a picker whose options
  // the `ai.models` backend resolver fills at emission time; the chosen
  // model id rides in `args.model`, which the frontend applies as the
  // per-board model selection.
  {
    id: "ai.model",
    name: "Set AI Model",
    undoable: false,
    params: [
      { name: "model", from: "args", shape: "enum", options_from: "ai.models" },
    ],
    execute: () => null,
  },

  // ─── ai.cancel ──────────────────────────────────────────────────────────
  // ai.yaml: keys cua/vim/emacs Mod+. (already canonical — punctuation keeps
  // no case); undoable:false. Stop the in-flight AI generation. The
  // registry-palette availability gate reads the event-driven cached
  // `cachedStreaming` flag (kept in sync by the `aiStreaming` subscription in
  // `load`): unavailable while idle, available mid-stream. `ok: false` is set
  // EXPLICITLY — an object missing `ok` is treated as available by the command
  // service's `interpret_available`.
  {
    id: "ai.cancel",
    name: "Stop AI Generation",
    undoable: false,
    keys: { cua: "Mod+.", vim: "Mod+.", emacs: "Mod+." },
    available: (): Availability =>
      cachedStreaming
        ? { ok: true }
        : { ok: false, reason: "No AI generation is running" },
    execute: () => null,
  },
];

/**
 * The ai-commands builtin plugin.
 *
 * Registers the five `ai.*` window-layer commands ported from `ai.yaml`, each
 * with its source metadata 1:1. Identity is the bundle directory name
 * (`ai-commands`); `name` / `description` are descriptive metadata only.
 */
export default class AiCommandsPlugin extends Plugin {
  /** Human-readable name — descriptive metadata only, not plugin identity. */
  readonly name = "AI Commands";

  /** One-line description — descriptive metadata only. */
  readonly description =
    "Builtin AI-panel window-layer commands (toggle / focus / new chat / set model / stop generation). The behaviour is webview-side, so each backend execute is a deliberate no-op; the registrations carry the palette / menu / keybinding metadata in the unified command registry.";

  /**
   * Activate the `commands` + `ui_state` services, subscribe to the
   * AI-streaming notification, then register the five `ai.*` commands.
   *
   * The convention every command-registering plugin follows: `ensureServices`
   * FIRST — so the registries are live before any registration — then
   * `registerCommands`. `commands` carries the registrations; `ui_state` is the
   * server that declares the `aiStreaming` notification this plugin subscribes
   * to so `ai.cancel`'s `available` flag tracks the live conversation.
   *
   * The subscription updates the module-level {@link cachedStreaming} flag on
   * every published streaming-status change (from the `ai_set_streaming` Tauri
   * command's `notifications/ui_state/ai_streaming` publish). The teardown the
   * `.on()` returns is auto-disposed on plugin unload, so no `unload()` body is
   * needed. The command registrations route to no backend (the webview owns the
   * effect); their metadata is `ai.yaml`'s metadata, 1:1.
   */
  async load(): Promise<void> {
    await ensureServices(this, ["commands", "ui_state"]);

    // Cache the webview's streaming status as it changes, so `ai.cancel`'s
    // synchronous `available` callback can gate the registry palette entry
    // without a synchronous handle to the webview-only conversation.
    this.ui_state.on("aiStreaming", (params: unknown) => {
      cachedStreaming = (params as { streaming?: unknown })?.streaming === true;
    });

    await registerCommands(this, AI_COMMANDS);

    this.log.info(
      `ai-commands: registered ${AI_COMMANDS.length} commands (${AI_COMMANDS.map(
        (c) => c.id,
      ).join(" / ")}) as webview-reactive no-ops; ai.cancel gated on the ` +
        `aiStreaming notification`,
    );
  }
}
