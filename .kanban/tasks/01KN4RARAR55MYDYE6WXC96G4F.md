---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffce80
title: test_warn_high_overhead uses thread::sleep(120ms) making it flaky in CI
---
**File**: `swissarmyhammer-shell/src/performance.rs` line 721\n**Layer**: Tests / Reliability\n**Severity**: Medium\n\n`test_warn_high_overhead` calls `std::thread::sleep(Duration::from_millis(120))` and then asserts the elapsed overhead is >= 100ms. On a heavily loaded CI machine, thread scheduling can cause the sleep to return earlier than expected (OS-level jitter) or the overhead measurement to be skewed. More critically, finish_profiling() recomputes overhead as `total_execution_time - command_execution_time`, so a short sleep causes a false negative.\n\nThe existing `profiler_with_injected_metrics` helper already demonstrates the right pattern: inject pre-computed values and assert on them. This test should inject `overhead_time` directly instead of relying on wall-clock sleep.\n\n**Recommendation**: Replace the sleep-based approach with direct field injection, the same pattern used by `test_warn_high_memory_growth` and `test_warn_slow_cleanup`. #review-finding