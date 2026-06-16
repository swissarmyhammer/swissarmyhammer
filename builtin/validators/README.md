# `.validators/`

The **validator store** for SwissArmyHammer (`sah`). This directory is created
and maintained by `sah init`.

## What's here

Each subdirectory is one **validator set** — a named bundle of code-review rules
(for example `rust/`, `naming/`, `no-secrets/`). A set is a folder with a
`VALIDATOR.md` (the set manifest) plus a `rules/` directory of rule files. The
review engine reads this directory directly — validators are **not** symlinked
into agent directories.

## Customize and override

Validators resolve with this precedence — **later wins**:

    built-in (shipped in sah)  <  user (~/.validators/)  <  this project (./.validators/)

A set or rule in this folder therefore overrides a user-level or built-in one of
the same name, and anything you add here is picked up immediately.

- **Add your own** — create `./.validators/<set>/VALIDATOR.md` (and `rules/`).
  Validators you add are never touched by `sah init`.
- **Replace a built-in** — give your set or rule the same name as a built-in;
  yours wins by the precedence above.

`sah init` refreshes the built-in validator files on every run but leaves your
own files in place, so keep your changes as your own named set or rule so they
always persist.

## Learn more

Run `sah --help`.
