import { forwardRef, useCallback, useMemo, useRef } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { useSchema } from "@/lib/schema-context";
import { useEntityStore } from "@/lib/entity-store-context";
import { remarkMentions } from "@/lib/remark-mentions";
import { TagPill } from "@/components/tag-pill";
import { MentionPill } from "@/components/mention-pill";
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
 * GFM, mention pills for all mentionable types, and interactive checkboxes.
 */
export function MarkdownDisplay({ value, mode, onCommit }: MarkdownDisplayProps) {
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
      onCommit={onCommit}
    />
  );
}

const MarkdownFull = forwardRef<HTMLDivElement, {
  text: string;
  onCommit?: (value: string) => void;
}>(function MarkdownFull({ text, onCommit }, ref) {
  const { mentionableTypes } = useSchema();
  const { getEntities } = useEntityStore();

  const mentionData = useMemo(() => {
    return mentionableTypes.map((mt) => {
      const entities = getEntities(mt.entityType);
      return {
        ...mt,
        entities,
        slugs: entities.map((e) => getStr(e, mt.displayField)).filter(Boolean),
      };
    });
  }, [mentionableTypes, getEntities]);

  const remarkPlugins = useMemo(() => {
    const plugins: Array<ReturnType<typeof remarkMentions> | typeof remarkGfm> = [remarkGfm];
    for (const md of mentionData) {
      if (md.slugs.length === 0) continue;
      plugins.push(
        remarkMentions(md.prefix, md.slugs, `${md.entityType}Pill`, `${md.entityType}-pill`)
      );
    }
    return plugins;
  }, [mentionData]);

  const mentionComponents = useMemo(() => {
    const comps: Record<string, React.ComponentType> = {};
    for (const md of mentionData) {
      if (md.entityType === "tag") {
        comps["tag-pill"] = (props: { slug?: string }) => (
          <TagPill slug={props.slug ?? ""} tags={md.entities} />
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
        ) as any;
      } else {
        comps[`${md.entityType}-pill`] = (props: { slug?: string }) => (
          <MentionPill
            entityType={md.entityType}
            slug={props.slug ?? ""}
            prefix={md.prefix}
          />
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
        ) as any;
      }
    }
    return comps;
  }, [mentionData]);

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
        remarkPlugins={remarkPlugins}
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
          ...(mentionComponents as Record<string, React.ComponentType>),
        }}
      >
        {text}
      </ReactMarkdown>
    </div>
  );
});
