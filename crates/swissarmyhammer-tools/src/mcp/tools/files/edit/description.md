Edit a file with forgiving `find`/`replace` pairs, applied as one atomic batch.

Each edit pairs a `find` with a `replace`. The `find` is interpreted with a
small cascade so the same tool handles both byte-exact edits and the looser
descriptions a model tends to emit:

1. **Hashline anchor** — if `find` is shaped like `N:HH` (a 1-based line number
   and a two-hex-digit content hash, e.g. `42:a3`, optionally with a `|text`
   suffix) **and it resolves** (line `N` still hashes to `HH`), the whole
   referenced **line** is replaced with `replace`. A stale anchor — well-formed
   but line `N`'s content changed — is *not* applied as an anchor; it falls
   through and is treated as literal text. These anchors are exactly what
   `read file` emits when it tags lines `N:HH|…`.
2. **Literal substring** — if `find` occurs verbatim in the file, the first
   occurrence (or every occurrence with `replace_all: true`) is replaced.
3. **Recovery** — otherwise the `find` is treated as a description of a span and
   resolved against the file even if it lost its indentation, had its line
   endings normalized, or drifted slightly; the matched **span** is replaced and
   the file's original surrounding bytes (including indentation) are preserved.

Delete by giving an empty `replace`. Insert by replacing a line with itself plus
the new content.

The file's character encoding and line-ending convention are detected and
preserved.

## Ambiguity and `occurrence`

When `replace_all` is false and a `find` has **more than one** confident match
(several lines that resolve the same way, or a `find` that both resolves as a
hashline anchor and occurs as literal text), the edit is *not* a failure: it
returns a **successful** result listing each candidate with its 1-based
`occurrence` index, line number, current text, and a few lines of surrounding
context — and the file is left byte-identical. Re-issue the edit with
`occurrence: N` to apply exactly that candidate, or `replace_all: true` to change
every match. A `find` that matches nothing still fails.

```json
{
  "file_path": "/home/user/project/src/config.rs",
  "find": "log(\"start\")",
  "replace": "log(\"started\")",
  "occurrence": 2
}
```

## Input shapes

A single edit, parallel arrays, or an `edits` array are all accepted, under the
canonical keys or their aliases (`old_string`/`new_string`, `oldText`/`newText`,
`search`/`with`, `from`/`to`, …):

```json
{
  "file_path": "/home/user/project/src/config.rs",
  "find": "const DEBUG: bool = true;",
  "replace": "const DEBUG: bool = false;"
}
```

```json
{
  "file_path": "/home/user/project/src/config.rs",
  "edits": [
    { "find": "42:a3", "replace": "    let timeout = 30;" },
    { "find": "old_name", "replace": "new_name", "replace_all": true }
  ]
}
```

## Atomicity

The whole batch is applied atomically: all pairs are resolved against an
in-memory copy of the file, then committed in a single rewrite. If any pair
fails to resolve, no pair is committed and the file is byte-identical to before.

## Returns

Returns the bytes written, the number of edit operations applied, the detected
encoding, and the preserved line-ending format. Diagnostics for the edited file
are folded into the result when it is a diagnosable source file.
