---
assignees:
- claude-code
position_column: todo
position_ordinal: ffdd80
title: A rule's run block is a shell script that nothing checks
---
**This card was originally written as "no validator reads a .md file, so rule prose is never reviewed." That framing was wrong and is retracted.** Running `duplication`, `naming` or `rust/api-design` over Markdown prose would be noise. Prose does not need reviewing.

The real gap is narrower and it is not about prose at all.

## The run block is executable code

Every tool rule carries a `tool.run` shell script. It ships, it runs on every review, and its exit status decides whether a file reads as clean. It is code — it just lives inside a `.md` file, so no validator matches it and nothing checks it.

`builtin/validators/README.md` already states the contract that script must meet, at lines 144-148 and 258-267:

> A shell pipeline takes the exit status of its LAST command, so a pipe that ends in `jq` or `awk` throws away the tool's own status. Write a pipe only where the tool cannot exit nonzero. Otherwise write a script: run the tool into a file, test the status, and exit nonzero yourself.
>
> One status can carry both a measured run and a broken run... The script must then test the REPORT beside the status.

**Nothing enforces any of it.**

## The cost, measured over one session

Eight defects of exactly this class, every one inside a `run` block, every one found by hand:

| card | the defect |
| --- | --- |
| `^y4xyw1g` | three cargo pipelines exited 0 for a crate that did not compile |
| `^mms9g8d` | a golangci-lint lock clash and a shared cache gave 0 findings, and the generated-code filter failed OPEN |
| `^kmxvk6r` | ruff `invalid-syntax` rows were reported as function-length findings |
| `^108bh4y` | a project's own `ts-prune` config silenced the entire gate |
| `^2vxg70a` | `dart pub get` discarded both streams and its exit status |
| `^gxncs25` | ts-prune crashes and the rule answers zero findings — OPEN |
| `^d3j6sbt` | ruff exits 0 with a warning for an absent or unreadable file — OPEN |
| `^hc2pcyp` | a stale Dart SDK floor zeroed a 3508-file run, across two rules — OPEN |

Every one reads as a clean tree when the tool actually failed. That is the worst failure mode a validator has, and it is a single repeating pattern in a single kind of file.

## The mechanism already exists

`crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/` already holds structural guards that read the SHIPPED rule bytes and assert properties across every rule: `zero_argument.rs` requires a zero-argument guard on every files-scope rule and vets the shell lines above it; `temp_directory.rs` and `scope_roster.rs` do the same for other properties.

So the answer is not a new validator over Markdown. It is extending a guard family that already works, plus a real tool for the syntax-level half.

## What to do

1. **Extract each rule's `run` block and run `shellcheck` over it.** A deterministic tool beats judgment, which is this project's whole direction. It catches the shell-level half — masked pipeline status, unquoted expansion, `$?` read indirectly. Note `shellcheck` is NOT currently installed; add it to doctor and install if adopted.

2. **Extend the shipped-bytes guards for what shellcheck cannot know.** These are contract facts, not shell syntax:
   - a pipeline whose last command is `jq`, `awk` or `sort` where the tool can exit nonzero
   - a tool invocation whose status is never tested
   - a command whose stderr goes to `/dev/null` without its status being read
   - the README's own "test the REPORT beside the status" requirement, for tools that share one status between a finding and a failure

3. **Judge what is worth enforcing mechanically and what is not.** Some of the eight above are domain knowledge — that `ruff` exits 0 on an unreadable file is a fact about ruff, not a shell defect. Say plainly which of the eight a guard would have caught and which it would not; that is the honest measure of this card's value.

## Done when

- Every shipped `run` block is checked by something, and a rule that discards a tool's failure status fails a test.
- The check states which of the eight known instances it would have caught.

#tool-validators