---
position_column: done
position_ordinal: t6
title: Fix borrow checker errors in ane-embedding/src/model.rs (E0502)
---
Compilation of ane-embedding fails with 3 E0502 errors in model.rs lines 185-223. A mutable borrow of `inner` (via `inner.model.as_mut()`) at line 185 conflicts with immutable borrows of `inner.tokenizer` (line 189), `inner.max_length` (line 201), and `inner.max_length` again (line 204). The fix is to read the immutable fields (tokenizer ref, max_length) before taking the mutable borrow of model, or to restructure the borrows so they do not overlap. #test-failure