# `.expect/` — behavioral expectations

This directory holds everything the `expect` feature owns: configuration, the
worked example, repo-global expectations, approved goldens, and the last received
run per spec. `sah init` (and `expect init`) scaffolds it and is safe to re-run —
it never overwrites a file you already have.

## What is an expectation?

An expectation is a human-authored statement of how the system *should* behave,
written in natural language and checked against the **running** system. Each
expectation lives in a `*.expect.md` file with:

- `description` and `surface` frontmatter (the only required keys),
- a prose body stating the *intent* (what "correct" means, and why),
- at least one bounded acceptance criterion in a `## Then` checklist.

`expect` drives the system, observes the authoritative state, and grades each
criterion against the stated intent — never trusting the driver's own claim of
success.

## How to write one

1. Copy `example.expect.md` next to the feature it describes (specs may live
   anywhere in the tree, e.g. `src/checkout/coupon.expect.md`), or under
   `expectations/` for repo-global specs not tied to one source dir.
2. State the intent in the body. The prose *is* the intent — there is no
   `intent:` field; a spec that is all mechanics and no stated reason is a
   failure mode `doctor` flags.
3. List bounded criteria in `## Then`. Keep each one a single checkable claim.
4. Run `expect expectation check <path>` to observe and grade it, then
   `expect observation approve` to promote a good run to its golden baseline.

## Layout

| Path             | Purpose                                                       |
|------------------|--------------------------------------------------------------|
| `config.toml`    | grading model, embedder, thresholds, approval policy         |
| `example.expect.md` | one worked expectation, ready to copy                     |
| `expectations/`  | repo-global expectations not tied to a single source dir     |
| `goldens/`       | approved, scrubbed observations (committed)                  |
| `received/`      | last run per spec (gitignored)                               |
| `.gitignore`     | ignores `received/`, keeps `goldens/` tracked                |

A feature-local spec at `src/checkout/coupon.expect.md` keeps its golden at
`goldens/src/checkout/coupon.golden.json` — the golden tree mirrors each spec's
repo-relative path, so identity needs no `id`.

See `ideas/expect.md` for the full design.
