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
 * # Escape drills out — back to the composer scope
 *
 * Once the CM6 prompt has DOM focus, Escape drills OUT: it blurs the
 * contenteditable and dispatches `nav.focus` to return kernel spatial focus
 * to the `ui:ai-panel.composer` scope, so the panel stops trapping keys and
 * `s` (jump) works again. This is the exact `nav.drillOut` story the filter
 * formula bar uses — see `FilterEditorDrillOutWiring` in
 * `perspective-tab-bar.tsx`.
 *
 * The drill-out routes Escape through the SHARED
 * `buildSubmitCancelExtensions` helper (`@/lib/cm-submit-cancel.ts`) — the
 * exact same mechanism every other CM6 editor in the app (the filter formula
 * bar, the inline rename, the markdown field, the date / single-select /
 * multi-select editors, and the command palette) uses for its Escape policy.
 * The helper's vim-mode Escape is a two-phase DOM capture listener that
 * correctly preempts `@replit/codemirror-vim`'s insert-mode Escape (a
 * `keymap.of` binding would lose the race to the vim plugin), so the
 * composer's drill-out fires from vim insert mode too. The CUA / emacs
 * branch is a `Prec.highest` keymap binding. The drill-out callback is
 * composed inside the composer scope (where the scope's FQM is available) by
 * {@link ComposerEditorDrillOutWiring} and handed to the CM6 editor body via
 * {@link ComposerEditorEscapeContext}; the body passes it as
 * `onCancelRef` to `buildSubmitCancelExtensions`. In the no-spatial-stack
 * unit-test path the context value is `null`, so the cancel callback ref
 * is `null` and the helper's Escape handler is an inert no-op, exactly like
 * the filter editor mounted bare.
 *
 * # Why we still hand-roll the Enter-submit binding
 *
 * The composer's drill-out shares `buildSubmitCancelExtensions` with every
 * other CM6 editor, but the Enter-submit policy is composer-specific: plain
 * Enter must always submit (including in vim insert mode), Shift-Enter must
 * always insert a newline. The helper's Enter binding is gated on
 * `singleLine`, so neither flag combination expresses "Enter always submits,
 * Shift-Enter always inserts a newline". So the composer calls
 * `buildSubmitCancelExtensions` with `singleLine: false` — the helper then
 * contributes only its Escape handling — and supplies its own `Prec.highest`
 * Enter binding (see {@link buildEnterSubmitExtension}).
 *
 * # Slash-command autocomplete
 *
 * The composer hosts a `/` slash-command completion menu fed by the agent's
 * live ACP `availableCommands` (the {@link AiPromptComposerProps.availableCommands}
 * prop), assembled by {@link useCommandCompletionExtension} on the same shared
 * CM6 autocomplete primitive as the filter editor's `#`/`@`/`^` mentions. Because
 * the menu can be open when Enter is pressed, {@link buildEnterSubmitExtension}
 * carries the `completionStatus` autocomplete-yield guard: with the menu open
 * (or an async source pending) plain Enter accepts the highlighted completion
 * instead of submitting. Accepting `/plan` inserts the literal text `/plan` —
 * the agent owns command execution; the composer only aids discoverability.
 */

import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useRef,
  type ReactNode,
} from "react";
import { EditorView, keymap } from "@codemirror/view";
import { Prec, type Extension } from "@codemirror/state";
import { completionStatus } from "@codemirror/autocomplete";
import type { AvailableCommand } from "@agentclientprotocol/sdk";
import { SquareIcon, CornerDownLeftIcon } from "lucide-react";
import {
  TextEditor,
  type TextEditorHandle,
} from "@/components/fields/text-editor";
import { useCommandCompletionExtension } from "@/hooks/use-command-completion";
import {
  AiPanelFocusScope,
  AiPanelPressable,
} from "@/components/ai-panel-focus";
import { useOptionalFullyQualifiedMoniker } from "@/components/fully-qualified-moniker-context";
import { useDispatchCommand, type CommandDef } from "@/lib/command-scope";
import { useUIState } from "@/lib/ui-state-context";
import { buildSubmitCancelExtensions } from "@/lib/cm-submit-cancel";
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
  /**
   * The slash commands the agent currently advertises (ACP
   * `available_commands_update`). Drives the composer's `/` autocomplete menu.
   * Defaults to `[]` — typing `/` then opens no menu and Enter submits
   * normally.
   */
  availableCommands?: AvailableCommand[];
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
 * # Autocomplete-yield guard
 *
 * The composer now hosts a `/` slash-command autocomplete (see {@link
 * useCommandCompletionExtension}). When the completion menu is open, plain
 * `Enter` must accept the highlighted completion, not submit the buffer. The
 * binding returns `false` in that case so it yields to CM6's completion keymap
 * (which binds `Enter` to `acceptCompletion`).
 *
 * The guard tests `completionStatus(view.state) === "active"` — NOT `!== null`
 * like the shared `cm-submit-cancel.ts` helper. The difference is deliberate:
 * `activateOnTyping` transiently reports `"pending"` while it debounces over
 * ordinary (non-`/`) prose, and a chat composer must still submit on Enter in
 * that window. Only an actually-open menu (`"active"`) yields. (CM6's own
 * `interactionDelay` still guards against an Enter landing in the first ~75ms
 * after the menu opens; within that window acceptance is refused and Enter
 * falls through to a newline, exactly as everywhere else CM6 autocomplete is
 * used.)
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
          // Yield to autocomplete so Enter accepts the highlighted completion
          // instead of submitting — only when the menu is actually open.
          if (completionStatus(view.state) === "active") {
            return false;
          }
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

/**
 * Carries the composer scope's "Escape from inside CM6" drill-out handler
 * down to the descendant CM6 editor body without prop-drilling.
 *
 * Provided by {@link ComposerEditorDrillOutWiring} (which sits inside the
 * `ui:ai-panel.composer` `<FocusScope>` so it can compose the scope's FQM)
 * and consumed by {@link ComposerEditorBody}, which wires it into the shared
 * {@link buildSubmitCancelExtensions} helper as its `onCancelRef`. `null`
 * when no spatial-nav stack is present (a standalone unit test) — the helper's
 * Escape handler is then an inert no-op. Same shape as
 * `FilterEditorEscapeContext` in `perspective-tab-bar.tsx`.
 */
const ComposerEditorEscapeContext = createContext<(() => void) | null>(null);

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
 * host `<button>`. `ariaLabel` is the trigger's visible label (the model
 * name, or "Select a model") so the accessible name matches the text the
 * button shows.
 *
 * # Keyboard hand-off to the Radix Select
 *
 * A Radix `Select` needs DOM focus on its trigger `<button>` for Space /
 * Enter / ↑↓ to open and navigate the listbox. Spatial-nav focusing the leaf
 * does NOT move DOM focus there, so `onPress` (the leaf's Enter / Space
 * activation) calls `triggerRef.current?.focus()` — a real focus hand-off
 * onto the trigger, mirroring the composer scope's CM6 `drillIn` command
 * calling `editorRef.current?.focus()`. After the hand-off Radix's own
 * trigger handler drives every subsequent keystroke. A pointer click is
 * unaffected: Radix focuses the trigger and opens the listbox itself, and a
 * redundant `.focus()` on the already-focused trigger is a no-op.
 */
function ComposerModelSelect({
  models,
  selectedModel,
  onSelectModel,
}: ComposerModelSelectProps): ReactNode {
  const triggerLabel = selectedModel?.label ?? "Select a model";
  const hasModels = !!models && models.length > 0;

  // The Radix select trigger `<button>`. Activating the model-picker leaf
  // (drill-in / Enter / Space) hands DOM focus to it so the Radix Select's
  // own keyboard interaction (Space/Enter/↑↓) becomes reachable.
  const triggerRef = useRef<HTMLButtonElement>(null);

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
        onPress={() => triggerRef.current?.focus()}
        disabled={!hasModels}
      >
        <PromptInputSelectTrigger ref={triggerRef} size="sm">
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
 * Compose the composer scope's "Escape from inside CM6" drill-out handler
 * and publish it to the descendant CM6 editor body via context.
 *
 * Rendered INSIDE the `ui:ai-panel.composer` `<FocusScope>` so it can read
 * that scope's composed FQM. The drill-out handler blurs the CM6
 * contenteditable (the kernel's spatial-focus update alone does not move DOM
 * focus, so the caret would keep blinking) and dispatches `nav.focus` to
 * return kernel spatial focus to the composer scope — after which the panel
 * stops trapping keys and `s` (jump) works again. The handler body is the
 * same shape as `FilterEditorDrillOutWiring` in `perspective-tab-bar.tsx`.
 *
 * One intentional simplification over the filter reference: there is no
 * outer `FilterFormulaBarFocusable` guard checking the spatial-nav stack
 * separately from FQM availability. Instead this single component reads
 * `useOptionalFullyQualifiedMoniker()` directly — outside the spatial-nav
 * stack (a standalone unit test) it returns `null`, the context is then
 * provided as `null`, and the cancel callback ref in `ComposerEditorBody`
 * stays `null` so the shared helper's Escape handler is an inert no-op,
 * exactly like the filter editor mounted bare.
 */
function ComposerEditorDrillOutWiring({
  children,
}: {
  children: ReactNode;
}): ReactNode {
  const composerFq = useOptionalFullyQualifiedMoniker();
  // Focus claims flow through the single auditable `nav.focus` command (card
  // `01KR7CDEFWWVF4WH0BCHE8Y21J`) — the same primitive the filter-editor
  // drill-out uses.
  const dispatchNavFocus = useDispatchCommand("nav.focus");

  const onEditorEscape = useMemo<(() => void) | null>(() => {
    if (!composerFq) return null;
    return () => {
      // Drop DOM focus from the CM6 contenteditable so the caret stops
      // blinking — the kernel's spatial-focus update alone does not move
      // DOM focus.
      if (
        typeof document !== "undefined" &&
        document.activeElement instanceof HTMLElement
      ) {
        document.activeElement.blur();
      }
      void dispatchNavFocus({ args: { fq: composerFq } }).catch((err) =>
        console.error(
          "[ComposerEditorDrillOutWiring] nav.focus dispatch failed",
          err,
        ),
      );
    };
  }, [composerFq, dispatchNavFocus]);

  return (
    <ComposerEditorEscapeContext.Provider value={onEditorEscape}>
      {children}
    </ComposerEditorEscapeContext.Provider>
  );
}

/** Props for {@link ComposerEditorBody}. */
interface ComposerEditorBodyProps {
  /** Ref to the shared `TextEditor` primitive — drives drill-in `focus()`. */
  editorRef: React.RefObject<TextEditorHandle | null>;
  /**
   * Base CM6 extensions from {@link AiPromptComposer} — the Enter-submit
   * keymap, the read-only toggle, and the accessible-name content attribute.
   * The shared `buildSubmitCancelExtensions` Escape handling is appended here
   * because it needs the active keymap mode and the scope-provided
   * drill-out callback, both of which are only available below the composer
   * scope.
   */
  baseExtensions: Extension[];
  /** Placeholder shown while the buffer is empty. */
  placeholder: string;
}

/**
 * The composer's CM6 editor body — the `<TextEditor>` and its extensions.
 *
 * Rendered inside the `ui:ai-panel.composer` `<FocusScope>` (below
 * {@link ComposerEditorDrillOutWiring}) so it can consume the drill-out
 * callback from {@link ComposerEditorEscapeContext} and route it through the
 * shared {@link buildSubmitCancelExtensions} helper. The helper is invoked
 * with `singleLine: false` so it contributes ONLY its Escape handling — the
 * composer keeps its own Enter-submit binding (see {@link
 * buildEnterSubmitExtension}). `onSubmitRef` is a no-op ref because the
 * helper's Enter path is disabled by `singleLine: false`; `onCancelRef` is
 * the drill-out callback. The cancel callback is read through a stable ref
 * so the extension identity never churns.
 *
 * The helper's vim-mode Escape uses a two-phase DOM capture listener (see
 * `cm-submit-cancel.ts`) so the drill-out preempts `@replit/codemirror-vim`'s
 * insert-mode Escape and fires from vim insert mode too. Its CUA / emacs
 * branch is a `Prec.highest` keymap binding.
 */
function ComposerEditorBody({
  editorRef,
  baseExtensions,
  placeholder,
}: ComposerEditorBodyProps): ReactNode {
  // The scope-provided drill-out callback. `null` outside the spatial-nav
  // stack — the helper's Escape handler is then an inert no-op (calls
  // through `?.()` on a null ref).
  const onEditorEscape = useContext(ComposerEditorEscapeContext);
  const { keymap_mode: keymapMode } = useUIState();

  // Stable ref to the drill-out callback — keeps the extension identity
  // stable so the live `EditorView` is never reconfigured.
  const cancelRef = useRef<(() => void) | null>(onEditorEscape);
  cancelRef.current = onEditorEscape;

  // The helper requires a submit ref even when `singleLine: false` (where
  // its Enter binding is disabled). Hold a stable no-op ref so the helper
  // can call `?.()` harmlessly if anything else ever wires to it.
  const submitNoopRef = useRef<(() => void) | null>(null);

  const submitCancelExts = useMemo(
    () =>
      buildSubmitCancelExtensions({
        mode: keymapMode,
        onSubmitRef: submitNoopRef,
        onCancelRef: cancelRef,
        // Enter is owned by the composer's own bespoke `Prec.highest`
        // binding (plain Enter always submits, Shift-Enter always inserts a
        // newline — a policy the helper's Enter binding cannot express).
        // `singleLine: false` makes the helper contribute only Escape.
        singleLine: false,
      }),
    [keymapMode],
  );

  const extensions = useMemo<Extension[]>(
    () => [...baseExtensions, ...submitCancelExts],
    [baseExtensions, submitCancelExts],
  );

  return (
    <TextEditor
      ref={editorRef}
      value=""
      extensions={extensions}
      placeholder={placeholder}
      autoFocus={false}
    />
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
  availableCommands = [],
}: AiPromptComposerProps): ReactNode {
  const editorRef = useRef<TextEditorHandle>(null);

  // The `/` slash-command autocomplete extension, fed by the live ACP
  // `availableCommands`. An empty list yields an empty array, so typing `/`
  // opens no menu and Enter submits per the normal policy.
  const commandExtensions = useCommandCompletionExtension(availableCommands);

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

  // Base CM6 extensions: the Enter-submit keymap, plus a read-only toggle and
  // the accessible-name attribute on the content DOM. The content DOM keeps
  // `aria-label="Message the AI agent"` so `ai.focus` and the panel tests can
  // locate the prompt input by its accessible name. The Escape drill-out
  // keymap is appended inside `ComposerEditorBody`, which can read the
  // scope-provided drill-out callback.
  const baseExtensions = useMemo<Extension[]>(
    () => [
      buildEnterSubmitExtension(submitRef),
      EditorView.editable.of(!disabled),
      EditorView.contentAttributes.of({
        "aria-label": "Message the AI agent",
      }),
      // The `/` slash-command autocomplete — empty when the agent advertises
      // no commands. Assembled on the same shared CM6 autocomplete primitive
      // as the filter editor's mention completions.
      ...commandExtensions,
    ],
    [disabled, commandExtensions],
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
        {/* `ComposerEditorDrillOutWiring` sits INSIDE the composer scope so
            it can compose the scope's FQM and publish the Escape drill-out
            callback; `ComposerEditorBody` consumes it and wires the CM6
            Escape keymap. */}
        <ComposerEditorDrillOutWiring>
          <ComposerEditorBody
            editorRef={editorRef}
            baseExtensions={baseExtensions}
            placeholder={placeholder}
          />
        </ComposerEditorDrillOutWiring>
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
