---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffca80
title: StoreHandle.store field is pub(crate) -- should be private with accessor
---
swissarmyhammer-store/src/handle.rs\n\n`StoreHandle` has `pub(crate) store: Arc<S>`. The erased.rs blanket impl accesses `self.store` directly. This is a minor coupling concern but acceptable for crate-internal use.\n\nHowever, the field is exposed to the entire crate, meaning any future module added to the crate could directly access the inner store. A private field with a `store()` accessor would be cleaner.\n\nSuggestion: Make `store` private, add `pub(crate) fn store(&self) -> &S` accessor, and update erased.rs. #review-finding