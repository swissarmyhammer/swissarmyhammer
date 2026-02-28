import { useEffect, useRef } from "react";
import { X } from "lucide-react";
import { EditableText } from "@/components/editable-text";
import type { Task } from "@/types/kanban";

interface TaskDetailPanelProps {
  task: Task | null;
  onClose: () => void;
  onUpdateTitle?: (taskId: string, title: string) => void;
}

export function TaskDetailPanel({ task, onClose, onUpdateTitle }: TaskDetailPanelProps) {
  const panelRef = useRef<HTMLDivElement>(null);

  // Close on Escape key
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
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
              <EditableText
                value={task.title}
                onCommit={(title) => onUpdateTitle?.(task.id, title)}
                className="text-lg font-semibold leading-snug flex-1 cursor-text"
                inputClassName="text-lg font-semibold leading-snug flex-1 bg-transparent border-b border-ring outline-none w-full"
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
                {task.description ? (
                  <p className="text-sm leading-relaxed whitespace-pre-wrap">
                    {task.description}
                  </p>
                ) : (
                  <p className="text-sm text-muted-foreground italic">
                    No description
                  </p>
                )}
              </section>

              {/* Subtasks */}
              {task.subtasks.length > 0 && (
                <section>
                  <h3 className="text-xs font-medium text-muted-foreground uppercase tracking-wide mb-2">
                    Subtasks
                    <span className="ml-2 text-muted-foreground">
                      {task.subtasks.filter((s) => s.completed).length}/
                      {task.subtasks.length}
                    </span>
                  </h3>
                  <ul className="space-y-1">
                    {task.subtasks.map((sub) => (
                      <li
                        key={sub.id}
                        className="flex items-center gap-2 text-sm"
                      >
                        <span
                          className={`h-4 w-4 rounded border flex items-center justify-center shrink-0 ${
                            sub.completed
                              ? "bg-primary border-primary text-primary-foreground"
                              : "border-input"
                          }`}
                        >
                          {sub.completed && (
                            <svg
                              className="h-3 w-3"
                              fill="none"
                              viewBox="0 0 24 24"
                              stroke="currentColor"
                              strokeWidth={3}
                            >
                              <path
                                strokeLinecap="round"
                                strokeLinejoin="round"
                                d="M5 13l4 4L19 7"
                              />
                            </svg>
                          )}
                        </span>
                        <span
                          className={
                            sub.completed
                              ? "line-through text-muted-foreground"
                              : ""
                          }
                        >
                          {sub.title}
                        </span>
                      </li>
                    ))}
                  </ul>
                </section>
              )}

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
