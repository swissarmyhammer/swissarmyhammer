---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffff9c80
title: '[warning] build_simple_args has silent catch-all that produces empty args'
---
**File**: code-context-cli/src/ops.rs (build_simple_args function)\n\n**What**: The `build_simple_args` function has a `_ => {}` arm that produces an empty `Map` with no `op` key. While lifecycle commands should never reach this path because `build_args` filters them first, the silent catch-all is a latent bug: if a new `Commands` variant is added and the developer forgets to update `build_args`, the new variant will silently produce an empty args map instead of erroring at compile time.\n\n**Why**: The compiler cannot warn about unmatched variants when a wildcard is present. This defeats exhaustive matching, one of Rust's strongest safety guarantees.\n\n**Suggestion**: Replace `_ => {}` with an explicit `unreachable!(\"lifecycle commands should not reach build_simple_args\")`, or better yet, restructure `build_args` to handle all operation variants exhaustively and remove the `_ =>` catch-all there too. The ideal fix: enumerate all remaining variants in `build_args` and remove `build_simple_args` entirely by inlining the simple ones.\n\n**Verify**: Add a new `Commands` variant temporarily and confirm the compiler forces you to handle it." #review-finding