---
name: manifests
description: >-
  Flag defects in changed dependency manifests — a dependency a package
  declares and no source of that package ever names.
metadata:
  version: "{{version}}"
match:
  files:
    - "**/Cargo.toml"
---

# Manifests

A manifest is not source code, and the questions it answers are its own. A
dependency is declared in the manifest and used in the source, so "is this
dependency used" is a question about the pair — and the finding lands on the
manifest, because the manifest is the file that changes.

`code-hygiene` matches `@file_groups/source_code`, which declares no manifest
pattern, so it can neither match a manifest nor report a finding in one. That
is why this set exists.

## The set fires only when a manifest changed

Every rule here inherits the set's `match`, so the set is silent until a
changed file is a manifest. That is the correct trigger for a dependency
question: a source file that stops using a dependency and a manifest that keeps
declaring it are one change, and the manifest is the half a reviewer reads.

## What the set matches today, and where it grows

`**/Cargo.toml`, and that one pattern reaches every Cargo manifest in a
repository. The engine compiles its file patterns with the glob crate under
`require_literal_separator: false`, so a leading `**/` matches no directory as
readily as several: a single-crate repository's own `Cargo.toml` and a
workspace member's `crates/<name>/Cargo.toml` both match. Measured — the pattern
was removed and the set stopped matching the member, and a bare `Cargo.toml`
literal was tried alone and matched only the root.

`**/package.json`, `pyproject.toml`, `go.mod` and their siblings belong here
when a rule arrives that reads them. Add the pattern with the rule, never
before it: a set that matches a file no rule reads makes the engine plan work
that reports nothing.

## Rules

| Rule | Tool | Inline suppression |
|---|---|---|
| `unused-dependencies-rust` | `cargo machete` | `[package.metadata.cargo-machete] ignored = [...]` |

`unused-dependencies-rust` supersedes nothing. No prompt rule in any shipped
set asks whether a declared dependency is used, so there is no prompt rule to
replace and no fallback to degrade to. A workspace whose machine has no
`cargo machete` simply gets no answer to the question, which is where every
workspace stood before this rule.

## Reversed decisions

- The **`cargo machete`** rejection recorded in
  `builtin/validators/code-hygiene/VALIDATOR.md` is reversed. It rested on two
  legs. The first was scope — every machete finding names a `Cargo.toml`, and
  that set matches source code — and this set answers it. The second was that
  machete misreports in `--with-metadata`, its metadata-resolving mode, calling
  `tauri-build` unused for two apps whose build scripts call
  `tauri_build::build()`. This rule does not run that mode, and the rule file
  records the measurement that says so.
