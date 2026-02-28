export interface Column {
  id: string;
  name: string;
  order: number;
  task_count?: number;
  ready_count?: number;
}

export interface Swimlane {
  id: string;
  name: string;
  order: number;
  task_count?: number;
}

export interface Tag {
  id: string;
  name: string;
  description?: string;
  color: string;
  task_count?: number;
}

export interface BoardSummary {
  total_tasks: number;
  total_actors: number;
  ready_tasks: number;
  blocked_tasks: number;
}

export interface Board {
  name: string;
  description?: string;
  columns: Column[];
  swimlanes: Swimlane[];
  tags: Tag[];
  summary?: BoardSummary;
}

export interface Position {
  column: string;
  swimlane?: string;
  ordinal: string;
}

export interface Task {
  id: string;
  title: string;
  description?: string;
  position: Position;
  tags: string[];
  assignees: string[];
  depends_on: string[];
  progress?: number;
  created_at: string;
  updated_at: string;
}

export interface OpenBoard {
  path: string;
  is_active: boolean;
}

export interface RecentBoard {
  path: string;
  name: string;
  last_opened: string;
}
