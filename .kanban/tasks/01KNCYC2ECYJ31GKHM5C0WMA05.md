---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffcf80
title: 'NIT: matches_changed_files takes Option<&Vec<String>> instead of Option<&[String]>'
---
avp-common/src/validator/types.rs:469-471\n\nThe parameter type `Option<&Vec<String>>` should be `Option<&[String]>` per Rust API guidelines (accept &[T] not &Vec<T>). The Vec reference works but is unnecessarily specific.\n\nSuggestion: Change the signature to `changed_files: Option<&[String]>` and update the call sites to use `.as_deref()` instead of `.as_ref()`." #review-finding