---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m6bp6ss29dvaazxerqbc6reg
  text: |-
    ### implement — changed
    - evidence: deleted crates/llama-embedding; edited Cargo.toml (member, llama-cpp-2, llama-cpp-sys-2, llama-embedding, fork comments), Cargo.lock (cargo update --workspace removed llama-cpp-2, llama-cpp-sys-2, libloading, shlex), benches/embedding_benchmark (Cargo.toml + 3 bench files ported to ANE-only), crates/model-embedding (lib.rs, batch.rs docs), crates/model-loader (example + api test comments), crates/ane-embedding/src/lib.rs docs, ARCHITECTURE.md, .config/nextest.toml (llama-embedding-serial renamed embedding-serial, dead override removed), .github/workflows/ci.yml (LLAMA_N_GPU_LAYERS comment removed).
    - commands: cargo tree -i llama-cpp-sys-2 and llama-cpp-2 find nothing; cargo check --workspace --all-targets passes with no warnings.
    - next: review
  timestamp: 2026-11-01T13:53:56.514077+00:00
- actor: claude-code
  id: 01m6bpjc0zj02kycsctnkt636h
  text: |-
    ### test — green
    - evidence: cargo nextest run --workspace: 13373 passed, 0 failed, 0 skipped (1 leaky: ane-embedding error::tests::test_debug_impl, a pure test; it did not leak when ane-embedding ran alone, 53/53 passed). cargo clippy --workspace --all-targets -- -D warnings: clean. cargo fmt --all --check: clean. cargo nextest list --profile embedding-models -p swissarmyhammer-embedding --features embedding-models: resolves, lists tests. cargo check -p embedding-benchmark --benches: clean.
    - next: review
  timestamp: 2026-11-01T14:00:15.647240+00:00
- actor: claude-code
  id: 01m6bpksr5scp9bgfbxtj74mjb
  text: |-
    ### commit — changed
    - evidence: commit subject "refactor(embedding): delete the llama-embedding crate and drop llama-cpp-2". Files: crates/llama-embedding deleted; Cargo.toml, Cargo.lock, benches/embedding_benchmark, crates/model-embedding, crates/model-loader, crates/ane-embedding/src/lib.rs, ARCHITECTURE.md, .config/nextest.toml, .github/workflows/ci.yml, and the task's .kanban files. The sha is in the step reply.
    - next: review
  timestamp: 2026-11-01T14:01:02.469395+00:00
- actor: claude-code
  id: 01m6bps5r3ddphvk1asshzv8gc
  text: |-
    ### review — clean
    - evidence: review sha HEAD~1..HEAD (8f242ebee). counts: findings 0, confirmed 0, refuted 0, attempted 7, failed 0. 23 files reviewed. No validator matched .config/nextest.toml, ARCHITECTURE.md, Cargo.lock.
    - next: task moved to done.
  timestamp: 2026-11-01T14:03:58.595228+00:00
- actor: claude-code
  id: 01m6bpsp9vteggsegjvhvz0v7g
  text: |-
    ### finish iteration 1 — clean
    - implement: changed — crate deleted, workspace deps removed
    - test: green — cargo nextest workspace, 13373 passed
    - commit: 8f242ebee
    - review: clean — 0 findings, task moved to done
  timestamp: 2026-11-01T14:04:15.547453+00:00
depends_on:
- 01M6B3R8GZ203082MAWAXFRBDN
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffffd680
title: Delete llama-embedding crate and llama-cpp workspace dependencies
---
Goal: the Cargo build no longer compiles llama.cpp.

Steps:
- Delete `crates/llama-embedding`.
- In root `Cargo.toml`: remove the workspace member, `llama-embedding`, `llama-cpp-2`, `llama-cpp-sys-2`, and the fork comments near line 233.
- Remove `llama-embedding` from `benches/embedding_benchmark/Cargo.toml`, or port the benchmark to ANE.
- Fix `crates/model-loader` comments and tests that name `llama-cpp-2`.
- Update `crates/model-embedding` docs and `ARCHITECTURE.md` line ~495.
- Run `cargo update` and check that `cargo tree -i llama-cpp-sys-2` finds nothing.
- Remove the `LLAMA_N_GPU_LAYERS` comment in `.github/workflows/ci.yml`.

Depends on the card "Remove llama-embedding backend from swissarmyhammer-embedding".

Done when: CI `Test` job no longer runs the llama.cpp CMake build, and the OpenSSL 4 failure in run 41285795949 is gone. #remove-llama-cpp