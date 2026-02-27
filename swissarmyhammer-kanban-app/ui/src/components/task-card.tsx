import type { Task } from "@/types/kanban";

interface TaskCardProps {
  task: Task;
  isBlocked?: boolean;
}

export function TaskCard({ task, isBlocked }: TaskCardProps) {
  return (
    <div
      className={`rounded-md bg-card px-3 py-2 text-sm shadow-sm ${
        isBlocked ? "opacity-50" : ""
      }`}
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
