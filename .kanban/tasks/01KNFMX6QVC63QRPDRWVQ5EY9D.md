---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffe980
title: StoreContext missing Debug impl
---
swissarmyhammer-store/src/context.rs\n\n`StoreContext` is a public type that does not implement `Debug`. It holds `RwLock<UndoStack>`, `RwLock<Vec<Arc<dyn ErasedStore>>>`, and a `PathBuf`, all of which can produce useful debug output.\n\nSuggestion: Add a manual `Debug` impl that prints the root path, number of registered stores, and stack size. #review-finding