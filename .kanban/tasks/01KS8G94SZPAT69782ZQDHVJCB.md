---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffa380
title: Fix elicitation response never returning to agent (block_task in raw spawn)
---
Localize and fix the elicitation round-trip failure: the agent sends elicitation/create but the user's answer never returns. Step 1: write a Rust reproduction test mimicking run_stream_loop's raw tokio::spawn context. Step 2: fix the broken leg. Step 3: add diagnostic logging on both sides.