Virtual command shell with persistent history and process management used to run shell commands. Every command that exits stores its full output for later retrieval and grep.

`execute command` blocks until the command exits or the timeout kills it. There is no partial or streaming result. When the command exits, the response shows the last lines of the output, and the full output stays in the history. When the timeout kills the command, no output is stored — raise `timeout` and run the command again.

Rules:

- Do not pipe to `tail`, `head`, or `grep`. Read output with `get lines` or `grep history`.
- Do not use grep to search files. Use your file search tools. If you must, use `rg`.
- Do not use shell to edit files. Use your file editing tools.
