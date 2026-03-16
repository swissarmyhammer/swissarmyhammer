import { useMemo } from "react";
import { RadialBarChart, RadialBar, PolarAngleAxis } from "recharts";
import type { BoardData } from "@/types/kanban";
import { getNum } from "@/types/kanban";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";

interface BoardProgressProps {
  board: BoardData;
}

/**
 * Tiny radial progress ring for the navbar.
 * Shows done tasks (last column) / total tasks as a percentage.
 */
export function BoardProgress({ board }: BoardProgressProps) {
  const { done, total, pct } = useMemo(() => {
    const totalTasks = board.summary.total_tasks;
    if (totalTasks === 0) return { done: 0, total: 0, pct: 0 };

    // Last column is the terminal/done column
    const lastCol = board.columns[board.columns.length - 1];
    const doneTasks = lastCol ? getNum(lastCol, "task_count", 0) : 0;
    const percent = Math.round((doneTasks / totalTasks) * 100);

    return { done: doneTasks, total: totalTasks, pct: percent };
  }, [board]);

  if (total === 0) return null;

  const data = [{ value: pct, fill: "var(--color-chart-2)" }];

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <div className="flex items-center gap-1.5 cursor-default">
          <div className="relative flex items-center justify-center w-7 h-7">
            <RadialBarChart
              width={28}
              height={28}
              cx={14}
              cy={14}
              innerRadius={9}
              outerRadius={13}
              barSize={4}
              data={data}
              startAngle={90}
              endAngle={-270}
            >
              <PolarAngleAxis
                type="number"
                domain={[0, 100]}
                angleAxisId={0}
                tick={false}
              />
              <RadialBar
                background={{ fill: "var(--color-muted)" }}
                dataKey="value"
                cornerRadius={2}
                angleAxisId={0}
              />
            </RadialBarChart>
          </div>
          <span className="text-xs text-muted-foreground tabular-nums">
            {pct}%
          </span>
        </div>
      </TooltipTrigger>
      <TooltipContent side="bottom">
        <p className="text-xs">
          {done}/{total} tasks done ({pct}%)
        </p>
      </TooltipContent>
    </Tooltip>
  );
}
