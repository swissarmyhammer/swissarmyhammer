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
// `ai.cancel` availability
// ───────────────────────────────────────────────────────────────────────────
//
// `ai.cancel` stops an in-flight generation, so it is meaningful only while the
// conversation is streaming. That live gate is owned frontend-side: the
// webview bus handler for `ai.cancel` (registered by `AppShell`) reads
// `ai/commands.ts`'s `aiStreaming()` at dispatch time and no-ops when the
// conversation is idle. The backend registration here leaves `available`
// absent (always-available at the registry layer) because the command-service
// `available` callback has no view into the webview-only streaming flag — the
// authoritative gate is the frontend one, exactly as it was under the legacy
// YAML/Rust pairing where `AiCancelCmd::available()` read a transient flag the
// webview mirrored in.
//
// Consequence: the registry-driven palette (`useCommandList` /
// `useCommandAvailability` → backend `available command`) shows
// "Stop AI Generation" as enabled even when idle. Gating it there needs the
// event-driven cached-flag pattern (command-service.md), which in turn needs
// the SDK event/subscription API (`on`/`subscribe`) that is currently
// RESERVED/inert (sdk/plugin.ts `reservedHandler`). Tracked as a follow-up:
// kanban 01KT7DB01HTR9SNRRG145F009P ("Gate ai.cancel in the palette via
// event-driven cached availability").
//
// Backend routing — 5 commands, no backend (webview-reactive no-ops):
//   ai.toggle   → no-op (frontend `triggerAiToggle`)
//   ai.focus    → no-op (frontend `triggerAiFocus`)
//   ai.newChat  → no-op (frontend `triggerAiNewChat`)
//   ai.model    → no-op (frontend `triggerAiModel`, reads the `model` arg)
//   ai.cancel   → no-op (frontend `triggerAiCancel`)

import {
  Plugin,
  ensureServices,
  registerCommands,
} from "@swissarmyhammer/plugin";

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
  // streaming-only availability gate is owned frontend-side (see the file
  // header); the backend registration leaves `available` absent.
  {
    id: "ai.cancel",
    name: "Stop AI Generation",
    undoable: false,
    keys: { cua: "Mod+.", vim: "Mod+.", emacs: "Mod+." },
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
   * Activate the `commands` registry, then register the five `ai.*` commands.
   *
   * The convention every command-registering plugin follows: `ensureServices`
   * FIRST — so the `commands` registry is live before any registration — then
   * `registerCommands`. These commands route to no backend (the webview owns
   * the effect), so `commands` is the only service required. The metadata on
   * each registration is `ai.yaml`'s metadata, 1:1.
   */
  async load(): Promise<void> {
    await ensureServices(this, ["commands"]);

    await registerCommands(this, AI_COMMANDS);

    this.log.info(
      `ai-commands: registered ${AI_COMMANDS.length} commands (${AI_COMMANDS.map(
        (c) => c.id,
      ).join(" / ")}) as webview-reactive no-ops`,
    );
  }
}
