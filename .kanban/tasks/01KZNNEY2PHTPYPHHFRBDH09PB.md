---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m01c4z6by7f38w58hkx7p67p
  text: |-
    Research done. Corpus cloned to the scratchpad and measured. Four findings change the plan.

    **1. vulture 2.14 already reads TWO of the three facts, not one.** `core.py` shows it:
    - `_assigns_special_variable__all__` accepts a LIST **or a TUPLE**, so `psf/requests`, which states a tuple `__all__`, IS read. The rule body's claim that "neither library declares in `__all__`" is wrong for requests.
    - `_add_aliases` does `if alias is not None: self.used_names.add(name_and_alias.name)`. Any aliased import marks the name used, so the PEP 484 explicit re-export `from .views import MethodView as MethodView` is already read. Probe: a name behind the redundant alias is silent; the same name behind a plain import reports.
    - `_ignore_import` drops every import in an `__init__.py`.
    - `_ignore_class` drops a class with `Test` in its name IN a test file; `_ignore_function`/`_ignore_method` drop `test_*` and the pytest setup names there.

    So a whitelist naming `__init__.py` re-exports moves ZERO findings. Measured 100 -> 100 over the two package directories.

    **2. The one surface fact vulture does NOT read is the entry point.** Probe: a function named in `[project.scripts]` still reports. That is the gap the run has to close.

    **3. The rule body's 118/114/100 table was measured over the two PACKAGE DIRECTORIES, not the repositories.** Reproduced exactly: `src/requests` + `src/flask` copied side by side gives 100 findings with the shipped flags, and `MethodView`, `HTTPDigestAuth`, `get_namespace`, `iter_lines` and `from_key_val_list` are all in it. Over the WHOLE repositories the same names are silent, because each library's own tests are callers. Whole-repo counts at the pinned commits: requests 89, flask 105, fastapi 1318, django 3255.

    **4. Carve-out 2 is the big one.** Decorators on the reported items: fastapi has 685 `@*.get`, 277 `@*.post`, 37 `@*.put`, 27 `@*.websocket`, 11 `@*.exception_handler`; django has 57 `@*.register_lookup`, 50 `@*.tag`, 23 `@receiver`, 21 `@*.display`, 19 `@*.filter`.

    Carve-out 3's stated gaps do not appear in the corpus: 0 `Test*` class findings over 4184 `.py` files, and no repo writes a bare `from pytest import fixture`. Vulture's native test-file handling covers them. The residue is a TestCase subclass OUTSIDE a test file, which the run can close by reading the class statement.

    Tool survey (real installs, same probe): `dead` 2.1.0 reads entry points but only from `setup.py`/`setup.cfg`, never `pyproject.toml`, where all four corpus repos state theirs; it also needs `git ls-files` and has no decorator lever. `deadcode` 2.4.1 reads `__all__` but NOT the re-export alias, and it crashes on Python 3.14. `ruff` has no cross-module unused-symbol rule at all. vulture stays.
  timestamp: 2026-08-15T00:11:35.499342+00:00
- actor: claude-code
  id: 01m01d46e8xqc5273c1fr5hv0m
  text: |-
    Implementation landed. The decision on the public-surface carve-out follows ^108bh4y: read the facts the package states, and add no marker to correct code.

    **The decision.** Python states its surface in three places. Vulture 2.14 already reads two of them — `__all__` as a list or a tuple, and the PEP 484 explicit re-export `from .m import N as N` — and the rule body now states both with the `core.py` function that reads each and a probe that proves it. The one fact vulture reads nowhere is the ENTRY POINT, so the run reads that: `[project.scripts]`, `[project.gui-scripts]` and every `[project.entry-points]` group of each `pyproject.toml`, `[options.entry_points]` of each `setup.cfg`, and the `entry_points=` literal of each `setup.py`, written into a vulture whitelist module. No marker is demanded of any correct name.

    **A defect the entry-point probe uncovered.** Vulture reads `[tool.vulture]` out of the project's own `pyproject.toml` and merges it UNDER the command line, so any option the run did not state was the project's. Measured over a probe holding one dead function: `ignore_names = ["*"]` reported 0 at exit 0; `min_confidence = 100` beside `make_whitelist = true` reported 0 at exit 0; and a `pyproject.toml` that is not TOML at all made vulture exit 1 on a traceback, which the pipe read as a clean tree. The run now writes a `[tool.vulture]` table of its own and passes `--config`, states `--min-confidence 60`, and became a script that accepts vulture's status 0 and 3 and breaks on anything else.

    **Whole-corpus before and after**, each repository read as a workspace:

    | repository | commit | .py | before | after | dropped |
    |---|---|---|---|---|---|
    | psf/requests | 8068356 | 37 | 89 | 89 | 0 |
    | pallets/flask | 2a8a38b | 83 | 105 | 98 | 7 |
    | fastapi/fastapi | a1fa70d | 1136 | 1318 | 272 | 1046 |
    | django/django | 3436cf9 | 2928 | 3255 | 2952 | 303 |
    | this workspace's Python | HEAD | 3 | 11 | 11 | 0 |

    flask's 7 are route handlers under `@app.get` / `@app.post` inside its own tests — Flask 3's HTTP-method shortcuts, which the old `@*.route` never covered. fastapi's 1046 are 775 in `tests/` and 271 in `docs_src/`. django's 303 are 147 classes, 120 functions, 34 methods and 2 properties under the Django registration roster.

    The old table's 100 reproduces exactly over `src/requests` + `src/flask` side by side, and stays 100 after the change, because `MethodView`, `HTTPDigestAuth`, `get_namespace`, `iter_lines` and `from_key_val_list` are stated nowhere. The rule body says so plainly and names the three answers an author has.

    TDD: watched the fixture pair go RED first (the pass fixture reported exactly 2: `published_command` and `PassCase`), then the acceptance tests RED (4 of 6 failed with the expected diffs; the two facts vulture already reads passed from the start, which is the point of that test).
  timestamp: 2026-08-15T00:28:38.728527+00:00
- actor: claude-code
  id: 01m01d4dv21jn2y787erhg5sep
  text: |-
    ### implement — changed
    - evidence: 6 files — builtin/validators/code-hygiene/rules/dead-code-python.md, builtin/validators/code-hygiene/fixtures/pyproject.toml.tmpl (new), builtin/validators/code-hygiene/fixtures/dead-code-python.fail.py.tmpl, builtin/validators/code-hygiene/fixtures/dead-code-python.pass.py.tmpl, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/dead_code_python.rs (new), crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/temp_directory.rs. `cargo nextest run -p swissarmyhammer-validators` 749/749 pass; `-p mirdan -p swissarmyhammer-cli` 1224/1224 pass; `cargo fmt --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean.
    - next: /review
  timestamp: 2026-08-15T00:28:46.306179+00:00
- actor: claude-code
  id: 01m01dgpb32h8m66yvnxqvvhw1
  text: |-
    ### review — clean
    - evidence: `review sha 7db629a21~1..7db629a21` — 0 findings, 0 confirmed, 0 refuted, 9 validator attempts, 0 failed. No findings to record, so no dated Review Findings section was appended.
    - scope: the report scope line states "reviewed the diffs only — lines this change added or modified". 3 files reviewed, 3 excluded.
    - fixtures: the engine excluded all three `.tmpl` fixtures by category, each named in the report with the reason "validator fixture". No finding asked to remove or use the dead code the fixtures demonstrate, so the mechanism is intact.
    - coverage limit (read before trusting "clean"): the 544-line rule body `builtin/validators/code-hygiene/rules/dead-code-python.md` — the substance of this commit — was NOT read by any validator. No validator in the roster declares a `*.md` match glob. The three reviewed files are the three `.rs` files, and the 9 attempts are exactly the 9 validators matching `*.rs`. The clean result covers the Rust acceptance tests, not the prose of the rule.
    - vulture claim verified by measurement, not taken on trust (vulture 2.14 installed). Two-file probe: `pkg/m.py` defines three functions, `pkg/__init__.py` re-exports one behind `from .m import alias_used as alias_used` and names one in a tuple `__all__`. Result: only the third, `control`, reported. Negative control — the plain import and an empty `__all__` — made all three report. A list `__all__` behaved the same as the tuple. Both mechanisms the carve-out rests on are real and load-bearing, so no marker was needed and the carve-out reproduces.
    - next: none — task moved to done.
  timestamp: 2026-08-15T00:35:28.227843+00:00
- actor: claude-code
  id: 01m01djk8bwh6rkjr70rhrp0sa
  text: |-
    ### finish iteration 1 — clean. Card done.
    - **The card's premise was corrected by reading vulture's source.** vulture does not know only `__all__`. `_add_aliases` already reads the PEP 484 explicit re-export (`from .views import MethodView as MethodView`), which is how flask states its whole surface in 39 lines and fastapi in 20; and `_assigns_special_variable__all__` accepts a TUPLE, so `requests` — which the card says declares nothing — is read. So NO marker was added to correct code. The run reads the one fact vulture reads nowhere: the entry point, from pyproject.toml, setup.cfg and setup.py, into a whitelist module.
    - **A live defect surfaced while building the probe**: vulture reads a `[tool.vulture]` section out of the PROJECT'S OWN pyproject.toml and merges it under the command line. `ignore_names = ["*"]` reported 0 findings at exit 0, and a pyproject.toml that is not valid TOML made vulture exit 1 on a traceback, which the old pipe read as a clean tree. The run now passes its own `--config`, states `--min-confidence 60`, and is a script accepting only vulture's status 0 and 3.
    - **Tool survey drove five candidates over one probe package**, ruling out four with reasons: `dead` never reads pyproject.toml — where all four corpus packages state their entry points — needs `git ls-files`, and has no decorator roster; `deadcode` does not read the redundant alias and crashes on Python 3.14; neither `ruff` nor `pylint` has any cross-module unused-symbol rule, their whole unused set being scoped to a file or a private name.
    - Measured whole-repository at named commits: requests 8068356, 37 files, 89→89; flask 2a8a38b, 83 files, 105→98; fastapi a1fa70d, 1136 files, **1318→272**; django 3436cf9, 2928 files, 3255→2952. flask's 7 are Flask 3's own `@app.get`/`@app.post` shortcuts, which `@*.route` never covered.
    - Carve-out 3's stated gaps DO NOT OCCUR: 0 `Test`-named classes reported over 4184 files, because vulture handles them natively in test files, and the residue is closed by reading the class statement transitively.
    - The old table's 100 reproduces exactly and stays 100 — MethodView, HTTPDigestAuth, get_namespace, iter_lines and from_key_val_list are stated nowhere, and the rule now says so.
    - test: green — 749 validators, 1224 combined. fmt and clippy clean.
    - commit: 7db629a21
    - review: clean — 0 findings, 9 attempts, 0 failed. The reviewer VERIFIED the load-bearing vulture claim itself rather than accepting it: a two-file probe reported only the control, and switching to a plain import with an empty `__all__` made all three report — a negative control settling causation. Both legs hold. All three fixtures were excluded by the engine as `validator fixture`.

    **The clean verdict is narrower than it looks, and the reviewer said so loudly.** The 544-line rule body — the substance of this commit — was read by NO validator, because none declares a `*.md` glob. Exact accounting: 9 changed files, 2 `.kanban` dropped by .reviewignore, 3 fixtures excluded by category, leaving 4; the 3 reviewed are the `.rs` files, and `attempted: 9` is precisely the number of validators matching `*.rs`. Third occurrence this session, now carded as ^j169agt.
  timestamp: 2026-08-15T00:36:30.603065+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffff8980
title: dead-code-python reports a package's whole public API when it declares no __all__
---
`builtin/validators/code-hygiene/rules/dead-code-python.md` runs `vulture` and declares `supersedes: [dead-code]`.

`dead-code.md` exempts "**Exported public API**: a `pub`/exported item that is the crate's/library's surface for *external* callers. Its callers live outside this repo, so an empty inbound callgraph is expected, not dead."

vulture's only notion of an exported surface is `__all__`. The rule's own measurement is the proof: of 118 findings over `psf/requests` and `pallets/flask`, "The 100 that remain are almost all the two libraries' public API — `MethodView`, `HTTPDigestAuth`, `get_namespace`, `iter_lines`, `from_key_val_list` — which neither library declares in `__all__`." The prompt rule would report none of them.

The entry-point carve-out is partial. `--ignore-decorators` is a fixed list. It covers Flask `@*.route`, click `@*.command`, celery `@*.task` and pytest `@*.fixture`. It does NOT cover FastAPI or Starlette routing (`@app.get`, `@router.post`), Django `@receiver`, or typer. Those handlers report.

The test carve-out is partial for the same reason: `--ignore-names` is case-sensitive fnmatch, so `test_*` covers `def test_foo` but not a `unittest` `class TestFoo(TestCase)`, and a bare `from pytest import fixture` used as `@fixture` does not match `@*.fixture`.

`# noqa: V1xx` works, so an annotation contract is available. Decide how the rule states the public-surface carve-out.

Found by the `supersedes` survey on ^h7garpc. #tool-validators #objectivity