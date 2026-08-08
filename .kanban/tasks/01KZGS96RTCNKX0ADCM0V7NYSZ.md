---
assignees:
- claude-code
position_column: todo
position_ordinal: ffa780
title: 'dead-code goes objective: tool rules supersede the prompt rule; staged work must be annotated'
---
## STANDING ORDER

This card is a decision, not a proposal. Your job is to implement it exactly.
- Do NOT re-evaluate the tool choices. The evaluation happened. It is recorded here and reversed on ^teemmch.
- Do NOT preserve the prompt rule's judgment. Supersede it as stated.
- Do NOT reject a tool because a zero-config run is noisy. Configure it. Exemptions go in tool configuration or inline suppressions, never prose.
- Do NOT file a follow-up card in place of doing the work.
- Stop and escalate ONLY when a step is impossible. Report the exact command and its output. "It seems risky" is not an escalation.

## The work

Correction to ^teemmch — that card preserved the prompt rule's judgment. The goal is objectivity per the matrix row: dead code = compiler + cargo machete (Rust), knip/ts-prune (TS), vulture (Python), dart analyze unused (Dart), periphery (Swift), compiler (Go). Cover EVERY column. The one subjective carve-out — work-in-process scaffolding — becomes an annotation contract: staged code carries the language's own suppression marker, or it is dead. The compilers already exempt the other carve-outs natively (pub/exported items, cfg(test), main, FFI).

Per language, each rule `supersedes: dead-code` for the files it matches:

- Rust — `dead-code-rust`, workspace scope: `cargo check --message-format=json` piped through jq selecting the `dead_code` lint code. Suppression: `#[expect(dead_code, reason = "...")]` — the staging contract. Plus the orphan-module check the compiler cannot make: a changed `.rs` file that is not `lib.rs`/`main.rs`/`mod.rs`/a `#[path]` target must be named by a `mod` declaration in its crate; grep answers this. (`cargo machete` for unused dependencies is ^s6bn43j, same batch.)
- Go — promote the existing `unused-code-go` to `supersedes: dead-code`. The compiler already errors on unused locals and imports; U1000 covers the rest. Suppression: `//lint:ignore U1000 <reason>`.
- TypeScript — `dead-code-typescript`, workspace scope: `ts-prune` (chosen over knip: it has the inline suppression `// ts-prune-ignore-next` and a narrower, objective claim — unused exports). Map its `path:line - name` output. Pin the version.
- Python — `dead-code-python`: `vulture` per the matrix, default confidence. Suppressions: `--ignore-names`/`--ignore-decorators` in the run script for framework patterns, and vulture's whitelist mechanism for the code's own exemptions; verify current noqa support against the installed tool and state the working suppression in the rule body. `unreachable-code-python` folds into this rule — one owner per finding.
- Dart — `dead-code-dart`, workspace scope: `dart analyze` selecting the `unused_element`, `unused_field`, `unused_import`, `unused_local_variable` diagnostics; keep findings in changed files. Suppression: `// ignore: unused_element` style comments.
- Swift — `dead-code-swift`, workspace scope: `periphery scan --format json`. It needs an SPM/Xcode project and a build; when the project or the tool is absent, doctor reports it and the prompt rule runs — the designed fallback. Suppression: `// periphery:ignore`. Fixtures carry a minimal `Package.swift`, the same shape as the cargo fixture package.

Documentation in the same change:
- Rewrite `dead-code.md` down to the fallback it now is, and state the annotation contract.
- Amend `builtin/validators/README.md` rules-for-tool-rules: an exemption a human would argue in prose must be an inline suppression the tool reads; staging is the canonical example.
- Update the code-hygiene VALIDATOR.md dead-code record: the ^teemmch "do not supersede" decision is reversed, and the knip/periphery/vulture rejections are superseded by this card.

Each rule ships a fail/pass fixture pair (fail = unannotated dead item; pass = the same item with the suppression and a reason). Extend `SHIPPED_DEAD_CODE_RULES` and the acceptance test to every rule.

#tool-validators #dead-code #objectivity