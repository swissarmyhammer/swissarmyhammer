---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m6c44xtcgpf8jdzxrx4vv8dq
  text: |-
    ### implement — changed
    - evidence: 1 file, .github/workflows/release.yml. Five jobs (plan, build-global-artifacts, host, publish-homebrew-formula, announce) now use `runs-on: self-hosted`. Linuxbrew PATH line replaced by /opt/homebrew/bin. Added an "Ensure jq is available" step (brew, same check as ci.yml) to plan, build-global-artifacts, host, publish-homebrew-formula. Added a step that puts $HOME/.cargo/bin on GITHUB_PATH in build-global-artifacts and host (cached dist lives there). The dist installer URL is unchanged: cargo-dist-installer.sh picks the build by uname, so it fetches the macOS arm64 build. No apt installs or Linux paths remained. docs.yml and ci.yml already use self-hosted; no ubuntu/macos/windows runner remains. `grep runs-on` shows only self-hosted and ${{ matrix.runner }}. No job removed, no publish gate changed. dist-workspace.toml needs no change.
    - actionlint before/after: same 7 messages (6 shellcheck notes plus the docs.yml configure-pages@v3 note, which existed before); only line numbers moved.
    - real check, after push: run the Release workflow by hand with tag input `dry-run`.
    - next: /review
  timestamp: 2026-11-01T17:57:35.180124+00:00
- actor: claude-code
  id: 01m6c45xm4h0rcjrpemjgxas75
  text: |
    ### test — green
    - evidence: `git diff --stat 1f43d2af3 -- '*.rs' '*.toml' Cargo.lock` printed nothing, and `git status --short` for the same globs printed nothing. No Rust or Cargo file changed, so the nextest run was skipped (rerun of unchanged code). `actionlint .github/workflows/*.yml`: 7 notes only (6 shellcheck notes in release.yml, 1 configure-pages@v3 note in docs.yml), 0 new errors. YAML parse with `ruby -ryaml`: ci.yml, docs.yml, release.yml all ok. `cargo fmt --all --check`: exit 0, no output. `grep -rn runs-on .github/workflows`: 12 hits, all `self-hosted` or `${{ matrix.runner }}`.
    - next: review
  timestamp: 2026-11-01T17:58:07.748241+00:00
- actor: claude-code
  id: 01m6c46sfthdxgy5ayrxq3qaac
  text: |-
    ### commit — changed
    - evidence: one local commit, subject "ci(release): run every release job on the self-hosted runner". It holds .github/workflows/release.yml, the 2 kanban files of this task, and the 2 kanban files of task 01M6C36YC1HHGMDA7GFVXQDPJ9. Find the sha with `git log -1 --format=%h --grep='run every release job'`, because a commit cannot hold its own sha. Not pushed.
    - next: review
  timestamp: 2026-11-01T17:58:36.282493+00:00
- actor: claude-code
  id: 01m6c487056da32eck36f1ps4s
  text: |-
    ### review — clean
    - evidence: review sha a1b42ef0d~1..a1b42ef0d. 0 findings, 0 confirmed, 0 refuted. 1 file reviewed. 4 .kanban files skipped by .reviewignore.
    - next: task moved to done. Real check after push: run the release workflow with the dry-run tag input.
  timestamp: 2026-11-01T17:59:22.885905+00:00
- actor: claude-code
  id: 01m6c48krj22e5bcmtw3zs1e40
  text: |-
    ### finish iteration 1 — clean
    - implement: changed — release.yml, 1 file
    - test: green — no Rust changed, nextest skipped with diff evidence; actionlint 0 errors
    - commit: a1b42ef0d
    - review: clean — 0 findings, task moved to done
  timestamp: 2026-11-01T17:59:35.954179+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffffe480
title: Run every release job on the self-hosted runner
---
Goal: no job in this repo runs on a GitHub-hosted runner. This matches the other Foundation packages and the shared `swissarmyhammer/workflows` workflows, which use `self-hosted`.

User decision: change the CI to run on the local runners.

Current state, measured:
- `.github/workflows/ci.yml` and `docs.yml` already use `runs-on: self-hosted`.
- `dist-workspace.toml` already maps `aarch64-apple-darwin` to `self-hosted`.
- `.github/workflows/release.yml` still has five jobs on `runs-on: "ubuntu-22.04"`: `plan`, `build-global-artifacts`, `host`, `publish-homebrew-formula`, `announce`.

Steps:
- Change those five jobs to `runs-on: self-hosted`. Use the same label as `ci.yml`.
- The runner is a Mac. Check each job for Linux-only steps: the `linuxbrew` PATH line in `publish-homebrew-formula`, `apt` or other Linux package installs, Linux paths, and the dist installer script (it must pick the macOS build). Fix each one so it works on macOS. Do not remove any job and do not change a publish gate.
- `release.yml` is hand-maintained (`allow-dirty = ["ci"]`), so edit it by hand. Keep `dist-workspace.toml` and `release.yml` consistent.
- Check `docs.yml` and any other workflow for `ubuntu-*`, `macos-*` or `windows-*` runners. Fix each.
- Do not add tests that assert on workflow text.

Verification: run `actionlint .github/workflows/*.yml`; no new errors. The release workflow has a `dry-run` tag input. Name it in the report as the real check, to run after the user pushes.

Depends on the card "Drop Linux release targets and llama.cpp leftovers from release config".

Done when: `grep -rn "runs-on" .github/workflows` shows only `self-hosted` and `${{ matrix.runner }}`, and no Linux-only step is left in a job that now runs on the Mac. #remove-llama-cpp