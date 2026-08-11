Virtual command shell with persistent history and process management used to run shell commands. Every command that exits stores its full output for later retrieval and grep.

`execute command` blocks until the command exits or the timeout kills it. There is no partial or streaming result. When the command exits, the response shows the last lines of the output, and the full output stays in the history. When the timeout kills the command, no output is stored — raise `timeout` and run the command again.

Do not pipe to `tail`, `head`, or `grep`. The tool keeps the full output of a command that exits, and a pipeline throws it away. Run the bare command. Then read the output with `get lines`, or search it with `grep history`. You can search the same output many times without a re-run.

Do not search files with this tool. To find text in files, use the `files` tool op `grep files` — it honors `.gitignore` and skips binary files. If you must run a shell search, use `rg`, never `grep -r`. `grep -r` does not read `.gitignore`, so it scans build directories such as `target/` and every binary artifact in them; `--include=*` cancels every exclusion; and a trailing `| grep -v ./target/` filters the output only after the scan has already paid to read it. One such command held a core for 22 minutes where `rg` answered in 0.044 seconds.
