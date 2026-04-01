---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffcd80
title: SecurityStatistics::total_commands_analyzed is hardcoded to 0 -- test encodes the bug
---
**File**: `swissarmyhammer-shell/src/hardening.rs` line 481\n**Layer**: Functionality / Correctness\n**Severity**: Medium\n\nIn `ThreatDetector::get_security_statistics()`, `total_commands_analyzed` is hardcoded to 0 with a comment \"Command history tracking was removed as dead code\". The new test `test_get_security_statistics_fresh_detector` asserts `total_commands_analyzed == 0`, and `test_get_security_statistics_unique_commands` does not check this field at all (it only checks `unique_commands`).\n\nThis means the metric is permanently broken: after analyzing 3 commands, `total_commands_analyzed` still reports 0. The test cements the broken behavior rather than flagging it.\n\n**Recommendation**: Either remove the `total_commands_analyzed` field from `SecurityStatistics` since it cannot be populated, or track total commands (e.g., summing all `CommandFrequency::count` values) and update the test to verify the correct count. #review-finding