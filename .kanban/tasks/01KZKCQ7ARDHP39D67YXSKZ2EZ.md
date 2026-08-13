---
assignees:
- claude-code
position_column: todo
position_ordinal: ffac80
title: 'dart goes objective: complexity and function-length tool rules'
---
Dart is the only language with no complexity gate and no function-length gate. The prompt rules `cognitive-complexity` and `function-length` still apply to Dart files, so an LLM measures and decides.

Rust, Swift and TypeScript each get both gates from one `complexity-<lang>` rule that declares:

```yaml
supersedes:
  - cognitive-complexity
  - function-length
```

Dart needs the same shape, if a tool can do it.

## Survey first

`dart analyze` has no length lint and no complexity lint. Do not stop there. Enumerate the full Dart lint and metric tool space before you report a gap:

- every lint in the `lints` and `flutter_lints` packages
- `dart_code_metrics` — the open source releases, and what the move to DCM changed
- any analyzer plugin that reports cyclomatic complexity or lines per function

Record what you found in the card, tool by tool, with the version you tested.

## Then

If a tool exists, write `builtin/validators/code-hygiene/rules/complexity-dart.md` to the contract in `builtin/validators/README.md`:

- `match.files: ["**/*.dart"]`, `match.project_types: [flutter]`
- `supersedes: [cognitive-complexity, function-length]`
- a `tool.run` shell script that writes its own config to a temp path, never the project's `analysis_options.yaml`
- `doctor` and `install` blocks, with the tool version pinned
- a pass fixture and a fail fixture
- a measurement on a real Dart repository: finding count, run time, and whether every finding is true

If no tool exists, say so with the survey as evidence, and leave Dart on the prompt rules.

## Done when

Dart has both gates from a tool, or the card records why no tool can give them. #tool-validators #objectivity