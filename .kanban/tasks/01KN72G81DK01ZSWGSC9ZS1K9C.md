---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffff9880
title: 'NIT: Perspective public fields violate Rust future-proofing guideline'
---
swissarmyhammer-perspectives/src/types.rs:61-80\n\nAll fields on Perspective, PerspectiveFieldEntry, and SortEntry are public. The Rust review guidelines state: \"Private struct fields. Public fields are a permanent commitment. Use getters/setters.\"\n\nHowever, these types derive Serialize/Deserialize and are used as data transfer objects. Making fields private would require custom serde implementations or builder patterns for construction, which would be over-engineering for DTO types.\n\nSuggestion: Acceptable as-is for serde DTO types. The Perspective struct has `#[derive(Clone, PartialEq, Serialize, Deserialize)]` which is appropriate. No change needed.",
<parameter name="tags">["review-finding"] #review-finding