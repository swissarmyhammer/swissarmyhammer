---
assignees:
- claude-code
position_column: todo
position_ordinal: ffaf80
title: duplication test exclusion misses a test attribute that carries arguments
---
Found while measuring `^80nbway`.

`attribute_names` in `crates/swissarmyhammer-sem/src/parser/plugins/code/duplication.rs` compares the stripped attribute text against the marker for equality only. An attribute that carries arguments therefore names nothing:

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]

strips to `tokio::test(flavor = "multi_thread", worker_threads = 2)`, whose last path segment is `test(flavor = ...)`, which is not `test`. The function is not read as test code, so `duplication-parsed` reports it.

The plain `#[tokio::test]` form works. `#[cfg(test)]` works because the table lists the whole text `cfg(test)`.

## Measured

Over the 1183 tracked `.rs` files of this workspace the rule reports **416** findings. With the argument form read, it reports **408**. So the gap costs 8 wrong findings, all of them `#[tokio::test(...)]` functions in `crates/swissarmyhammer-tools/src/mcp/tools/review/tests.rs`.

The workspace holds 107 `tokio::test(` sites.

## The fix, already measured

Read an attribute as naming a marker when its text is the marker OR starts with the marker followed by `(`:

    fn names(text: &str, marker: &str) -> bool {
        text == marker || text.strip_prefix(marker).is_some_and(|rest| rest.starts_with('('))
    }

Apply it to the stripped text and to its last path segment, as `attribute_names` already does for equality. Measured: 416 -> 408.

## Acceptance

- `#[tokio::test(flavor = "multi_thread")]` marks the function it decorates as test code
- `#[cfg(test)]` keeps working
- The rule body's measured count is updated to the new number

#tool-validators