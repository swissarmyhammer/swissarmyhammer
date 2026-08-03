Virtual command shell with persistent history and process management used to run shell commands. Every command that exits stores its full output for later retrieval and grep.

`execute command` blocks until the command exits or the timeout kills it. There is no partial or streaming result. When the command exits, the response shows the last lines of the output, and the full output stays in the history. When the timeout kills the command, no output is stored — raise `timeout` and run the command again.

Do not pipe to `tail`, `head`, or `grep`. The tool keeps the full output of a command that exits, and a pipeline throws it away. Run the bare command. Then read the output with `get lines`, or search it with `grep history`. You can search the same output many times without a re-run.
