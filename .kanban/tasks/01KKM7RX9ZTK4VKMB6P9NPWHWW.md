---
assignees:
- assistant
position_column: done
position_ordinal: ffffff8580
title: 'Refactor DirectoryConfig: rename .swissarmyhammer → .sah, add XDG support'
---
## What
Rewrite the `swissarmyhammer-directory` crate to:
1. Rename `SwissarmyhammerConfig::DIR_NAME` from `.swissarmyhammer` to `.sah` — the ONE place the app name string appears
2. Add XDG Base Directory support to `ManagedDirectory` — new constructors: `xdg_config()`, `xdg_data()`, `xdg_cache()`
3. **Keep** `AvpConfig`, `ShellConfig`, `CodeContextConfig` as separate DirectoryConfig impls — they are independent subsystems, NOT nested under `.sah/`
4. Each subsystem gets its own XDG namespace (e.g., `$XDG_CONFIG_HOME/avp/`, `$XDG_DATA_HOME/shell/`)
5. Replace `from_user_home()` with XDG-compliant constructors so nothing puts dot-dirs in `~/` directly
6. Keep `from_git_root()` for project-local dirs — XDG doesn't apply there

### XDG path layout (per subsystem)
```
$XDG_CONFIG_HOME/sah/           # config files (sah.toml, etc.)
$XDG_DATA_HOME/sah/             # persistent data (prompts, workflows)
$XDG_CACHE_HOME/sah/            # ephemeral cache (tmp, transcripts)
$XDG_CONFIG_HOME/avp/           # AVP config
$XDG_DATA_HOME/avp/             # validators
$XDG_DATA_HOME/shell/           # shell security configs
$XDG_CACHE_HOME/code-context/   # code index DB
{git_root}/.sah/                # project-local (not XDG)
{git_root}/.avp/                # project-local validators
```

### Key files
- `swissarmyhammer-directory/src/config.rs` — rename DIR_NAME to `.sah`
- `swissarmyhammer-directory/src/directory.rs` — add XDG constructors, deprecate `from_user_home()`
- `swissarmyhammer-directory/src/lib.rs` — update re-exports
- `swissarmyhammer-directory/src/file_loader.rs` — update VFS default user-level path resolution

## Acceptance Criteria
- [ ] `.sah` string literal appears once in `SwissarmyhammerConfig::DIR_NAME`
- [ ] XDG constructors added to `ManagedDirectory`
- [ ] `from_user_home()` deprecated or removed
- [ ] Each DirectoryConfig impl keeps its own identity
- [ ] All tests updated and passing

## Tests
- [ ] Unit tests verify DIR_NAME = \".sah\"
- [ ] Unit tests for XDG constructors with env var overrides
- [ ] Unit tests for fallback when XDG vars not set
- [ ] `cargo nextest run -p swissarmyhammer-directory`