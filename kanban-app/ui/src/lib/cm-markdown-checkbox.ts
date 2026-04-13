/**
 * CM6 ViewPlugin that renders markdown task-list checkboxes as interactive
 * `<input type="checkbox">` widgets.
 *
 * Scans the viewport for `- [ ]` / `- [x]` patterns via MatchDecorator and
 * replaces each match with a widget. When the user clicks a checkbox, the
 * widget computes its 0-based index among all checkboxes in the document
 * and invokes the `onToggle(sourceIndex)` callback provided via the
 * `checkboxToggleFacet`.
 *
 * Used by the read-only markdown viewer to preserve interactive task-list
 * checkboxes that previously lived in the ReactMarkdown pipeline.
 */

import { Facet } from "@codemirror/state";
import {
  Decoration,
  type DecorationSet,
  EditorView,
  MatchDecorator,
  ViewPlugin,
  type ViewUpdate,
  WidgetType,
} from "@codemirror/view";

/** Matches a markdown task-list checkbox: `- [ ]`, `- [x]`, or `- [X]`. */
const CHECKBOX_RE = /- \[([ xX])\]/g;

/**
 * Facet carrying the `onToggle` callback invoked when a rendered checkbox
 * is clicked. The callback receives the 0-based source index of the
 * toggled checkbox (counted across all matches in the document).
 *
 * Callers provide the callback via `checkboxToggleFacet.of(fn)` in their
 * extension array; the widget reads it from the view state on every click.
 */
export const checkboxToggleFacet = Facet.define<
  (sourceIndex: number) => void,
  ((sourceIndex: number) => void) | null
>({
  combine(values) {
    return values.length > 0 ? values[values.length - 1] : null;
  },
});

/**
 * Count how many `- [ ]` / `- [x]` patterns occur in `source` strictly
 * before `pos`. Used to derive the 0-based source index of a checkbox
 * widget from its document position.
 *
 * @param source - The full document text.
 * @param pos - The character position of the checkbox match's start.
 * @returns The number of prior checkbox matches.
 */
function countCheckboxesBefore(source: string, pos: number): number {
  let count = 0;
  CHECKBOX_RE.lastIndex = 0;
  let m: RegExpExecArray | null;
  while ((m = CHECKBOX_RE.exec(source)) !== null) {
    if (m.index >= pos) break;
    count++;
  }
  return count;
}

/**
 * CM6 widget that renders a single markdown task-list checkbox.
 *
 * Carries the match's starting document position and checked state. On
 * click, it recomputes its source index from the current document so it
 * remains correct even if other checkboxes are edited between renders.
 */
class CheckboxWidget extends WidgetType {
  constructor(
    readonly from: number,
    readonly checked: boolean,
  ) {
    super();
  }

  /**
   * Build the DOM: a `<span>` wrapping an `<input type="checkbox">`.
   *
   * The span carries `cm-markdown-checkbox` for styling; the input is
   * disabled for read-only display but still fires `click`/`change`
   * events so the facet callback can run.
   */
  toDOM(view: EditorView): HTMLElement {
    const span = document.createElement("span");
    span.className = "cm-markdown-checkbox";

    const input = document.createElement("input");
    input.type = "checkbox";
    input.checked = this.checked;

    // Stop clicks from reaching the editor / parent handlers.
    input.addEventListener("click", (e) => {
      e.stopPropagation();
      const onToggle = view.state.facet(checkboxToggleFacet);
      if (!onToggle) return;
      const source = view.state.doc.toString();
      const index = countCheckboxesBefore(source, this.from);
      onToggle(index);
    });

    span.appendChild(input);
    return span;
  }

  /**
   * Reuse DOM when both position and checked state match. This keeps
   * CM6 from rebuilding widgets on unrelated updates.
   */
  eq(other: CheckboxWidget): boolean {
    return this.from === other.from && this.checked === other.checked;
  }

  /**
   * Let events bubble so CM6 selection handling still works around the
   * widget (matches the mention widget's convention).
   */
  ignoreEvent(): boolean {
    return false;
  }
}

/**
 * Build the MatchDecorator that replaces each `- [ ]` / `- [x]` with a
 * CheckboxWidget anchored to the match's start position.
 */
function buildCheckboxDecorator(): MatchDecorator {
  return new MatchDecorator({
    regexp: CHECKBOX_RE,
    decorate(add, from, to, match) {
      const checked = match[1] !== " ";
      add(
        from,
        to,
        Decoration.replace({
          widget: new CheckboxWidget(from, checked),
          inclusive: false,
        }),
      );
    },
  });
}

/**
 * Create the markdown-checkbox ViewPlugin extension.
 *
 * The plugin maintains a DecorationSet via MatchDecorator and rebuilds
 * it on document/viewport changes. Clicks on rendered checkboxes fire
 * the `onToggle` callback supplied through `checkboxToggleFacet`.
 */
export function createMarkdownCheckboxPlugin() {
  const decorator = buildCheckboxDecorator();

  return ViewPlugin.fromClass(
    class {
      decorations: DecorationSet;

      constructor(view: EditorView) {
        this.decorations = decorator.createDeco(view);
      }

      update(update: ViewUpdate) {
        if (update.docChanged || update.viewportChanged) {
          this.decorations = decorator.updateDeco(update, this.decorations);
        }
      }
    },
    { decorations: (v) => v.decorations },
  );
}
