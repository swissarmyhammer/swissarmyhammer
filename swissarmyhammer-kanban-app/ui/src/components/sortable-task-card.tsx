import { useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { TaskCard } from "@/components/task-card";
import type { Tag, Task } from "@/types/kanban";

interface SortableTaskCardProps {
  task: Task;
  tags?: Tag[];
  isBlocked?: boolean;
  onClick?: (task: Task) => void;
  onUpdateTitle?: (taskId: string, title: string) => void;
}

export function SortableTaskCard({ task, tags, isBlocked, onClick, onUpdateTitle }: SortableTaskCardProps) {
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: task.id, data: { type: "task" } });

  const style: React.CSSProperties = {
    transform: CSS.Translate.toString(transform),
    transition,
    opacity: isDragging ? 0.3 : undefined,
  };

  return (
    <TaskCard
      ref={setNodeRef}
      style={style}
      task={task}
      tags={tags}
      isBlocked={isBlocked}
      onClick={onClick}
      onUpdateTitle={onUpdateTitle}
      dragHandleProps={{ ...listeners, ...attributes }}
    />
  );
}
