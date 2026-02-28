import { useEffect, useRef } from "react";
import { X } from "lucide-react";
import { EditableMarkdown } from "@/components/editable-markdown";
import type { Task } from "@/types/kanban";

interface TaskDetailPanelProps {
  task: Task | null;
  onClose: () => void;
  onUpdateTitle?: (taskId: string, title: string) => void;
  onUpdateDescription?: (taskId: string, description: string) => void;
}

export function TaskDetailPanel({ task, onClose, onUpdateTitle, onUpdateDescription }: TaskDetailPanelProps) {
  const panelRef = useRef<HTMLDivElement>(null);

  // Close on Escape key
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape") {
        // Don't close the panel if an editable field is focused
        const tag = (e.target as HTMLElement)?.tagName;
        if (tag === "INPUT" || tag === "TEXTAREA") return;
        // Don't close if a CodeMirror editor is focused
        if ((e.target as HTMLElement)?.closest?.(".cm-editor")) return;
        onClose();
      }
    }
    if (task) {
      document.addEventListener("keydown", handleKeyDown);
      return () => document.removeEventListener("keydown", handleKeyDown);
    }
  }, [task, onClose]);

  return (
    <>
      {/* Backdrop */}
      <div
        className={`fixed inset-0 bg-black/20 transition-opacity duration-200 ${
          task ? "opacity-100" : "opacity-0 pointer-events-none"
        }`}
        onClick={onClose}
      />

      {/* Panel */}
      <div
        ref={panelRef}
        className={`fixed top-0 right-0 h-full w-[420px] max-w-[85vw] bg-background border-l border-border shadow-xl flex flex-col transition-transform duration-200 ease-out ${
          task ? "translate-x-0" : "translate-x-full"
        }`}
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

            <div className="mx-3 h-px bg-border" />

            {/* Content */}
            <div className="flex-1 overflow-y-auto px-5 pb-5 space-y-5">
              {/* Description */}
              <section>
                <EditableMarkdown
                  value={task.description ?? ""}
                  onCommit={(desc) => onUpdateDescription?.(task.id, desc)}
                  className="text-sm leading-relaxed cursor-text"
                  inputClassName="text-sm leading-relaxed bg-transparent w-full"
                  multiline
                  placeholder="Add description..."
                />
              </section>

              {/* Tags */}
              {task.tags.length > 0 && (
                <section>
                  <h3 className="text-xs font-medium text-muted-foreground uppercase tracking-wide mb-2">
                    Tags
                  </h3>
                  <div className="flex flex-wrap gap-1.5">
                    {task.tags.map((tag) => (
                      <span
                        key={tag}
                        className="inline-flex items-center rounded-full bg-secondary px-2.5 py-0.5 text-xs font-medium text-secondary-foreground"
                      >
                        {tag}
                      </span>
                    ))}
                  </div>
                </section>
              )}

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

              {/* Metadata */}
              <section className="border-t border-border pt-4">
                <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1.5 text-xs">
                  <dt className="text-muted-foreground">ID</dt>
                  <dd className="font-mono">{task.id}</dd>
                  <dt className="text-muted-foreground">Column</dt>
                  <dd>{task.position.column}</dd>
                  {task.position.swimlane && (
                    <>
                      <dt className="text-muted-foreground">Swimlane</dt>
                      <dd>{task.position.swimlane}</dd>
                    </>
                  )}
                  <dt className="text-muted-foreground">Created</dt>
                  <dd>{new Date(task.created_at).toLocaleDateString()}</dd>
                  <dt className="text-muted-foreground">Updated</dt>
                  <dd>{new Date(task.updated_at).toLocaleDateString()}</dd>
                </dl>
              </section>
            </div>
          </>
        )}
      </div>
    </>
  );
}
