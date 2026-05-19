/**
 * The AI panel's prompt composer — a CodeMirror 6 instance.
 *
 * # CM6 everywhere
 *
 * Per `ideas/kanban/app-architecture.md`, every text input in the app is a
 * CodeMirror 6 editor honoring the user's keymap (vim / emacs / CUA). The AI
 * panel composer is no exception: it is built on the shared {@link TextEditor}
 * primitive — the exact same primitive behind the filter formula bar, the
 * markdown field editor, the inline rename, and quick-capture — so vim
 * motions, emacs bindings, and CUA editing all work inside it. It is NOT a
 * plain `<textarea>`.
 *
 * # AI Elements `PromptInput` layout
 *
 * The composer adopts the AI Elements `PromptInput` *layout*: one bordered
 * container holding the CM6 editor body above a footer toolbar. The footer
 * carries the model selector (left) and the submit/stop control (right). Only
 * the *layout* is borrowed — the editor body stays the CM6 {@link TextEditor},
 * not the AI Elements `PromptInputTextarea` (a plain `<textarea>`).
 *
 * The single bordered container is the only border around the input — the
 * hosting `ComposerArea` (in `ai-panel.tsx`) no longer adds its own
 * `border-t`, so there is no doubled edge.
 *
 * The CM6 editor body flexes to fill the panel's available vertical space:
 * the body is `flex-1`/`min-h-0` and the CM6 `.cm-editor` / `.cm-scroller`
 * fill it (`h-full`), so the prompt area grows with the panel. The footer
 * toolbar stays pinned at the bottom of the container.
 *
 * # Focus scopes: the prompt and the picker are independent siblings
 *
 * The CM6 prompt and the footer model picker are two INDEPENDENT spatial-nav
 * controls. The bordered shell carries NO focus scope; instead the
 * `ui:ai-panel.composer` scope wraps ONLY the CM6 editor body — so landing on
 * it and drilling in (Enter) focuses the CM6 prompt, exactly like the filter
 * formula bar's `filter_editor:${id}` scope. The footer's `ComposerModelSelect`
 * registers its own `ui:ai-panel.model-selector` leaf. With no scope on the
 * shell the two compose their FQM directly under the `ui:ai-panel` zone —
 * `/window/ui:ai-panel/ui:ai-panel.composer` and
 * `/window/ui:ai-panel/ui:ai-panel.model-selector` — as siblings, neither
 * nested inside the other.
 *
 * Drill-in actually moving the cursor in is NOT automatic: a bare
 * `<FocusScope>` only registers the scope as a nav target. The composer
 * scope is given a per-scope `ui.ai-panel.composer.drillIn` `CommandDef`
 * (keyed to Enter for every keymap) whose `execute` calls
 * `editorRef.current?.focus()` — the shared `TextEditor` primitive's
 * `TextEditorHandle.focus()`. That command shadows the global
 * `nav.drillIn: Enter` for the composer scope and is what drives the CM6
 * editing cursor in. This mirrors `FilterFormulaBarFocusable`'s
 * `filter_editor.drillIn` command in `perspective-tab-bar.tsx`.
 *
 * # Submit / stop policy
 *
 * The composer is a chat box: plain `Enter` submits the buffer, `Shift-Enter`
 * inserts a newline (a multi-line prompt). This mirrors the prior textarea's
 * behavior and is keymap-agnostic — `Enter` sends in vim insert mode too,
 * exactly as a chat composer is expected to behave. While a turn streams the
 * submit button becomes a stop control wired to `onCancel`.
 *
 * `TextEditor` is a pure string-editing primitive that owns no submit policy
 * (see its file docstring); this component supplies an Enter-submit keymap
 * via the `extensions` prop.
 *
 * # Why a bespoke keymap, not the shared `buildSubmitCancelExtensions`
 *
 * `FilterEditor` and the markdown adapter route their submit/cancel through
 * the shared `buildSubmitCancelExtensions` helper (`@/lib/cm-submit-cancel.ts`).
 * The composer deliberately does NOT — that helper cannot express the chat
 * composer's policy:
 *   - The helper always wires an Escape→cancel binding. Inside the composer
 *     Escape must stay a plain vim insert→normal toggle; the composer's cancel
 *     is the stop button, not a key. There is no "cancel" callback to give it.
 *   - The helper's CUA/emacs Enter binding is gated on `singleLine`, not on
 *     `alwaysSubmitOnEnter`. A multi-line composer (`singleLine: false`) would
 *     therefore get NO Enter-submit binding in CUA/emacs mode, while
 *     `singleLine: true` would suppress vim insert-mode newline insertion and
 *     break multi-line prompts. No single flag combination yields
 *     "Enter always submits, Shift-Enter always inserts a newline".
 * So this component hand-rolls a `Prec.highest` Enter binding (see
 * {@link buildEnterSubmitExtension}). It intentionally omits the helper's
 * `completionStatus` autocomplete-yield guard because the composer has no
 * autocomplete (`TextEditor`'s `BASIC_SETUP` disables it and the composer adds
 * no mention extensions); a future maintainer adding autocomplete here must
 * re-introduce that guard.
 */

import { useCallback, useMemo, useRef, type ReactNode } from "react";
import { EditorView, keymap } from "@codemirror/view";
import { Prec, type Extension } from "@codemirror/state";
import { SquareIcon, CornerDownLeftIcon } from "lucide-react";
import {
  TextEditor,
  type TextEditorHandle,
} from "@/components/fields/text-editor";
import {
  AiPanelFocusScope,
  AiPanelPressable,
} from "@/components/ai-panel-focus";
import type { CommandDef } from "@/lib/command-scope";
import {
  PromptInputSelect,
  PromptInputSelectContent,
  PromptInputSelectItem,
  PromptInputSelectTrigger,
  PromptInputSelectValue,
} from "@/components/ai-elements/prompt-input";
import { asSegment } from "@/types/spatial";
import { cn } from "@/lib/utils";
import type { AiModel } from "@/components/ai-panel";

/** Props for {@link AiPromptComposer}. */
export interface AiPromptComposerProps {
  /** When true the editor is read-only and the action button is disabled. */
  disabled: boolean;
  /** Placeholder shown while the buffer is empty. */
  placeholder: string;
  /** Whether a prompt turn is currently streaming. */
  streaming: boolean;
  /**
   * Submit the composed prompt. Called with the trimmed buffer text on a
   * plain `Enter` (non-empty buffer) or a click of the submit button.
   */
  onSend: (text: string) => void;
  /** Stop the in-flight turn — wired to the stop button while streaming. */
  onCancel: () => void;
  /**
   * The selectable models, or `undefined` while the container is still
   * fetching `ai_list_models`. Drives the footer model picker.
   */
  models: AiModel[] | undefined;
  /** The currently selected model, or `null` when none is chosen yet. */
  selectedModel: AiModel | null;
  /**
   * Report the user's model choice — wired to the footer model picker. The
   * container persists it per board and feeds the new id back down.
   */
  onSelectModel: (modelId: string) => void;
}

/**
 * Build the CM6 Enter-submit keymap extension.
 *
 * A `Prec.highest` binding so plain `Enter` is intercepted ahead of CM6's
 * default `insertNewline`. `Shift-Enter` is deliberately left unbound, so it
 * falls through to the default keymap and inserts a newline — the multi-line
 * prompt affordance.
 *
 * On a non-empty buffer plain `Enter` submits. On an empty (or whitespace-only)
 * buffer plain `Enter` is a true no-op: the handler returns `true` to swallow
 * the keystroke, so it neither submits nor falls through to `insertNewline`.
 * Without this, repeated `Enter` on an empty composer would pile up blank
 * lines.
 *
 * The submit callback is read through a ref so the extension identity stays
 * stable across re-renders and never reconfigures the live `EditorView`.
 */
function buildEnterSubmitExtension(
  submitRef: React.RefObject<(() => void) | null>,
): Extension {
  return Prec.highest(
    keymap.of([
      {
        key: "Enter",
        run: (view) => {
          // An empty (or whitespace-only) buffer has nothing to send. Swallow
          // the keystroke (return true) so it neither submits nor inserts a
          // blank line — a stray Enter on an empty composer is a true no-op.
          if (view.state.doc.toString().trim().length === 0) {
            return true;
          }
          submitRef.current?.();
          return true;
        },
      },
    ]),
  );
}

/** Props for {@link ComposerModelSelect}. */
interface ComposerModelSelectProps {
  models: AiModel[] | undefined;
  selectedModel: AiModel | null;
  onSelectModel: (modelId: string) => void;
}

/**
 * The footer model picker — the AI Elements `PromptInputSelect*` family.
 *
 * Lists every model from `ai_list_models`; an unavailable model is a disabled
 * option that still surfaces its hint (e.g. "install Claude Code"). Selecting
 * an available one reports the choice via `onSelectModel`.
 *
 * The trigger is the `ui:ai-panel.model-selector` spatial-nav focus leaf:
 * `<AiPanelPressable asChild>` mounts the leaf and the Enter / Space
 * keyboard-activation CommandDefs, and the Radix `Select` trigger becomes the
 * host `<button>`. `onPress` is a no-op — Radix's own trigger handler opens
 * the listbox; the Pressable is here purely for the focus leaf and keyboard
 * activation. `ariaLabel` is the trigger's visible label (the model name, or
 * "Select a model") so the accessible name matches the text the button shows.
 */
function ComposerModelSelect({
  models,
  selectedModel,
  onSelectModel,
}: ComposerModelSelectProps): ReactNode {
  const triggerLabel = selectedModel?.label ?? "Select a model";
  const hasModels = !!models && models.length > 0;

  return (
    <PromptInputSelect
      value={selectedModel?.id}
      onValueChange={onSelectModel}
      disabled={!hasModels}
    >
      <AiPanelPressable
        asChild
        moniker={asSegment("ui:ai-panel.model-selector")}
        ariaLabel={triggerLabel}
        onPress={() => {}}
        disabled={!hasModels}
      >
        <PromptInputSelectTrigger size="sm">
          <PromptInputSelectValue placeholder="Select a model" />
        </PromptInputSelectTrigger>
      </AiPanelPressable>
      <PromptInputSelectContent align="start">
        {(models ?? []).map((model) => (
          <PromptInputSelectItem
            key={model.id}
            value={model.id}
            // An unavailable model cannot be picked, but its hint stays
            // visible so the user knows why (e.g. the Claude Code CLI was
            // not found).
            disabled={!model.available}
            title={model.hint ?? undefined}
            className="flex-col items-start gap-0.5"
          >
            <span>{model.label}</span>
            {model.hint && (
              <span className="text-muted-foreground text-xs">
                {model.hint}
              </span>
            )}
          </PromptInputSelectItem>
        ))}
      </PromptInputSelectContent>
    </PromptInputSelect>
  );
}

/**
 * The AI panel's prompt composer.
 *
 * Renders the AI Elements `PromptInput`-style shell: a single bordered
 * container with the CM6 editor body above a footer toolbar holding the model
 * selector and the submit/stop action button. The CM6 body flexes to fill the
 * available height; the footer stays pinned at the bottom. While a turn
 * streams the action button is a stop control.
 *
 * This component is the inner editor surface; its hosting `ComposerArea`
 * (in `ai-panel.tsx`) owns the panel-section padding and the "New
 * conversation" action above it.
 */
export function AiPromptComposer({
  disabled,
  placeholder,
  streaming,
  onSend,
  onCancel,
  models,
  selectedModel,
  onSelectModel,
}: AiPromptComposerProps): ReactNode {
  const editorRef = useRef<TextEditorHandle>(null);

  // The live buffer is the source of truth (the `TextEditor` primitive does
  // not re-apply `value` after mount), so submit reads it imperatively.
  const handleSubmit = useCallback(() => {
    // While a turn streams the action is "stop", not "send".
    if (streaming) {
      onCancel();
      return;
    }
    const text = editorRef.current?.getValue().trim() ?? "";
    if (text.length === 0) {
      return;
    }
    onSend(text);
    // Clear the composer once the prompt is handed off, ready for the next.
    editorRef.current?.setValue("");
  }, [streaming, onCancel, onSend]);

  // Stable ref to the latest submit handler — keeps the Enter-submit keymap
  // extension identity stable so the `EditorView` is never reconfigured
  // mid-typing.
  const submitRef = useRef<(() => void) | null>(handleSubmit);
  submitRef.current = handleSubmit;

  // CM6 extensions: the Enter-submit keymap, plus a read-only toggle and the
  // accessible-name attribute on the content DOM. The content DOM keeps
  // `aria-label="Message the AI agent"` so `ai.focus` and the panel tests can
  // locate the prompt input by its accessible name.
  const extensions = useMemo<Extension[]>(
    () => [
      buildEnterSubmitExtension(submitRef),
      EditorView.editable.of(!disabled),
      EditorView.contentAttributes.of({
        "aria-label": "Message the AI agent",
      }),
    ],
    [disabled],
  );

  // Per-scope drill-in command for the CM6 body's `ui:ai-panel.composer`
  // scope. A bare `<FocusScope>` only *registers* the scope as a nav
  // target — landing on it and pressing Enter does not move the editing
  // cursor into the editor. This `CommandDef` (keyed to Enter for every
  // keymap) shadows the global `nav.drillIn: Enter` for the composer
  // scope and calls `editorRef.current?.focus()`, the shared
  // `TextEditor` primitive's `TextEditorHandle.focus()` (which drives
  // the underlying CM6 `view.focus()`). This is the exact pattern
  // `FilterFormulaBarFocusable` uses for the filter formula bar's
  // `filter_editor.drillIn` command (see `perspective-tab-bar.tsx`).
  const drillInCommands = useMemo<readonly CommandDef[]>(
    () => [
      {
        id: "ui.ai-panel.composer.drillIn",
        name: "Edit Prompt",
        keys: { cua: "Enter", vim: "Enter", emacs: "Enter" },
        execute: () => {
          editorRef.current?.focus();
        },
      },
    ],
    [],
  );

  return (
    // The single bordered container — the AI Elements `PromptInput` shell.
    // It is the only border around the input; `ComposerArea` no longer adds
    // its own, so there is no doubled edge. `flex flex-col` stacks the CM6
    // body above the footer toolbar; `min-h-0 flex-1` lets the shell flex to
    // fill the height its `ComposerArea` section gives it.
    <div
      data-slot="ai-prompt-composer"
      className={cn(
        "flex min-h-0 flex-1 flex-col rounded-md border bg-background",
        disabled && "opacity-60",
      )}
    >
      {/* Only the CM6 editor body is a focus scope — `ui:ai-panel.composer`
          under the panel zone. Landing on it and drilling in (Enter)
          focuses the CM6 prompt, exactly like the filter formula bar's
          `filter_editor:${id}` scope. The `drillInCommands` array carries
          the per-scope `ui.ai-panel.composer.drillIn` `CommandDef` keyed
          to Enter — that command is what actually drives the editing
          cursor into the CM6 editor on drill-in (a bare scope only
          registers the nav target). The footer toolbar — with the model
          picker's own `ui:ai-panel.model-selector` leaf — stays OUTSIDE
          this scope, so the two are independent spatial-nav siblings.
          `<FocusScope>` deliberately does NOT steal a click that lands
          inside the CM6 editor, so caret placement in the prompt is
          untouched. The CM6 editor body flexes to fill the container's
          available height: `min-h-24` is a content-height floor so the
          prompt area never collapses below a few lines; `flex-1` lets it
          grow past that floor with the panel. The `[&_.cm-editor]` /
          `[&_.cm-scroller]` arbitrary selectors make the CM6 surfaces
          themselves fill the body so the prompt area grows rather than
          staying content-height. */}
      <AiPanelFocusScope
        moniker={asSegment("ui:ai-panel.composer")}
        commands={drillInCommands}
        className="min-h-24 flex-1 overflow-auto px-2 py-1.5 [&_.cm-editor]:h-full [&_.cm-scroller]:h-full"
      >
        <TextEditor
          ref={editorRef}
          value=""
          extensions={extensions}
          placeholder={placeholder}
          autoFocus={false}
        />
      </AiPanelFocusScope>
      {/* The footer toolbar — pinned at the bottom of the container. The
          model selector sits on the left, the submit/stop control on the
          right. */}
      <div className="flex items-center justify-between gap-2 px-2 py-1.5">
        <ComposerModelSelect
          models={models}
          selectedModel={selectedModel}
          onSelectModel={onSelectModel}
        />
        <div className="flex items-center gap-2">
          <span className="text-muted-foreground text-xs">
            {streaming ? "Streaming - click to stop" : ""}
          </span>
          <button
            type="button"
            aria-label={streaming ? "Stop" : "Submit"}
            disabled={disabled}
            onClick={handleSubmit}
            className={cn(
              "inline-flex size-7 items-center justify-center rounded-md",
              "bg-primary text-primary-foreground transition-colors",
              "hover:bg-primary/90 disabled:pointer-events-none disabled:opacity-50",
            )}
          >
            {streaming ? (
              <SquareIcon className="size-4" />
            ) : (
              <CornerDownLeftIcon className="size-4" />
            )}
          </button>
        </div>
      </div>
    </div>
  );
}
