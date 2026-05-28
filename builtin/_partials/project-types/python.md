---
title: Python Project Guidelines
description: Best practices and tooling for Python projects
partial: true
---

### Python Project Guidelines

**Virtualenv:** look for `venv/`, `.venv/`, or `env/`. Activate before running anything: `source venv/bin/activate` (Unix) or `venv\Scripts\activate` (Windows). Create: `python -m venv venv`.

**Dependencies:**
- `requirements.txt` → `pip install -r requirements.txt`
- `pyproject.toml` (Poetry) → `poetry install`
- `setup.py` → `pip install -e .` (editable)

**Testing — let the runner discover tests; do NOT glob:**
- All: `pytest` (or `python -m pytest`); unittest: `python -m unittest discover`
- File: `pytest tests/test_file.py`
- Single: `pytest tests/test_file.py::test_function_name`

**Formatting** (check `pyproject.toml` for what's configured):
- Black: `black .` / `black --check .`
- Ruff: `ruff format .` / `ruff format --check .`
- isort: `isort .` / `isort --check .`

**Lint/type:** `ruff check`, `pylint`, `flake8`; `mypy .` for types.

**Best practices:** activate venv first; prefer pytest; check `tox.ini`/`noxfile.py` for multi-version testing; check `.python-version`/`pyproject.toml` for required Python version.

**File locations:** `src/` or package dir (source), `tests/` or `test/`, `venv/` and `__pycache__/` git-ignored.
