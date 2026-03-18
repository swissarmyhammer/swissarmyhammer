---
position_column: done
position_ordinal: ffffffffe880
title: AvpElicitationResultOutput missing deny_from_validator constructor
---
`avp-common/src/types/avp_output.rs` — AvpElicitationResultOutput

AvpElicitationOutput has `deny_from_validator()` but AvpElicitationResultOutput, AvpConfigChangeOutput, AvpTeammateIdleOutput, and AvpTaskCompletedOutput only have `block()`/`allow()` — they're missing `block_from_validator()` constructors.

The existing blockable types (AvpStopOutput, AvpSubagentStopOutput) all have `block_from_validator()`. The new blockable types should follow the same pattern for consistency and so validators can properly attribute their blocks.

**Severity**: warning #review-finding