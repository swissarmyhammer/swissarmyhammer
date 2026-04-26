---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffb280
title: Percentile calculation in from_metrics is off-by-one for small sample sizes
---
**File**: `swissarmyhammer-shell/src/performance.rs` lines 445-449 (production code, exercised by new tests)\n**Layer**: Functionality / Correctness\n**Severity**: Low (not a test-only issue; the tests encode the current behavior)\n\nThe percentile calculation uses `(total_commands as f64 * 0.95) as usize` which truncates. For N=4, p95 index = `(4 * 0.95) as usize = 3`, which is the last element -- the same as the max. For N=10, p95 index = `(10 * 0.95) as usize = 9`, which is also the last element. Standard percentile computation (e.g., nearest-rank) would use `ceil(N * p) - 1` or interpolation. The current approach systematically over-estimates percentiles for small N by always selecting the maximum.\n\nThe new tests (`test_from_metrics_percentile_ordering`, `test_from_metrics_multiple_varied`) encode this behavior, which means fixing the production code later will require updating the tests too.\n\n**Recommendation**: Consider using `min(ceil(N * p), N) - 1` for nearest-rank, or document the current approach as intentional (conservative/upper-bound percentile). #review-finding