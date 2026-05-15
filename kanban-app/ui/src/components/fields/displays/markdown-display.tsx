import { useCallback, useMemo } from "react";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import type { Extension } from "@codemirror/state";
import { TextViewer } from "@/components/text-viewer";
import { useMentionExtensions } from "@/hooks/use-mention-extensions";
import {
  createMarkdownCheckboxPlugin,
  checkboxToggleFacet,
} from "@/lib/cm-markdown-checkbox";
import { CompactCellWrapper } from "./compact-cell-wrapper";
import type { DisplayProps } from "./text-display";

/** Matches a markdown task-list checkbox: `- [ ]`, `- [x]`, or `- [X]`. */
const CHECKBOX_RE = /- \[([ xX])\]/g;

/**
 * Toggle the Nth checkbox in `source`, returning the updated string.
 *
 * Counts matches left-to-right and flips the `index`-th one between
 * `- [ ]` and `- [x]`. Returns `null` if fewer than `index + 1` matches
 * exist in the source.
 */
function toggleCheckbox(source: string, index: number): string | null {
  let count = 0;
  let replaced = false;
  const out = source.replace(CHECKBOX_RE, (match, check) => {
    if (count++ === index) {
      replaced = true;
      return check === " " ? "- [x]" : "- [ ]";
    }
    return match;
  });
  return replaced ? out : null;
}

interface MarkdownDisplayProps extends Omit<DisplayProps, "onCommit"> {
  onCommit?: (value: string) => void;
}

/**
 * Markdown display — compact: truncated plain text, full: a read-only
 * CM6 viewer with the markdown language, mention decoration/widget
 * extensions, and an interactive task-list checkbox plugin.
 *
 * Compact mode is intentionally plain text: a miniature editor per row
 * in list views would be wasteful and visually noisy.
 */
export function MarkdownDisplay({
  value,
  mode,
  onCommit,
}: MarkdownDisplayProps) {
  const text = typeof value === "string" ? value : "";

  if (mode === "compact") {
    const inner = !text ? (
      <span className="text-muted-foreground/50">-</span>
    ) : (
      <span className="truncate block">{text}</span>
    );
    return <CompactCellWrapper>{inner}</CompactCellWrapper>;
  }

  if (!text) {
    return <span className="text-muted-foreground italic">Empty</span>;
  }

  return <MarkdownFull text={text} onCommit={onCommit} />;
}

/**
 * Full-mode markdown viewer: CM6 TextViewer with markdown language,
 * mention widgets, and the task-list checkbox plugin.
 *
 * Checkbox clicks are bridged into `onCommit` via the checkbox plugin's
 * facet: the plugin computes the 0-based source index of the clicked
 * checkbox and invokes `onToggle`, which we use to mutate the markdown
 * source and fire `onCommit` with the updated text.
 */
function MarkdownFull({
  text,
  onCommit,
}: {
  text: string;
  onCommit?: (value: string) => void;
}) {
  const mentionExtensions = useMentionExtensions();

  const handleToggle = useCallback(
    (sourceIndex: number) => {
      if (!onCommit) return;
      const updated = toggleCheckbox(text, sourceIndex);
      if (updated !== null) onCommit(updated);
    },
    [text, onCommit],
  );

  const extensions = useMemo<Extension[]>(
    () => [
      markdown({ base: markdownLanguage }),
      ...mentionExtensions,
      createMarkdownCheckboxPlugin(),
      checkboxToggleFacet.of(handleToggle),
    ],
    [mentionExtensions, handleToggle],
  );

  return (
    <div className="prose prose-sm dark:prose-invert max-w-none">
      <TextViewer text={text} extensions={extensions} />
    </div>
  );
}
