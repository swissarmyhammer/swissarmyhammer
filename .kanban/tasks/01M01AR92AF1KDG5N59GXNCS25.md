---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m02v5f95mz575d1r67bxh1ve
  text: |-
    NOT MOOTED. ^3r5bhpj decided the knip question and KEPT `ts-prune`, so this card stands as written and its fix is still owed.

    Worth recording before that work starts: the swap would NOT have answered this card either. Measured with knip 6.32.2 over `zod` at `4e1720c` with `node_modules` installed, replacing `packages/zod/tsconfig.json` with bytes that are not JSON — knip exits 1, the finding count holds at 17, and the JSON on stdout is BYTE-IDENTICAL to the healthy run. 174 bytes on stderr are the only difference. The shipped ts-prune rule has the same shape at exit 0 with its count held at 76.

    So a `tsconfig.json` a tool cannot read is silent under BOTH tools, and neither status nor stdout finds it. The fix this card asks for has to READ STDERR as well as the exit status. That is a firmer requirement than the card states today, and it is the same shape the three shipped swiftlint rules already carry, inverted: they read stderr to answer clean, this one must read stderr to break.

    The status table for ts-prune that this card asks to measure is still owed. `dead-code-typescript.md` now carries the knip status table beside it, which is a useful comparison when the ts-prune one is written.
  timestamp: 2026-08-15T13:53:15.045870+00:00
position_column: todo
position_ordinal: ffda80
title: dead-code-typescript answers zero findings when ts-prune crashes
---
`builtin/validators/code-hygiene/rules/dead-code-typescript.md` ends its per-project pipe in `sed` and the loop in `sort`, so the exit status of `ts-prune` is thrown away.

`ts-prune` 0.10.3 crashes with an unhandled error when it cannot read a `tsconfig.json`. Measured on a probe holding one dead export beside a `tsconfig.json` of bytes that are not JSON: `@ts-morph/common` throws, the stack goes to stderr, and the shipped script reports 0 findings and exits 0. The engine reads exit 0 as "the tool judged the code", so a project with a broken tsconfig reads as a clean workspace.

`builtin/validators/README.md` names this trap word for word: "Write a pipe only where the tool cannot exit nonzero. Otherwise write a script: run the tool into a file, test the status, and exit nonzero yourself."

The fix is the shape `complexity-swift` and `missing-docs-python` already carry: run `ts-prune` into a file, read its status, and exit nonzero with a line on stderr for the statuses that mean a broken run. Measure which statuses `ts-prune` answers with for a clean project, for a project holding findings, and for each broken shape, and state the table in the rule body. Ship an acceptance test beside the five in `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/dead_code_typescript.rs`.

Found while implementing ^108bh4y. #tool-validators #objectivity