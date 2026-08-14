---
assignees:
- claude-code
position_column: todo
position_ordinal: ffdc80
title: Remove the second source of the permissions.deny JSON pointer in strategy/mod.rs
---
`crates/mirdan/src/strategy/mod.rs:35` declares:

    const PERMISSIONS_DENY_POINTER: &str = "/permissions/deny";

`crates/mirdan/src/install/profile.rs` declared the same const with the same
value until card `^4kzxdex` round 4 replaced it with a function that builds the
pointer from `POINTER_KEY_PERMISSIONS` and `POINTER_KEY_DENY`.

The pointer names the Claude Code settings schema. It is one external contract
with two sources in one crate. A change to the schema must reach both, and
nothing makes that happen.

## What to build

Give the crate one source of the two keys and one source of the pointer built
from them, and let `strategy/mod.rs` read it. `permissions_deny_pointer`,
`POINTER_KEY_PERMISSIONS` and `POINTER_KEY_DENY` are private to
`install::profile` today, so this needs a decision on where the single source
lives (probably `settings.rs` or a small module both can read), not only a
visibility change.

## Done when

- One declaration of the pointer in the crate.
- `strategy/mod.rs:194` and `strategy/mod.rs:213` read it.
- A test fails when either key changes but the pointer does not.

## How this was found

A substring detector (every `const` VALUE against every string literal, not
whole-literal equality) run for card `^4kzxdex`. Whole-literal sweeps in
earlier rounds could not see it.

#tool-validators