---
position_column: done
position_ordinal: j7
title: Fix 6 failing doctests in llama-agent/src/types/mcp.rs
---
6 doctests fail in types/mcp.rs: HttpServerConfig::sse_keep_alive_secs (line 244), stateful_mode (line 273), timeout_secs (line 216); ProcessServerConfig::args (line 69), command (line 46), timeout_secs (line 111). #test-failure