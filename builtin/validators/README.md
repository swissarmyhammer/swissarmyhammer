# `.validators/`

This directory is the **validator store** for SwissArmyHammer (`sah`).
`sah init` creates and maintains this directory.

## What's here

Each subdirectory is one **validator set**. A validator set is a named
bundle of code-review rules — for example `rust/`, `naming/`,
`no-secrets/`. A set is a folder with a `VALIDATOR.md` file (the set
manifest) and a `rules/` directory of rule files. The review engine
reads this directory directly. Validators are **not** symlinked into
agent directories.

## Customize and override

Validators resolve with this precedence. The **later** entry wins:

    built-in (shipped in sah)  <  user (~/.validators/)  <  this project (./.validators/)

A set or rule in this folder overrides a user-level or built-in set or
rule of the same name. `sah` picks up anything you add here right
away.

- **Add your own** — create `./.validators/<set>/VALIDATOR.md` (and a
  `rules/` directory). `sah init` never touches validators you add.
- **Replace a built-in** — give your set or rule the same name as a
  built-in set or rule. Yours wins by the precedence above.

`sah init` refreshes the built-in validator files on every run. `sah
init` leaves your own files in place. Keep your changes as your own
named set or rule, so they always persist.

## Learn more

Run `sah --help`.
