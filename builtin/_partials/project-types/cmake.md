---
title: CMake Project Guidelines
description: Best practices and tooling for CMake projects
partial: true
---

### CMake Project Guidelines

**Always use out-of-source builds.**

1. `mkdir -p build && cd build`
2. `cmake ..` (or `cmake -DCMAKE_BUILD_TYPE=Release ..`)
3. `cmake --build .` (or `make`)
4. `ctest` (test)
5. `cmake --install .` (install)

**Configurations:**
- Debug/Release: `-DCMAKE_BUILD_TYPE=Debug|Release`
- Generator: `-G "Unix Makefiles"` or `-G Ninja`
- Parallel: `cmake --build . -j $(nproc)`
- Clean rebuild: `rm -rf build && mkdir build && cd build && cmake ..`

**Testing — do NOT glob; CTest discovers tests in `CMakeLists.txt`:**
- All: `ctest`
- On failure: `ctest --output-on-failure` (recommended)
- Verbose: `ctest -V`
- Filter: `ctest -R <regex>`
- Parallel: `ctest -j$(nproc)`

**Formatting:**
- C/C++: `clang-format -i` (check: `clang-format --dry-run --Werror`). Config: `.clang-format`.
- CMake files: `cmake-format -i CMakeLists.txt` (if installed)

**File locations:** `CMakeLists.txt` (root + subdirs), `src/`/`lib/`, `include/`, `test/`, `build/` git-ignored. Use `ccache` if available for faster rebuilds.
