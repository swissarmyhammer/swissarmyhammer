---
title: Python Project Guidelines
description: Best practices and tooling for Python projects
partial: true
---

### Python Project Guidelines

**Environment Management:**
- Check for virtual environment: `venv/`, `.venv/`, or `env/`
- Activate before running commands: `source venv/bin/activate` (Unix) or `venv\Scripts\activate` (Windows)
- Create new venv: `python -m venv venv`

**Dependency Management:**
- `requirements.txt` → use `pip install -r requirements.txt`
- `pyproject.toml` with Poetry → use `poetry install`
- `setup.py` → use `pip install -e .` for editable install

**Common Commands:**
- Install dependencies: `pip install -r requirements.txt` or `poetry install`
- **Run ALL tests:** `pytest` or `python -m pytest` (discovers all tests automatically)
- **Run specific test file:** `pytest tests/test_file.py`
- **Run specific test:** `pytest tests/test_file.py::test_function_name`
- **For unittest:** `python -m unittest discover` (runs all tests)
- Format: `black .` (if using Black formatter)
- Lint: `pylint` or `flake8` or `ruff check`
- Type check: `mypy .` (if using type hints)

**IMPORTANT:** Do NOT glob for test files. Use `pytest` or `python -m unittest discover` - these tools automatically find all tests.

**Best Practices:**
- Always activate the virtual environment before running commands
- Use `pytest` for testing when available (more features than unittest)
- Check for `tox.ini` or `noxfile.py` for automated testing across Python versions
- Look for `.python-version` or `pyproject.toml` to determine required Python version

**File Locations:**
- Source code: `src/`, package name directory, or root
- Tests: `tests/` or `test/`
- Virtual environment: `venv/`, `.venv/`, or `env/` (git-ignored)
- Cache: `__pycache__/`, `.pytest_cache/` (git-ignored)
