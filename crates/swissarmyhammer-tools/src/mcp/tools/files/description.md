File operations for reading, writing, editing, and searching files.

**Use this `files` tool for ALL file work.** In this environment the native
`Read`, `Write`, `Edit`, `Glob`, and `Grep` tools are **disabled / denied** —
attempting one is rejected and wastes a turn. Call this `files` tool directly
with the right `op` from the start; do **not** try a native tool first and wait
to be redirected. It supersedes those tools: it preserves encoding and line
endings, writes atomically, and honors `.gitignore`.

Pick the operation with the `op` field:

| Instead of native… | Use `files` with `op` | Notes |
|---|---|---|
| `Read` | `"read file"` | supports `offset`/`limit` partial reads |
| `Write` | `"write file"` | atomic create/overwrite |
| `Edit` | `"edit file"` | precise string replacement; `replace_all` for every occurrence |
| `Glob` | `"glob files"` | scope patterns to a directory (e.g. `src/**/*.rs`) |
| `Grep` | `"grep files"` | ripgrep, full regex |

If `op` is omitted, the operation is inferred from the arguments present
(`old_string`->edit, `content`->write, `pattern`->glob/grep, `path`->read), but
passing `op` explicitly is clearer.
