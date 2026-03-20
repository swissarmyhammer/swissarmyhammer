---
position_column: done
position_ordinal: ffe580
title: get_board_data fetches all tasks twice for ready computation
---
In `commands.rs` `get_board_data`, `task_is_ready` is called once per task in the counting loop (line 578), and then again in the summary section (line 640-641). The second pass re-iterates all_tasks for each task. The ready/blocked counts from the first loop could be summed to avoid the second pass.