import type { Task } from "@/types/kanban";

interface TaskCardProps {
  task: Task;
  isBlocked?: boolean;
  onClick?: (task: Task) => void;
}

export function TaskCard({ task, isBlocked, onClick }: TaskCardProps) {
  return (
    <div
      className={`rounded-md bg-card px-3 py-2 text-sm border border-border cursor-pointer hover:ring-1 hover:ring-ring transition-shadow ${
        isBlocked ? "opacity-50" : ""
      }`}
      onClick={() => onClick?.(task)}
    >
      <p className="leading-snug">{task.title}</p>
      {task.subtasks.length > 0 && (
        <p className="text-xs text-muted-foreground mt-1">
          {task.subtasks.filter((s) => s.completed).length}/{task.subtasks.length}
        </p>
      )}
    </div>
  );
}
