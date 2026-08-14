---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzv7js101k3wtatk3543c7yt
  text: |-
    Closed on the direction of a person, 2026-08-12.

    This card carries no comment, no review finding, and progress 0.0. It moved to the review column on 2026-06-20 and nothing wrote to it after that date.

    There is no record on the board that the work was done. A person directed that it close.
  timestamp: 2026-08-12T14:56:18.464751+00:00
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffffffff280
title: Lease-based leadership takeover (^d8vae11)
---
What: Add lease/heartbeat/takeover to swissarmyhammer-leader-election so a stale-but-alive flock leader can be preempted. New lease.rs module, election.rs lease wiring, server.rs heartbeat+stepdown loop, subagent-gating policy seam. AC: lease tests RED->GREEN; election + workspace + server wired; nextest green (except known pre-existing failure); clippy clean on 3 crates. Tests: cargo nextest run -p swissarmyhammer-leader-election -p swissarmyhammer-code-context -p swissarmyhammer-tools