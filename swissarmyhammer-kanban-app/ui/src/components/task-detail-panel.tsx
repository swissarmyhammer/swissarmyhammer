import { useRef } from "react";
import { X } from "lucide-react";
import { EditableMarkdown } from "@/components/editable-markdown";
import { SubtaskProgress } from "@/components/subtask-progress";
import { TagPill } from "@/components/tag-pill";
import type { Tag, Task } from "@/types/kanban";

interface TaskDetailPanelProps {
  task: Task | null;
  tags?: Tag[];
  onClose: () => void;
  onUpdateTitle?: (taskId: string, title: string) => void;
  onUpdateDescription?: (taskId: string, description: string) => void;
  style?: React.CSSProperties;
}

export function TaskDetailPanel({ task, tags = [], onClose, onUpdateTitle, onUpdateDescription, style }: TaskDetailPanelProps) {
  const panelRef = useRef<HTMLDivElement>(null);

  return (
    <div
      ref={panelRef}
      className={`fixed top-0 h-full w-[420px] max-w-[85vw] bg-background border-l border-border shadow-xl flex flex-col transition-transform duration-200 ease-out ${
        task ? "translate-x-0" : "translate-x-full"
      }`}
      style={style}
    >
      {task && (
        <>
          {/* Header */}
          <div className="flex items-start justify-between gap-3 px-5 pt-5 pb-3">
            <EditableMarkdown
              value={task.title}
              onCommit={(title) => onUpdateTitle?.(task.id, title)}
              className="text-lg font-semibold leading-snug flex-1 cursor-text"
              inputClassName="text-lg font-semibold leading-snug flex-1 bg-transparent border-b border-ring w-full"
            />
            <button
              onClick={onClose}
              className="shrink-0 mt-0.5 p-1 rounded-md text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
            >
              <X className="h-4 w-4" />
            </button>
          </div>

          {task.tags.length > 0 && (
            <div className="flex flex-wrap gap-1.5 px-5 mb-2">
              {task.tags.map((tagId) => (
                <TagPill key={tagId} slug={tagId} tags={tags} taskId={task.id} />
              ))}
            </div>
          )}

          <SubtaskProgress description={task.description} className="mx-5 mb-3" />

          <div className="mx-3 h-px bg-border" />

          {/* Content */}
          <div className="flex-1 min-h-0 overflow-y-auto px-5 pb-5 flex flex-col gap-5">
            {/* Description — fills available space so clicking empty area enters edit */}
            <section className="flex-1 flex flex-col">
              <EditableMarkdown
                value={task.description ?? ""}
                onCommit={(desc) => onUpdateDescription?.(task.id, desc)}
                className="text-sm leading-relaxed cursor-text flex-1"
                inputClassName="text-sm leading-relaxed bg-transparent w-full flex-1"
                multiline
                placeholder="Add description..."
                tags={tags}
              />
            </section>

            {/* Assignees */}
            {task.assignees.length > 0 && (
              <section>
                <h3 className="text-xs font-medium text-muted-foreground uppercase tracking-wide mb-2">
                  Assignees
                </h3>
                <div className="flex flex-wrap gap-1.5">
                  {task.assignees.map((assignee) => (
                    <span
                      key={assignee}
                      className="inline-flex items-center rounded-full bg-accent px-2.5 py-0.5 text-xs font-medium text-accent-foreground"
                    >
                      {assignee}
                    </span>
                  ))}
                </div>
              </section>
            )}

            {/* Dependencies */}
            {task.depends_on.length > 0 && (
              <section>
                <h3 className="text-xs font-medium text-muted-foreground uppercase tracking-wide mb-2">
                  Depends on
                </h3>
                <div className="space-y-1">
                  {task.depends_on.map((depId) => (
                    <p
                      key={depId}
                      className="text-sm text-muted-foreground font-mono"
                    >
                      {depId}
                    </p>
                  ))}
                </div>
              </section>
            )}

          </div>
        </>
      )}
    </div>
  );
}
