---
assignees:
- claude-code
position_column: todo
position_ordinal: '80'
project: ai-panel
title: All tools must resolve CWD from the session working directory (the board dir), never std::env::current_dir(); grep must honor .gitignore
---
## P0 — root cause of "a simple grep hung forever" in the AI panel

### Binding requirement (per user, previously specified)
**The working directory of a board IS the working directory of its agent session.** Every tool MUST operate rooted at that **session working directory**. The app process CWD is irrelevant (and is `/` for the bundled GUI app). The board is not in `/` — comply with the board-dir-as-session-CWD contract.

### What happened (evidence — session 01KT9QXP…, pid 40625)
The qwen agent issued `grep_files {pattern:"builtin.*partial", output_mode:"content", case_insensitive:true}` with **no `path`** at 11:38:53. The handler never returned. Exactly 300s later (11:43:53) the MCP stream tore down (`serve finished quit_reason=Closed`); the agent then looped on `GET /mcp → 404` forever. That hang is the "forever" the user saw.

### Root cause (two compounding faults)
`crates/swissarmyhammer-tools/src/mcp/tools/files/grep/mod.rs::execute_grep`:
1. **Uses process CWD instead of the session working dir.** With `request.path == None` (~line 176) it falls back to `std::env::current_dir()`. For the bundled GUI app that is **`/`** (confirmed live: `lsof -p 40625 -d cwd` → `/`; memory `gui-cwd-readonly`). So it rooted the search at the filesystem root even though the board dir was known.
2. **Raw, un-ignored, unbounded walker.** Lines ~217-252 use a plain `WalkDir::new(search_dir)` with **no `.gitignore` honoring, no hidden/`target`/`.git` skip, no depth bound**. Rooted at `/`, it walks the entire Mac filesystem, regex-matching every file → effectively never finishes.

`send_mcp_log` (tool_registry.rs:1963) is fire-and-forget and is NOT the blocker. The `Failed to forward MCP notification: channel closed` warn is benign (broadcast, no subscribers).

### The session working dir is already known — just not applied
- The MCP server is constructed **with** a working dir: `McpServer::new_with_work_dir(PromptLibrary, working_dir, …)` — `apps/swissarmyhammer-cli/src/mcp_integration.rs:48`.
- The in-process board server already knows the board root: log line 4 → `started in-process MCP server for board board=/Users/wballard/github/swissarmyhammer/swissarmyhammer` (the dir **containing** `.kanban`).
- Gap: that working dir is neither threaded into `ToolContext` nor used to set the server's CWD per session.

### Two viable approaches (do BOTH where each applies)
**A. Thread session work_dir through `ToolContext` (required for the in-process multi-board server).** Tool handlers resolve their default root from `context.work_dir`, never `std::env::current_dir()`. This is the ONLY correct option for the shared in-process server, because **process CWD is global** and one app process hosts multiple boards/sessions — you cannot set a per-session CWD there.

**B. Start the MCP tool server with the session's CWD set (correct for per-process servers).** For servers that run one-process-per-session — `kanban-cli serve`, `sah` CLI — `std::chdir` to the board dir at startup so even `current_dir()` resolves correctly. Clean, but does NOT generalize to the in-process multi-board host (approach A still needed there).

### Required regardless of approach
- **Ban `std::env::current_dir()` in tool handlers** — add a lint/test guard; resolve root from the session working dir.
- grep **must honor `.gitignore`** — replace raw `WalkDir` with `ignore::WalkBuilder` (respects `.gitignore`/`.ignore`, skips hidden + `target`/`.git`, supports bounded depth), matching ripgrep semantics.
- Apply the same root-resolution fix to **all** file tools: `files`, `glob_files`, `read_file`, shell.
- Defensive guard: refuse / hard-anchor any unscoped search that would resolve to `/`.

### Verification (real-path)
- Process CWD = `/`, session work_dir = a repo: `grep_files` with `path:None` searches the repo, returns promptly, and skips `target/`, `.git/`, and `.gitignore`d paths.
- Guard test: tool handlers contain no `std::env::current_dir()`.