import { forwardRef } from "react";
import { GripVertical } from "lucide-react";
import { EditableMarkdown } from "@/components/editable-markdown";
import { SubtaskProgress } from "@/components/subtask-progress";
import type { Task } from "@/types/kanban";

interface TaskCardProps {
  task: Task;
  isBlocked?: boolean;
  onClick?: (task: Task) => void;
  onUpdateTitle?: (taskId: string, title: string) => void;
  dragHandleProps?: Record<string, unknown>;
  style?: React.CSSProperties;
}

export const TaskCard = forwardRef<HTMLDivElement, TaskCardProps>(
  function TaskCard({ task, isBlocked, onClick, onUpdateTitle, dragHandleProps, style, ...rest }, ref) {
    return (
      <div
        ref={ref}
        style={style}
        className={`rounded-md bg-card px-3 py-2 text-sm border border-border cursor-pointer hover:ring-1 hover:ring-ring transition-shadow flex items-start gap-2 ${
          isBlocked ? "opacity-50" : ""
        }`}
        onClick={() => onClick?.(task)}
        {...rest}
      >
        <button
          type="button"
          className="shrink-0 mt-0.5 p-0 text-muted-foreground/50 hover:text-muted-foreground cursor-grab active:cursor-grabbing touch-none"
          onClick={(e) => e.stopPropagation()}
          {...dragHandleProps}
        >
          <GripVertical className="h-4 w-4" />
        </button>
        <div
          className="flex-1 min-w-0"
          onClick={(e) => e.stopPropagation()}
          onDoubleClick={(e) => {
            e.stopPropagation();
            // Blur the CodeMirror editor so it commits and returns to display mode
            if (document.activeElement instanceof HTMLElement) {
              document.activeElement.blur();
            }
            onClick?.(task);
          }}
        >
          <EditableMarkdown
            value={task.title}
            onCommit={(title) => onUpdateTitle?.(task.id, title)}
            className="leading-snug"
            inputClassName="leading-snug bg-transparent border-b border-ring w-full"
          />
          <SubtaskProgress description={task.description} className="mt-1.5" />
        </div>
      </div>
    );
  }
);
