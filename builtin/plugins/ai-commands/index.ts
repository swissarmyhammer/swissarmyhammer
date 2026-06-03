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
// MCP server — for any of it. The frontend already owns the live behaviour:
// `apps/kanban-app/ui/src/components/app-shell.tsx`'s `buildAiCommands(...)`
// registers the same five `ai.*` ids into the window-layer React command scope
// and routes each `execute` through the `ai/commands.ts` module bus
// (`triggerAiToggle`, `triggerAiFocus`, …) into the mounted AI panel.
//
// This bundle therefore mirrors the established `ui.entity.startRename`
// precedent: the command is registered through the `CommandService` so it
// carries its full palette / menu / keybinding metadata in the one unified
// command registry, but its backend `execute` is a deliberate no-op returning
// `null` — the webview intercepts the id and runs the real effect before any
// dispatch would reach a backend. No `ai` server is required, and none is
// created.
//
// ───────────────────────────────────────────────────────────────────────────
// `ai.cancel` availability
// ───────────────────────────────────────────────────────────────────────────
//
// `ai.cancel` stops an in-flight generation, so it is meaningful only while the
// conversation is streaming. That live gate is owned frontend-side: the React
// `buildAiCommands` rebuilds `ai.cancel` with `available: streaming` whenever
// the conversation's turn status flips (driven by `ai/commands.ts`'s
// `subscribeAiStreaming`). The backend registration here leaves `available`
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

    const commands = [
      // ─── ai.toggle ──────────────────────────────────────────────────────
      // ai.yaml: keys cua/vim/emacs Mod+J; menu {path:[View], group:0,
      // order:0}; undoable:false. Show/hide the AI panel.
      {
        id: "ai.toggle",
        name: "Toggle AI Panel",
        undoable: false,
        keys: { cua: "Mod+J", vim: "Mod+J", emacs: "Mod+J" },
        menu: { path: ["View"], group: 0, order: 0 },
        execute: () => null,
      },

      // ─── ai.focus ───────────────────────────────────────────────────────
      // ai.yaml: keys cua/vim/emacs Mod+I; undoable:false. Move keyboard
      // focus into the AI panel's prompt input.
      {
        id: "ai.focus",
        name: "Focus AI Panel",
        undoable: false,
        keys: { cua: "Mod+I", vim: "Mod+I", emacs: "Mod+I" },
        execute: () => null,
      },

      // ─── ai.newChat ─────────────────────────────────────────────────────
      // ai.yaml: keys cua/vim/emacs Mod+Shift+J; undoable:false. Start a
      // fresh stateless AI chat, clearing the current conversation.
      {
        id: "ai.newChat",
        name: "New AI Chat",
        undoable: false,
        keys: { cua: "Mod+Shift+J", vim: "Mod+Shift+J", emacs: "Mod+Shift+J" },
        execute: () => null,
      },

      // ─── ai.model ───────────────────────────────────────────────────────
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

      // ─── ai.cancel ──────────────────────────────────────────────────────
      // ai.yaml: keys cua/vim/emacs Mod+.; undoable:false. Stop the in-flight
      // AI generation. The streaming-only availability gate is owned
      // frontend-side (see the file header); the backend registration leaves
      // `available` absent.
      {
        id: "ai.cancel",
        name: "Stop AI Generation",
        undoable: false,
        keys: { cua: "Mod+.", vim: "Mod+.", emacs: "Mod+." },
        execute: () => null,
      },
    ];

    await registerCommands(this, commands);

    this.log.info(
      `ai-commands: registered ${commands.length} commands (${commands
        .map((c) => c.id)
        .join(" / ")}) as webview-reactive no-ops`,
    );
  }
}
