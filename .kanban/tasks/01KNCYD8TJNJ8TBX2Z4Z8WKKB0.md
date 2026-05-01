---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffcc80
title: 'NIT: load_diffs_from_sidecar always sets is_new_file: false'
---
avp-common/src/chain/links/validator_executor.rs:90-94\n\nWhen reconstructing FileDiff from sidecar files, `is_new_file` is hardcoded to `false` with the comment 'Conservative default; diff text itself shows this.' The diff text does indeed contain `--- /dev/null` for new files, but any code that checks `diff.is_new_file` directly (without parsing the text) will get the wrong answer.\n\nThis is marked as a nit because the current code does not appear to branch on `is_new_file` for Stop hook validators. However, it is a latent inconsistency.\n\nSuggestion: Parse the first line of `diff_text` for `/dev/null` to set the flag correctly, or document on `FileDiff` that `is_new_file` is unreliable when reconstructed from sidecar." #review-finding