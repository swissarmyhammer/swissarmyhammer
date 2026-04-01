---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffc680
title: Add tests for warn_performance_issues
---
performance.rs:340-364\n\nCoverage: 0% (25 lines uncovered)\n\nUncovered lines: 331-364\n\n```rust\nfn warn_performance_issues(&self, metrics: &ShellPerformanceMetrics)\n```\n\nLogs warnings when overhead >= 100ms, memory growth >= 50MB, or cleanup >= 1s. Test by calling finish_profiling with metrics that violate each threshold (use tracing-test or similar to capture log output). Also covers log_metrics (lines 327-337). #coverage-gap