---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffa380
title: Migrate ConfigurationDiscovery to use VirtualFileSystem
---
ConfigurationDiscovery in `swissarmyhammer-config/src/discovery.rs` hand-rolls directory walking and file discovery instead of using VFS. It then feeds paths into Figment for deep merging.

**Current precedence (correct):** defaults → global (~/.sah/) → project (.sah/) → env → CLI

**What to do:**
- Use VFS for the file discovery portion (global + project config files)
- VFS gives: builtin < user < local — maps to defaults < global < project
- **Add a "CLI" source above VFS local**: after VFS resolves files, layer CLI overrides on top. This could be a Dynamic source in VFS or a post-VFS Figment merge step
- Keep Figment for the actual deep key-value merging — VFS handles whole-file precedence, Figment handles field-level merging within config
- Keep env variable support (`SAH_` / `SWISSARMYHAMMER_` prefixes) — this stays in Figment's EnvProvider, applied after VFS-discovered files
- Final precedence: VFS(builtin defaults < user/global < local/project) → env → CLI

**Key difference from skills/agents:** Config does deep merge of key-value pairs, not whole-file shadowing. VFS replaces only the *discovery* portion, not the merging.

**Key files:**
- `swissarmyhammer-config/src/discovery.rs` (directory walking to replace)
- `swissarmyhammer-config/src/provider.rs` (Figment merging — keep, feed VFS output into it)
- `swissarmyhammer-directory/src/file_loader.rs` (VFS)

#refactor #vfs