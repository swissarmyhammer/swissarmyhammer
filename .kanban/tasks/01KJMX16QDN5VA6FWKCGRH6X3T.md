---
title: 'Fix compilation error: TagId: From<&String> not implemented in integration_tag_storage.rs:178'
position:
  column: done
  ordinal: b9
---
The integration test `integration_tag_storage.rs` fails to compile at line 178:

```rust
.process(&UpdateTag::new(&tag_id).with_name("new-name"), &ctx)
```

`UpdateTag::new` requires `impl Into<TagId>`, but `&String` does not implement `Into<TagId>`. Fix by dereferencing to `&str` (e.g., `&*tag_id` or `tag_id.as_str()`).

#fix_it