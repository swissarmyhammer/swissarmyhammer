---
assignees:
- claude-code
position_column: todo
position_ordinal: d380
title: 'Decide: unify resolved_skill_names / resolved_agent_names, or document why not'
---
`resolved_skill_names` and `resolved_agent_names` in `crates/mirdan/src/install.rs` (real lines ~2151-2166) are now textually identical.

This is the one finding from ^qsr5rdt's review with a genuine causal link to that commit. The two functions were already parallel, but they built *different* maps: skills passed real profile tags, agents passed an empty `Vec::new()` placeholder. Collapsing `Selector::select` onto `&HashSet<String>` removed the difference, so the parallel structure became exact duplication. The structure is pre-existing; the convergence is the commit's consequence.

## The remedy may not be worth it

The review asked for a `BuiltinResolver` trait unifying `SkillResolver` and `AgentResolver`. Those live in **different crates**, so this is a cross-crate abstraction for two four-line functions. The reviewer flagged it as disproportionate, and I agree on first read.

So this card is a decision, not a foregone refactor. Pick one and record the reasoning in the code:

1. **Introduce the trait** — justified only if something else wants to resolve builtins generically. Check for a third caller before building it.
2. **Leave both and say why** — a comment at each site noting they are intentionally duplicated because unifying them would mean a cross-crate trait for eight lines. This is a legitimate outcome; two four-line twins are cheaper than a premature abstraction.
3. **A middle option** — a small generic helper local to `install.rs` taking the resolved-name closure, if that collapses them without a cross-crate trait.

Do NOT introduce the trait mechanically because a validator asked. The rule against declining findings exists to stop dismissal on grounds of taste; it does not require the most expensive remedy when a cheaper one satisfies the underlying concern. If you choose option 2, the finding is satisfied by the recorded decision.

## Acceptance

- One of the three outcomes above is implemented, and the reasoning is in the code where the next reader will find it.
- If a trait is introduced, it has at least two real implementors and a test.
- `cargo nextest run -E 'rdeps(mirdan)'`, `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` clean.

Related: ^927239f covers the rest of the `install.rs` cleanup. #refactor