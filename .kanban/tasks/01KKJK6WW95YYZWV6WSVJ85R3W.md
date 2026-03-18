---
position_column: done
position_ordinal: ffffffa380
title: Re-election loop runs forever even after promotion — wasted CPU for leaders
---
swissarmyhammer-tools/src/mcp/server.rs:411-444\n\nThe re-election loop spawns a `tokio::spawn` that runs `loop { sleep(5s); ... }` indefinitely. Once a follower is promoted to leader, the loop body hits the `if ws_lock.is_leader() { continue; }` branch every 5 seconds for the rest of the process lifetime. This is minor overhead but wastes a mutex lock + mode check every 5 seconds forever.\n\nMore importantly, there is no shutdown/cancellation mechanism for either the re-election loop or the LSP health-check loop. When the server shuts down, these tasks are just abandoned. For a library crate this is at minimum a leak of tokio task handles with no way to join/cancel them.\n\nSuggestion: Use a `tokio::sync::watch` channel or a `CancellationToken` (from `tokio-util`) so both background loops can be stopped cleanly. Or at minimum, break out of the re-election loop once promotion succeeds — no further re-elections are needed.",
<parameter name="tags">["review-finding"] #review-finding