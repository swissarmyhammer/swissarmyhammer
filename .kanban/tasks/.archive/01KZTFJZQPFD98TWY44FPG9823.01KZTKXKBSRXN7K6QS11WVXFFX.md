---
assignees:
- claude-code
position_column: todo
position_ordinal: ffd780
title: the_swift_package_root_restores_the_directory_before_it_removes_it fails when the suite runs in parallel
---
`review::tool_rules::tests::shipped::the_swift_package_root_restores_the_directory_before_it_removes_it` passes when it runs alone and fails when the whole `swissarmyhammer-validators` suite runs.

Measured 2026-08-12, on this branch AND on the tree with no change of mine applied, so it is not caused by ^y4xyw1g:

```
assertion `left == right` failed: the first element must restore the working directory;
instead the working directory was removed while the process still stood in it
  left: Some("/Users/wballard/github/swissarmyhammer/swissarmyhammer/crates/swissarmyhammer-validators")
 right: Some("/private/var/folders/.../T/.tmpXT5KiR")
```

`cargo test -p swissarmyhammer-validators --lib the_swift_package_root_restores_the_directory_before_it_removes_it` → ok. The same test inside `cargo test -p swissarmyhammer-validators` → failed, in four runs out of four.

The working directory is process state, so a test that reads it answers for whatever another test set. The shipped-rule tests hold `CurrentDirGuard` for the Swift package root, and nothing holds the other tests off that state.

#tool-validators</description>
<parameter name="tags">["tool-validators"]