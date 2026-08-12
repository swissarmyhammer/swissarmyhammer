---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzteqxm6hvm3j6kvc7b8r0nm
  text: |-
    Archived. The `duplication-parsed` rule is removed — see ^wwb6hk7.

    The rule shelled out to the `sah` binary to reach a function already linked
    into the calling process. That mechanism is deleted, not repaired. The
    `duplication`, `rust` and `swift` prompt rules decide again, and they hold
    the carve-outs this card says the tool rule could not reach.
  timestamp: 2026-08-12T07:42:12.614506+00:00
position_column: todo
position_ordinal: ffc880
title: duplication-parsed cannot reach the semantic-intent and generic-dispatch carve-outs it supersedes
---
`builtin/validators/duplication/rules/duplication-parsed.md` runs `sah tool code_context duplication find` and declares `supersedes: [duplication, rust, swift]`.

Three carve-outs are unreachable.

- `duplication.md` carves out "Structurally similar but semantically distinct code that genuinely does different things (similar shape, different intent) — similarity of form is not duplication of behavior." The rule inverts it on purpose: "a record of six `String` fields matches every other record of six `String` fields, whoever wrote it and for whatever purpose. That is what the rule reports, on purpose." The type normalization drops every declared name and keeps only member types, which IS similarity of form. The 40-token floor and the 90% gate limit the blast radius; they do not state the carve-out.
- `rust.md` and `swift.md` both carve out "**Generic dispatch over a fixed set of distinct types**". The function normalization maps "The first distinct identifier becomes `v1`, the second `v2`", which is exactly what makes N per-type adapter functions normalize to one identical stream. Any such set over 40 tokens is reported, one finding for each copy.
- `duplication.md` carves out "Generated code, macro expansions, and vendored/third-party code." The tool has no generated-file, no vendored-directory and no path exclusion of any kind. Two generated serde or protobuf structs, or two files under a checked-in `vendor/` tree, are compared like any other definitions.

The test carve-out is partial and self-documented: "A test attribute that carries arguments — `#[tokio::test(flavor = "multi_thread")]` — is not read as a test marker", so eight functions in `swissarmyhammer-tools/src/mcp/tools/review/tests.rs` are reported. Bash and Fortran get no test exemption at all.

`// sah:allow duplication <reason>` is honoured and reaches past doc comments and attributes, so an annotation contract exists. Decide whether it is enough, or whether the normalizer keeps the type identity these carve-outs turn on.

Found by the `supersedes` survey on ^h7garpc. #tool-validators #objectivity