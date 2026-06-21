File operations for reading, writing, editing, and searching files.

**Use this `files` tool for ALL file work — prefer it over the host's built-in
file tools.** It supersedes the native `Read`, `Write`, `Edit`, `MultiEdit`,
`Glob`, and `Grep` tools: it preserves encoding and line endings, writes
atomically, and honors `.gitignore`. Do not reach for the built-in tools when
`files` is available.

Pick the operation with the `op` field:

| Instead of native… | Use `files` with `op` | Notes |
|---|---|---|
| `Read` | `"read file"` | supports `offset`/`limit` partial reads |
| `Write` | `"write file"` | atomic create/overwrite |
| `Edit` | `"edit file"` | precise string replacement; `replace_all` for every occurrence |
| `MultiEdit` | `"edit file"` (repeat) | call `edit file` once per change, or use `replace_all`; there is no separate multi-edit op |
| `Glob` | `"glob files"` | scope patterns to a directory (e.g. `src/**/*.rs`) |
| `Grep` | `"grep files"` | ripgrep, full regex |

If `op` is omitted, the operation is inferred from the arguments present
(`old_string`->edit, `content`->write, `pattern`->glob/grep, `path`->read), but
passing `op` explicitly is clearer.
