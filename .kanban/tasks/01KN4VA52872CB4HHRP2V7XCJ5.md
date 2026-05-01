---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffff8e80
title: TrackedStore::serialize returns String -- no error path for serialization failure
---
**swissarmyhammer-store/src/store.rs:35**\n\n```rust\nfn serialize(&self, item: &Self::Item) -> String;\n```\n\nSerialization can fail (e.g., a YAML serializer encountering an unsupported type). The return type is `String`, not `Result<String>`, so implementors must either panic or return garbage on failure. This is inconsistent with `deserialize` which returns `Result<Self::Item>`.\n\n**Severity: nit**\n\n**Suggestion:** Change the signature to `fn serialize(&self, item: &Self::Item) -> Result<String>` and propagate the error in `StoreHandle::write()`. This is a breaking change to the trait, so best done early.\n\n**Subtasks:**\n- [ ] Change `serialize` return type to `Result<String>`\n- [ ] Update `StoreHandle::write` to propagate the error\n- [ ] Update all `TrackedStore` implementations across the workspace\n- [ ] Verify compilation and tests" #review-finding