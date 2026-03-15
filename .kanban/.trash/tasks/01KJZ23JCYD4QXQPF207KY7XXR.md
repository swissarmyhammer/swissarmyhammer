---
position_column: done
position_ordinal: i4
title: Add Deserialize to ModelMetadata or remove manual Serialize
---
**model-loader/src/types.rs**

`ModelMetadata` has a manual `Serialize` impl but no `Deserialize`. This asymmetry means metadata can be written but not read back. Either add `Deserialize` or derive both.

- [ ] Add `Deserialize` impl (manual or derived) for `ModelMetadata`
- [ ] Or derive both `Serialize` and `Deserialize` and remove the manual impl
- [ ] Verify tests pass #review-finding