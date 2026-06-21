---
assignees:
- claude-code
position_column: review
position_ordinal: '80'
title: Lease-based leadership takeover (^d8vae11)
---
What: Add lease/heartbeat/takeover to swissarmyhammer-leader-election so a stale-but-alive flock leader can be preempted. New lease.rs module, election.rs lease wiring, server.rs heartbeat+stepdown loop, subagent-gating policy seam. AC: lease tests RED->GREEN; election + workspace + server wired; nextest green (except known pre-existing failure); clippy clean on 3 crates. Tests: cargo nextest run -p swissarmyhammer-leader-election -p swissarmyhammer-code-context -p swissarmyhammer-tools