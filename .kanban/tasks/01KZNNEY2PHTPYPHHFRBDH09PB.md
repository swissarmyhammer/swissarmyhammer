---
assignees:
- claude-code
position_column: todo
position_ordinal: ffb780
title: dead-code-python reports a package's whole public API when it declares no __all__
---
`builtin/validators/code-hygiene/rules/dead-code-python.md` runs `vulture` and declares `supersedes: [dead-code]`.

`dead-code.md` exempts "**Exported public API**: a `pub`/exported item that is the crate's/library's surface for *external* callers. Its callers live outside this repo, so an empty inbound callgraph is expected, not dead."

vulture's only notion of an exported surface is `__all__`. The rule's own measurement is the proof: of 118 findings over `psf/requests` and `pallets/flask`, "The 100 that remain are almost all the two libraries' public API — `MethodView`, `HTTPDigestAuth`, `get_namespace`, `iter_lines`, `from_key_val_list` — which neither library declares in `__all__`." The prompt rule would report none of them.

The entry-point carve-out is partial. `--ignore-decorators` is a fixed list. It covers Flask `@*.route`, click `@*.command`, celery `@*.task` and pytest `@*.fixture`. It does NOT cover FastAPI or Starlette routing (`@app.get`, `@router.post`), Django `@receiver`, or typer. Those handlers report.

The test carve-out is partial for the same reason: `--ignore-names` is case-sensitive fnmatch, so `test_*` covers `def test_foo` but not a `unittest` `class TestFoo(TestCase)`, and a bare `from pytest import fixture` used as `@fixture` does not match `@*.fixture`.

`# noqa: V1xx` works, so an annotation contract is available. Decide how the rule states the public-surface carve-out.

Found by the `supersedes` survey on ^h7garpc. #tool-validators #objectivity