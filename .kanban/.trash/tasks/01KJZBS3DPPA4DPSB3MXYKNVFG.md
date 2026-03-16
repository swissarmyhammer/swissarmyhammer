---
position_column: done
position_ordinal: j4
title: Fix 6 failing doctests in llama-agent/src/acp/ (server.rs, mod.rs) - compile errors
---
6 doctests fail to compile in llama-agent/src/acp/server.rs (AcpServer::start_stdio line 128) and llama-agent/src/acp/mod.rs (line 129), plus agent.rs (load_session line 1512, should_compact_session line 1406, auto_compact_sessions line 1458, compact_session line 1357). All are compile failures. #test-failure