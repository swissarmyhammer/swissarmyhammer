---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kztjcpt71kvd15r699vt0t6z
  text: |-
    Research done. Read the current `builtin/validators/code-hygiene/rules/dead-code-rust.md` after f495f760c (^y4xyw1g), which rewrote the CARGO half to write `cargo check` into a file and test the raw report beside the status. That half stays untouched; this card changes the ORPHAN-MODULE half alone.

    Plan for the three defects:

    1. `include!("foo.rs")` — the index reads `include!` string paths, the same way it already reads `#[path = "..."]`. A file a literal `include!` names is compiled, so it is no orphan. Plus a suppression marker for the residue the scan cannot see (a `mod` a macro builds, an `include!(concat!(env!("OUT_DIR"), ...))`, a file a build script compiles): `// sah:ignore orphan-module <reason>` in the file itself, with a reason required. This follows `builtin/validators/README.md`: "An exemption a person would argue for in prose must become an inline suppression the tool reads."

    2. `mod` in a comment or a string — the index is no longer built by a bare `grep`. One `awk` pass lexes each `.rs` file: it drops line comments and block comments, and it keeps string contents out of the text the `mod` scan reads (raw strings and char literals included). `#[path]` and `include!` read the text WITH strings, because their payload IS a string.

    3. `mod x;` inside `#[cfg(test)] mod tests` — the same `awk` pass tracks inline `mod` nesting. A `mod NAME` at the top level indexes the bare STEM, as today. A `mod NAME` nested inside one or more inline modules indexes the RESOLVED PATH instead. So `#[cfg(test)] mod tests { mod orphan; }` in `src/lib.rs` names `src/tests/orphan.rs` and no longer excuses `src/orphan.rs`. This needs no `cfg(test)` special case, and it fixes the defect in BOTH directions: the file the nested declaration really names stays exempt.
  timestamp: 2026-08-12T08:45:59.495321+00:00
- actor: claude-code
  id: 01kztk83hmpezrxwan1m73vmy2
  text: |-
    Implementation landed. Seven acceptance tests were written FIRST and each behaviour change was watched RED, then GREEN, against the real shipped script.

    RED measurements over the earlier `grep` index (cargo 1.97.1):
    - `include!("generated.rs")` beside `src/generated.rs` → reported `src/generated.rs:1` for a file the compiler reads.
    - `// sah:ignore orphan-module <reason>` in the orphan file → reported `src/orphan.rs:1`; there was no suppression at all.
    - `mod orphan;` in a line comment, in a block comment at column zero, and in a `pub const` string → 0 findings for each, so the real orphan was hidden three ways.
    - `#[cfg(test)] mod tests { mod orphan; }` beside a real `src/orphan.rs` → 0 findings; the nested declaration's bare stem excused the wrong file.

    GREEN: all 7 report exactly what the probe names. The whole crate suite is 687/687, `cargo fmt --check` and `cargo clippy --workspace --all-targets -- -D warnings` are clean.

    Controls that hold the new gate honest:
    - `..._reports_a_file_no_declaration_names` — a scan that stopped reporting would pass every other probe.
    - `..._reports_an_orphan_whose_marker_states_no_reason` — the reason is required, so a marker with nothing after it suppresses nothing.
    - The nested probe stages `src/tests/orphan.rs` too, so the fix is measured in BOTH directions: the file the nested declaration really names stays exempt.

    Whole-workspace measurement, 1219 `.rs` files, lowest of three runs of each: the `awk` index takes 11.3 s against 7.9 s for the earlier `grep` index. Both report 0 orphan files, so the new index adds no false positive here. The shipped script run end to end over this repository answers exit 0, 0 findings, empty stderr, in 20.1 s.

    Notes for the next agent:
    - The awk program carries no `'` character, because the whole program stands inside a single-quoted shell string. The apostrophe it needs for a character literal comes from `quote = sprintf("%c", 39)`, and `plain_pattern` is built from that variable rather than written as a regex literal. An earlier draft wrote `/^[^"'\/rb]+/` as a literal and broke the shell quoting of the whole `run:` block.
    - First draft of the lexer stepped one character at a time and cost 33.9 s over the workspace. The run-skipping `match(substr(...), /^[^...]+/)` fast paths in each state brought it to 11.3 s.
    - `doctor.check_command` dropped `sed`, which the script no longer runs.
    - The cargo half from f495f760c (^y4xyw1g) is untouched; its six acceptance tests still pass.

    ### implement — changed
    - evidence: 2 files — builtin/validators/code-hygiene/rules/dead-code-rust.md, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/dead_code_rust.rs; 13/13 rule tests, 687/687 crate tests, fmt and clippy clean
    - next: /review
  timestamp: 2026-08-12T09:00:57.268808+00:00
position_column: doing
position_ordinal: '8280'
title: dead-code-rust reports an include! file as an orphan module, and offers no suppression
---
`builtin/validators/code-hygiene/rules/dead-code-rust.md` runs `cargo check --workspace --all-targets` for `dead_code`, and adds a shell orphan-module scan.

The `dead_code` half reproduces every carve-out of `dead-code.md`, because rustc makes them: a reachable `pub` item, `main`, `#[unsafe(no_mangle)]`, `extern "C"`, and `#[cfg(test)]` code are all exempt.

The orphan-module half makes a finding class the prompt rule does not. The index comes from `grep -rhoE '\bmod[[:space:]]+[A-Za-z_][A-Za-z0-9_]*'` plus `#[path = "..."]`. A file compiled through `include!("foo.rs")` is named by no `mod` declaration and no `#[path]`, so the scan reports "orphan module: no 'mod X;' declaration in this crate names this file, so nothing compiles it". That claim is false for such a file, and the rule states there is no suppression for it.

The lesser defect in the other direction: the grep also matches `mod` inside a comment or a string literal, and matches a `mod x;` that stands inside a `#[cfg(test)] mod tests`, which hides a real orphan.

Decide how the scan reads an `include!`, or give the scan a suppression marker.

Found by the `supersedes` survey on ^h7garpc. #tool-validators #objectivity