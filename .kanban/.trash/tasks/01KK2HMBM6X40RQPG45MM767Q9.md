---
position_column: todo
position_ordinal: c9
title: Update ane-embedding Cargo.toml dependencies
---
Swap coreml-rs for objc2-core-ml ecosystem.\n\n- [ ] Remove `coreml-rs = \"0.5.4\"`\n- [ ] Remove `ndarray = \"0.15\"`\n- [ ] Add `objc2 = \"0.6\"`, `objc2-core-ml = \"0.3\"`, `objc2-foundation = \"0.3\"`, `block2 = \"0.6\"`\n- [ ] Keep `half` crate for f16 conversion\n- [ ] Verify version compatibility with existing objc2 in lockfile\n- [ ] cargo check -p ane-embedding