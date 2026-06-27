Edit a file with forgiving `find`/`replace` pairs, applied as one atomic batch.

Each edit pairs a `find` with a `replace`. The `find` is interpreted with a
small cascade so the same tool handles both byte-exact edits and the looser
descriptions a model tends to emit:

1. **Hashline anchor** — if `find` is shaped like `N:HH` (a 1-based line number
   and a two-hex-digit content hash, e.g. `42:a3`, optionally with a `|text`
   suffix) **and it resolves**, the whole referenced **line** is replaced with
   `replace`. Resolution tolerates small drift: line `N` is tried first, and if
   it no longer hashes to `HH`, nearby lines (up to 50 on each side) are searched
   for the nearest one that does, so an anchor still applies after the file
   shifts by a few lines. When a `|text` suffix is present it verifies and
   relocates the anchor — the in-window line whose text matches `|text` is
   preferred. A *stale* anchor — no line within that window hashes to `HH` — is
   *not* applied as an anchor; it falls through and is treated as literal text.
   These anchors are exactly what `read file` emits when it tags lines `N:HH|…`.
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
every match.

## Safety and idempotency

These guard against common no-op and re-run mistakes. None of them mutate the
file:

- **No-op rejection** — an edit whose `find` and `replace` are identical changes
  nothing and is rejected up front with a clear error.
- **Already applied** — if `find` is absent but `replace` is already present, the
  edit was very likely already applied. This returns a **successful**
  informational result saying so (not a hard "not found" error); the file is left
  unchanged.
- **Consumed target** — in a multi-pair batch, if a later pair's target was
  overwritten by an earlier pair in the *same* batch, that pair returns a
  **successful** per-edit message naming the consumed target, instead of a generic
  miss. The batch stays atomic, so the file is byte-identical.

A `find` that matches nothing else still returns a structured near-miss (the
nearest current text plus a diff) rather than a bare failure.

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
