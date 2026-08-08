---
name: dead-code-python
description: Python names nothing reads and statements behind a jump — checked by vulture, not by prompt.
match:
  files:
    - "**/*.py"
    - "**/*.pyw"
  project_types:
    - python
supersedes: dead-code
tool:
  scope: workspace
  run: |
    vulture \
      --exclude '*/.venv/*,*/venv/*,*/node_modules/*,*/target/*,*/build/*,*/dist/*,*/__pycache__/*,*/.tox/*,*/.mypy_cache/*' \
      --ignore-names '__*__,test_*,setUp,setUpClass,tearDown,tearDownClass,visit_*,pytest_*,forward' \
      --ignore-decorators '@*.route,@*.fixture,@*.command,@*.task,@*.errorhandler,@*.before_*,@*.after_*,@*.teardown_*,@*.setter,@*.getter,@*.deleter,@*.register,@*.validator,@*.event,@overload,@*.overload,@abstractmethod,@*.abstractmethod' \
      . |
      sed -n 's/^\(.*\) ([0-9]*% confidence)$/\1/p'
  doctor:
    check_command: "which vulture sed"
    check_version_command: "vulture --version"
  install:
    commands:
      - "uv tool install vulture==2.14"
      - "pipx install vulture==2.14"
---

# Dead Code — Python

`vulture` reports every name no other name reads — the import, the class, the
function, the method, the property, the attribute, the module variable — and
every statement behind a jump its branch always takes. This one rule owns all of
it. `unreachable-code-python` was folded in here so that one finding has one
owner.

The confidence floor is vulture's own default of 60, not the 100 the folded rule
used. 100 admits unreachable code alone. 60 admits the whole set, which is the
point of this card: dead code stops being a judgment.

## The staging contract, and the exported surface

Python has no compiler to consult, so both carve-outs the compiler makes for
free in Rust and Go are markers here. Both were verified against vulture 2.14.

**Staged work** carries a `# noqa` with vulture's own code, on the line the tool
reports:

| Code | Kind |
|---|---|
| `V101` | unused attribute |
| `V102` | unused class |
| `V103` | unused function |
| `V104` | unused import |
| `V105` | unused method |
| `V106` | unused property |
| `V107` | unused variable |
| `V201` | unreachable code |

A bare `# noqa` suppresses every kind on its line. A wrong code suppresses
nothing — measured: `# noqa: V102` on an unused function still reports. Write
the reason beside the marker; the marker says the code is staged and the reason
says what lands the consumer. For a property the reported line is the
`@property` decorator, not the `def`, so the marker goes on the decorator.

Vulture reads `# noqa`. `# vulture: ignore` does nothing — measured, still
reported.

**Exported public API** is `__all__`. Vulture treats every name a module's
`__all__` lists as used, which is Python's own way of saying "the callers are
outside this module". Measured on a probe: with `__all__ = ["PublicThing",
"public_call"]` only the third, unlisted function reports; with `__all__ = []`
all three report. A package that exports without `__all__` has not declared its
surface, and this rule says so.

For a surface too large to annotate name by name, vulture reads a whitelist
module — a plain Python file that mentions each name — which the project owns
and passes on the command line. That is configuration the code carries, not
prose in a review.

## What the run script exempts, and the measurement behind it

`--ignore-names` and `--ignore-decorators` carry the framework patterns: a name
or a decorator that means "something outside this codebase calls it by
convention". They are the tool-configuration half of the contract, and each
entry is a convention rather than one project's exception.

Measured over `psf/requests` and `pallets/flask` at HEAD, neither of which
carries a vulture configuration of its own:

| Run | Findings |
|---|---|
| default confidence, no flags | 118 |
| plus `--ignore-names` | 114 |
| plus `--ignore-decorators` | 100 |

`--ignore-names` drops 4: the `__getattr__` module hooks Python calls by
protocol, and `test_client` / `test_cli_runner`, which the `test_*` pattern
covers. `--ignore-decorators` drops 14, and every one of them is a
`@t.overload` stub — `template_filter`, `app_template_test`, `iter_lines` and
their siblings, each declared twice as a typing overload before its real
definition. Without `@overload` in the list a typed codebase reports every
overload stub twice over.

The 100 that remain are almost all the two libraries' public API — `MethodView`,
`HTTPDigestAuth`, `get_namespace`, `iter_lines`, `from_key_val_list` — which
neither library declares in `__all__`. That is the finding, not a defect in the
tool: a Python package that never names its surface leaves a reader and a tool
in exactly the same position.

Measured over this workspace's own Python (`crates/ane-embedding/convert`): 15
findings, all framework-invoked — PyTorch's `forward`, and metadata attributes
written onto a `coremltools` model. `forward` is in `--ignore-names` for that
reason; it is the `nn.Module` protocol name, in the same class as `setUp` and
`visit_*`. The attributes take a `# noqa: V101`.

## How the run is shaped

The scope is `workspace` because "unused" is a whole-tree question. Passing only
the changed files makes vulture report every helper the unchanged files call.
The engine keeps only the findings in the changed files.

`--exclude` keeps the scan out of the directories a build writes — the
virtual environment, `node_modules`, `target`, `build`, `dist`, `__pycache__`,
`.tox` and `.mypy_cache`. Each pattern is written as an explicit glob because a
vulture pattern with no wildcard is matched as `*PATTERN*` against the absolute
path, and a bare `build` would then exclude any file whose path holds that word.

Selection in the pipe is attribution, not exemption. The `sed` strips the
confidence suffix vulture appends, so the line reads as the
`path:line: message` the engine parses. Ending the pipe in `sed` also normalizes
the exit status, because vulture exits 3 when it has findings.
