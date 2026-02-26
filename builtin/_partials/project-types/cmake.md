---
title: CMake Project Guidelines
description: Best practices and tooling for CMake projects
partial: true
---

### CMake Project Guidelines

**Build Process:**
1. Create build directory: `mkdir -p build && cd build`
2. Configure: `cmake ..` or `cmake -DCMAKE_BUILD_TYPE=Release ..`
3. Build: `cmake --build .` or `make` (if using Make generator)
4. Test: `ctest` or `make test`
5. Install: `cmake --install .` or `make install`

**Common Configurations:**
- Debug build: `cmake -DCMAKE_BUILD_TYPE=Debug ..`
- Release build: `cmake -DCMAKE_BUILD_TYPE=Release ..`
- Specify generator: `cmake -G "Unix Makefiles" ..` or `cmake -G Ninja ..`
- Parallel build: `cmake --build . -j $(nproc)` or `make -j$(nproc)`

**Best Practices:**
- Always use out-of-source builds (separate `build/` directory)
- Clean rebuild: `rm -rf build && mkdir build && cd build && cmake ..`
- Use `ccache` for faster rebuilds if available
- Check `CMakeLists.txt` for custom targets and options

**Testing:**
- **Run ALL tests:** `ctest` (discovers and runs all CTest tests automatically)
- **Run with failure output:** `ctest --output-on-failure` (recommended)
- **Run with verbose output:** `ctest -V` or `ctest --verbose`
- **Run specific test:** `ctest -R test_name` (regex pattern)
- **Run tests in parallel:** `ctest -j$(nproc)` or `ctest -j4`
- Alternative: `make test` (if using Make generator)

**IMPORTANT:** Do NOT glob for test files. CMake/CTest automatically discovers tests defined in CMakeLists.txt. Use `ctest` to run all tests.

**File Locations:**
- Configuration: `CMakeLists.txt` (root and subdirectories)
- Source code: `src/`, `lib/`, or project-specific
- Headers: `include/` or `src/`
- Tests: `test/` or `tests/`
- Build output: `build/` (git-ignored, out-of-source)
