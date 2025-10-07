Our MCP tools are allowed to call code, but are not allowed to shell our other programs.

In specific, we never invoke `sah` itself from an MCP tool.


## Alternative Approaches

- Call code in a swissarmyhammer- crate
- Find a crate that provides the needed functionality, such as ripgrep
