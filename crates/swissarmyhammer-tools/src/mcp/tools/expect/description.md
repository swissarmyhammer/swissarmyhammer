Capture, evaluate, and approve behavioral expectations against the running
system, dispatched by `op`.

An expectation is a `*.expect.md` spec — frontmatter plus stated intent plus
bounded criteria — that describes how the system should behave. `expect` drives
those specs through their lifecycle. Every op id is a `<verb> <noun>` pair; the
CLI renders them noun-first (`expect expectation check`, `expect expectations
list`), one command per noun, with cardinality following the noun (singular for
one, plural for the collection or a batch).

The nouns:

- **expectation** / **expectations** — the spec(s). `create` drafts one, `get`
  reads one, `delete` removes one (with its observation and golden), `list`
  surveys the collection with ledger state, and `observe` / `check` drive the
  system: `observe` captures an authoritative observation, `check` is doctor plus
  observe plus evaluate plus compare.
- **observation** / **observations** — one authoritative capture of a run (a
  checkpoint timeline plus the driver trajectory). `get` / `delete` address one,
  `list` surveys them, `evaluate` re-judges a stored observation against its
  criteria without re-running the system, and `approve` promotes it to a golden.
- **golden** / **goldens** — an approved, scrubbed observation; the committed
  baseline. `get` / `delete` address one, `list` surveys them, and `evaluate`
  re-grades the baseline against edited criteria without re-running the system.
- **surface** / **surfaces** — the adapter catalog (cli/http/browser/gui/file/db),
  read-only: `get` one, `list` all.

The static health check (`doctor`) and scaffolding (`init`) are top-level trait
verbs that roll up to `sah doctor` / `sah init`, not `<noun> <verb>` ops on this
tool.

Every op currently returns a structured "not implemented yet" placeholder; the
real implementations land in later tasks.
