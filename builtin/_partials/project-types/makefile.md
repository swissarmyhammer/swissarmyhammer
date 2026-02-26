---
title: Makefile Project Guidelines
description: Best practices and tooling for Makefile-based projects
partial: true
---

### Makefile Project Guidelines

**Common Commands:**
- Build default target: `make`
- Clean: `make clean`
- Install: `make install`
- **Run ALL tests:** `make test` or `make check` (standard test targets)
- **Try multiple targets:** If `make test` fails, try `make check`, `make tests`, or `make test-all`
- List targets: `make help` (if defined) or `grep '^[^#[:space:]].*:' Makefile`
- Parallel build: `make -j$(nproc)` or `make -j4`
- Verbose output: `make V=1` or `make VERBOSE=1`

**IMPORTANT:** Do NOT glob for test files. Makefiles define test targets. Use `make test` or `make check` to run all tests as configured.

**Best Practices:**
- Always run `make clean` before full rebuilds
- Check `Makefile` or run `make help` to see available targets
- Use parallel builds (`-j`) for faster compilation
- Look for `config.mk` or similar for configuration options

**Discovery:**
- List targets: `grep '^[^#[:space:]].*:' Makefile` (shows all targets)
- Common targets: `all`, `build`, `clean`, `install`, `test`, `check`, `distclean`
- Check for `configure` script that generates the Makefile

**Configuration:**
- If `configure` exists: `./configure && make`
- Custom configuration: `./configure --prefix=/custom/path`
- Autotools project: `./autogen.sh && ./configure && make`

**File Locations:**
- Configuration: `Makefile` (main) and possibly `*.mk` includes
- Source code: Project-specific (check `Makefile` for `SRC` variables)
- Build output: Often in-source, check for `build/` or `out/`
- Object files: `*.o` files (usually git-ignored)

**Variables:**
- Override at build time: `make CC=clang` or `make CFLAGS="-O2 -Wall"`
- Check common variables: `CC`, `CXX`, `CFLAGS`, `CXXFLAGS`, `LDFLAGS`
