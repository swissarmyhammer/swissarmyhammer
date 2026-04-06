---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffbe80
title: StoreHandle missing Debug impl
---
swissarmyhammer-store/src/handle.rs\n\n`StoreHandle<S>` does not implement `Debug`. The Rust review guidelines require all public types to implement `Debug`. The struct contains `Arc<S>` and `RwLock<Vec<ChangeEvent>>`, both of which are Debug-friendly when `S: Debug`.\n\nSuggestion: Add `impl<S: TrackedStore + Debug> Debug for StoreHandle<S>` or a manual impl that prints the store name and pending event count. #review-finding