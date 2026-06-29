Write content to files with atomic operations, creating new files or overwriting existing ones.

A full-file write always replaces the target — new or existing — with the same
unguarded code path. There is no freshness check and no token: whole-file
replacement is the whole point of `write`, and source control is the recovery
path for anything it overwrites. (Lost-update protection lives where it belongs:
line-anchored `edit files`, via the hashline guard.)

## Examples

Create a new file:

```json
{
  "file_path": "/workspace/src/new_module.rs",
  "content": "//! New module\\n\\npub fn hello() {\\n    println!(\\\"Hello, world!\\\");\\n}"
}
```

Overwrite an existing file (clobbers unconditionally):

```json
{
  "file_path": "/workspace/src/existing.rs",
  "content": "//! Updated module\\n"
}
```

## Returns

On a successful write, returns confirmation (`OK`) plus the mutation envelope:
the just-written content re-tagged with hashline anchors (so you can chain the
next `edit files` without re-reading) and the mutated path.
