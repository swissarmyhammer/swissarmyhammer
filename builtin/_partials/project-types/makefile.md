---
title: Makefile Project Guidelines
description: Best practices and tooling for Makefile-based projects
partial: true
---

### Makefile Project Guidelines

**Common commands:**
- Build default: `make`
- Clean: `make clean`
- Install: `make install`
- Parallel: `make -j$(nproc)`
- Verbose: `make V=1` or `make VERBOSE=1`

**Testing — do NOT glob; targets are defined in the Makefile:**
- Try `make test`, then `make check`, `make tests`, `make test-all`

**Discovery:**
- List targets: `make help` if defined, else `grep '^[^#[:space:]].*:' Makefile`
- Common targets: `all`, `build`, `clean`, `install`, `test`, `check`, `distclean`

**Formatting:** look for `make format`/`make fmt`. For C/C++ check for `.clang-format`.

**Configuration:**
- If `configure` exists: `./configure && make` (custom prefix: `./configure --prefix=/path`)
- Autotools: `./autogen.sh && ./configure && make`
- Variable overrides: `make CC=clang CFLAGS="-O2 -Wall"` (common: `CC`, `CXX`, `CFLAGS`, `CXXFLAGS`, `LDFLAGS`)

**File locations:** `Makefile` (+ optional `*.mk`); sources are project-specific (check `SRC` variables); output often in-source, sometimes `build/` or `out/`.
