---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffc480
title: Add tests for set_timeout_info and record_cleanup_time
---
performance.rs:205-216\n\nCoverage: 0% (6 lines uncovered)\n\nUncovered lines: 205-207, 212-214\n\n```rust\npub fn set_timeout_info(&mut self, timed_out: bool)\npub fn record_cleanup_time(&mut self, cleanup_time: Duration)\n```\n\nSimple setters on current_metrics. Test by starting profiling, calling these methods, then finishing and verifying the returned metrics contain the set values. #coverage-gap