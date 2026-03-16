import { RadialBarChart, RadialBar, PolarAngleAxis } from "recharts";
import type { BoardData } from "@/types/kanban";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";

interface BoardProgressProps {
  board: BoardData;
}

/**
 * Radial progress ring for the navbar.
 * Reads done_tasks and percent_complete from the backend-computed board summary.
 */
export function BoardProgress({ board }: BoardProgressProps) {
  const { done_tasks, total_tasks, percent_complete } = board.summary;

  if (total_tasks === 0) return null;

  const data = [{ value: percent_complete, fill: "var(--color-chart-2)" }];

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
            {percent_complete}%
          </span>
        </div>
      </TooltipTrigger>
      <TooltipContent side="bottom">
        <p className="text-xs">
          {done_tasks}/{total_tasks} tasks done ({percent_complete}%)
        </p>
      </TooltipContent>
    </Tooltip>
  );
}
