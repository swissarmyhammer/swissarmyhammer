---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff8280
project: local-review
title: 'Make validator deploy consistent with skills: ~/.validators store via shared mirdan mechanism (drop XDG + bespoke deployer)'
---
## What
Validator deployment diverged from the established `~/.<type>/` store convention used by skills/agents/tools. `sah init user` materializes builtin validators to XDG `~/.local/share/validators/` via a **bespoke** `install_profile_validators` routine, and the loader reads the same XDG path. Every other package type uses a home-dotfile store deployed through the one shared `mirdan::store` mechanism. Make validators consistent (decision: `~/.validators/` global + `./.validators/` project, via the shared store path).

### Convention to match
| Package | Global store | Project store | Mechanism |
|---|---|---|---|
| skills | `~/.skills/` | `./.skills/` | shared `store::` (+ symlink into agent dirs) |
| agents | `~/.agents/` | `./.agents/` | shared `store::` (+ symlink) |
| tools | `~/.tools/` | `./.tools/` | shared `store::` (store-only, `deploy_tool`) |
| **validators (target)** | **`~/.validators/`** | `./.validators/` | shared `store::`, **store-only** (no symlink — the engine loader reads the store directly, like tools) |

## Changes

### 1. Relocate the user/global store XDG → home dotfile
- **Loader** `crates/swissarmyhammer-validators/src/validators/loader.rs`: the three user-dir resolutions use `ManagedDirectory::<ValidatorsConfig>::xdg_data()` (→ `~/.local/share/validators`). Switch to `ManagedDirectory::<ValidatorsConfig>::from_user_home()` (→ `~/.validators`, since `ValidatorsConfig::DIR_NAME == ".validators"`). Update the doc comments (`$XDG_DATA_HOME/validators` → `~/.validators`). `from_git_root()` for project (`./.validators`) is already correct — leave it.
- `ValidatorsConfig` in `crates/swissarmyhammer-directory/src/config.rs` needs no field change (DIR_NAME already `.validators`); update its doc comment that currently says `$XDG_DATA_HOME/validators`.
- **`@file_groups` user-include resolution:** the generic `swissarmyhammer-directory` VFS/`YamlExpander`/`file_loader` resolves user includes via `xdg_data()` generically (shared with sah/skills — do NOT change that global behavior). Builtin file_groups are embedded via `add_builtin`, so the common path is unaffected. Verify the existing `@file_groups/source_code` expansion tests still pass; if user-level (`~/.validators`) `@`-include resolution is actually exercised for validators, make it resolve from the same `from_user_home` dir as the loader (do not silently leave includes pointing at XDG while validators load from `~/.validators`).

### 2. Add `store::validators_store_dir(global)`
- In `crates/mirdan/src/store.rs`, add `validators_store_dir(global)` mirroring `skill_store_dir`/`agent_store_dir`/`tool_store_dir`: global `~/.validators`, project `./.validators`. Add the parallel unit tests.

### 3. Route deploy through the shared store mechanism; delete the bespoke deployer
- In `crates/mirdan/src/install.rs`, replace the bespoke `install_profile_validators` / `deinit_profile_validators` / `prune_empty_dirs_up_to` with the shared store path. Model on `deploy_tool` (store-only: copy each builtin set into `validators_store_dir(global)/<set>`, overwrite = reference-copy policy). Reuse `store::` helpers (`copy_dir_recursive`/`remove_if_exists`/`rooted`) — no second copy/prune implementation.
- Keep the **reference-copy semantics** that already exist and are correct: builtin-owned files refreshed/overwritten each install; user-authored validators and user-created sets never touched. Preserve the idempotency + refresh-and-preserve test coverage.
- Point `crate::install::validators_dir(global)` at `~/.validators` (or just delegate to `store::validators_store_dir`). This one function already feeds `list.rs`/`info.rs`/`sync.rs`/`new.rs`, so repointing it fixes those call sites; confirm each reads correctly and no `.avp` / XDG remnant remains.
- `new.rs` `run_new_validator` global path: `~/.validators/<name>` (was `ManagedDirectory::<ValidatorsConfig>::xdg_data()`); keep `AvpConfig`/`avp/tools` for `run_new_tool` untouched.

### 4. Tests
- Loader precedence test currently sets a temp `XDG_DATA_HOME`; switch it to a temp `HOME` (so `from_user_home` resolves into the temp), keeping the CWD guard for `./.validators`. Same for any mirdan test asserting the global validators path — assert it ends with `~/.validators`, not `~/.local/share/validators`, with the temp-HOME isolation and `#[serial]`.
- Keep/adapt the deploy + idempotency/refresh-and-preserve tests through the shared path.

## Acceptance Criteria
- [ ] Global validator store is `~/.validators/`; project is `./.validators/`. No code (loader, mirdan, review tool, doctor) resolves the user validators dir to `~/.local/share/validators` / XDG anymore.
- [ ] Builtin validators deploy through the shared `mirdan::store` mechanism (store-only, like `deploy_tool`); the bespoke `install_profile_validators`/`deinit_profile_validators`/`prune_empty_dirs_up_to` routines are gone (or collapsed onto the shared helpers). No second deploy/prune implementation remains.
- [ ] Reference-copy policy preserved: re-running refreshes builtin-owned files; user-authored validators and user-created sets survive.
- [ ] Loader precedence builtin → user (`~/.validators`) → project (`./.validators`) still holds; `@file_groups/source_code` expansion still resolves.
- [ ] `sah init user` materializes builtin validators into `~/.validators/`; `cargo test -p swissarmyhammer-validators`, `-p mirdan`, `-p swissarmyhammer-directory` green; workspace builds; clippy clean.

## Tests
- [ ] Temp-HOME deploy test: `sah`/mirdan init (user scope) writes `~/.validators/<set>/VALIDATOR.md` (+ rules) matching embedded source.
- [ ] Idempotency/refresh-and-preserve test through the shared store path.
- [ ] Loader precedence test (temp HOME user + temp CWD project) green.

## Workflow
- Use `/tdd` for the relocated-path + shared-deploy tests. REUSE `mirdan::store` — do not write a parallel deployer (this is the whole point). After landing, the stale `~/.local/share/validators/` from prior installs is orphaned; note it for manual cleanup (a re-init writes the new location, it does not remove the old).

## Review Findings (2026-06-06 08:03)

Verification (all fresh, this review):
- `cargo test -p swissarmyhammer-validators -p mirdan -p swissarmyhammer-directory` — green (mirdan 394 passed / 0 failed; validators 216 / 0; directory doc-tests 6 / 0).
- `cargo test -p swissarmyhammer-tools --lib review::` — 9 passed / 0 failed.
- `cargo build --workspace` — exit 0.
- `cargo clippy -p mirdan -p swissarmyhammer-validators -p swissarmyhammer-directory -p swissarmyhammer-tools --all-targets` — clean, 0 warnings.

Acceptance criteria: all four functional criteria are MET. `~/.validators` global + `./.validators` project; no XDG validator-store remnants (grep over loader/mirdan/review-tool/doctor finds only one accurate doc comment plus the intentionally-untouched `AvpConfig`/`run_new_tool` `.avp/tools` path); bespoke `install_profile_validators`/`deinit_profile_validators`/`prune_empty_dirs_up_to` collapsed onto `validators_store_dir` + `copy_dir_recursive` + `store::remove_if_exists` (no parallel deployer/prune remains); reference-copy refresh-and-preserve proven by the passing idempotency test through the new path; loader precedence + `@file_groups/source_code` expansion proven by passing tests.

### Warnings
- [x] **Loader resolves `~/.validators` via the deprecated `ManagedDirectory::<ValidatorsConfig>::from_user_home()` under `#[allow(deprecated)]` — the deprecation note steers validators toward XDG, the exact thing this task reverses.** RESOLVED (2026-06-06): the loader now resolves the user store via a private `user_validators_dir()` helper that does `dirs::home_dir().map(|h| h.join(ValidatorsConfig::DIR_NAME))` — the same raw-home mechanism `mirdan::store::validators_store_dir` uses, so both resolve `~/.validators` through one mechanism. All three sites (`load_all`, `get_directories`, `diagnostics`) call the helper; every `#[allow(deprecated)]`, `from_user_home`, and `xdg_data` is gone from the loader's user-dir resolution (grep-confirmed: no matches). `from_git_root()` for the project dir (`./.validators`) is unchanged. `dirs` was added to `swissarmyhammer-validators/Cargo.toml` (workspace dep). The generic `swissarmyhammer-directory` VFS/`xdg_data` behavior (shared with sah/skills) was NOT touched.

### Nits
- [x] **`deinit_profile_validators` empty-dir cleanup is now hardcoded to the two-level builtin layout rather than generic.** RESOLVED (2026-06-06): replaced the inline `remove_dir(parent)` + trailing `remove_dir(target_root.join(set))` with a generic `remove_empty_dirs_up_to(start, boundary)` helper in `crates/mirdan/src/install.rs` that climbs from each removed file's parent up toward `target_root` (exclusive), removing each empty directory and halting at the first non-empty ancestor (the `remove_dir`-fails-on-non-empty guard preserves user files). This generalizes to builtin sets nested more than one subdir deep. Covered by two new TDD unit tests (`remove_empty_dirs_up_to_climbs_arbitrary_depth_and_stops_at_nonempty`, `remove_empty_dirs_up_to_halts_at_first_nonempty_ancestor`) written RED-first; the existing `init_profile_validators_idempotent_refreshes_builtin_preserves_user` test still proves refresh-and-preserve through the new path. mirdan now 396 passed / 0 failed.