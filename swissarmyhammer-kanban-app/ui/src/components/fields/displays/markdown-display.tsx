import { forwardRef, useCallback, useMemo, useRef } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { remarkTags } from "@/lib/remark-tags";
import { TagPill } from "@/components/tag-pill";
import type { Entity } from "@/types/kanban";
import { getStr } from "@/types/kanban";
import type { DisplayProps } from "./text-display";

const CHECKBOX_RE = /- \[([ xX])\]/g;

function toggleCheckbox(source: string, index: number): string | null {
  let count = 0;
  return source.replace(CHECKBOX_RE, (match, check) => {
    if (count++ === index) {
      return check === " " ? "- [x]" : "- [ ]";
    }
    return match;
  });
}

interface MarkdownDisplayProps extends DisplayProps {
  tags?: Entity[];
  onCommit?: (value: string) => void;
}

/**
 * Markdown display — compact: truncated plain text, full: rendered ReactMarkdown with
 * GFM, tag pills, and interactive checkboxes.
 */
export function MarkdownDisplay({ value, mode, tags, onCommit }: MarkdownDisplayProps) {
  const text = typeof value === "string" ? value : "";
  const displayRef = useRef<HTMLDivElement>(null);

  if (!text) {
    return mode === "compact"
      ? <span className="text-muted-foreground/50">-</span>
      : <span className="text-muted-foreground italic">Empty</span>;
  }

  if (mode === "compact") {
    return <span className="truncate block">{text}</span>;
  }

  return (
    <MarkdownFull
      ref={displayRef}
      text={text}
      tags={tags}
      onCommit={onCommit}
    />
  );
}

const MarkdownFull = forwardRef<HTMLDivElement, {
  text: string;
  tags?: Entity[];
  onCommit?: (value: string) => void;
}>(function MarkdownFull({ text, tags, onCommit }, ref) {
  const knownSlugs = useMemo(
    () => (tags ? tags.map((t) => getStr(t, "tag_name")) : []),
    [tags],
  );

  const handleCheckboxChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const container = (ref as React.RefObject<HTMLDivElement>)?.current;
      if (!container || !onCommit) return;
      const all = container.querySelectorAll('input[type="checkbox"]');
      const idx = Array.from(all).indexOf(e.target);
      if (idx >= 0) {
        const updated = toggleCheckbox(text, idx);
        if (updated !== null) onCommit(updated);
      }
    },
    [text, onCommit, ref],
  );

  return (
    <div ref={ref} className="prose prose-sm dark:prose-invert max-w-none">
      <ReactMarkdown
        remarkPlugins={[remarkGfm, remarkTags(knownSlugs)]}
        components={{
          input: (props) => {
            if (props.type === "checkbox") {
              return (
                <input
                  type="checkbox"
                  checked={props.checked ?? false}
                  onChange={handleCheckboxChange}
                  onClick={(e) => e.stopPropagation()}
                />
              );
            }
            return <input {...props} />;
          },
          ...({
            // eslint-disable-next-line @typescript-eslint/no-explicit-any
            "tag-pill": (props: any) => (
              <TagPill slug={props.slug ?? ""} tags={tags ?? []} />
            ),
          } as Record<string, React.ComponentType>),
        }}
      >
        {text}
      </ReactMarkdown>
    </div>
  );
});
