---
assignees:
- assistant
position_column: done
position_ordinal: d8
title: Refactor LspJsonRpcClient to accept ChildStdin + ChildStdout
---
Change new() to take std::process::ChildStdin + ChildStdout instead of Child. Update all internal methods to use stored stdin/reader directly.