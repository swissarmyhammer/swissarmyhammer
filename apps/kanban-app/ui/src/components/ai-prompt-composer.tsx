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
import { cn } from "@/lib/utils";

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

/**
 * The AI panel's prompt composer.
 *
 * Renders the CM6 editor and the submit/stop action button. The editor sits
 * in its own bordered well; while a turn streams the action button is a stop
 * control and a "Streaming — click to stop" hint shows below the editor.
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

  return (
    <div data-slot="ai-prompt-composer">
      {/* The editor well — bordered to read as an input, mirroring the
          inspector's CM6 field wells. */}
      <div
        className={cn(
          "rounded-md border bg-background px-2 py-1.5",
          disabled && "opacity-60",
        )}
      >
        <TextEditor
          ref={editorRef}
          value=""
          extensions={extensions}
          placeholder={placeholder}
          autoFocus={false}
        />
      </div>
      <div className="mt-1 flex items-center justify-between">
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
  );
}
