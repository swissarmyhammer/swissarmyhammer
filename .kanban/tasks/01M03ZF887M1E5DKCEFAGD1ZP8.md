---
assignees:
- claude-code
position_column: todo
position_ordinal: ffe880
title: deinit leaves validator content that an older binary wrote
---
`deinit_profile_validators` in `crates/mirdan/src/install/profile.rs` walks the CURRENT embedded roster to decide what to remove. So it clears every set the current binary installs, and it leaves behind any content that only an OLDER binary wrote.

## Measured

On a throwaway HOME:

1. `sah init user` deployed 13 validator sets, 156 files.
2. Two files of the pre-merge `no-secrets` set were put beside them, which is what a store written before the nine-set merge holds.
3. `sah deinit user` removed the 156 files and left `.validators/no-secrets/VALIDATOR.md` and `.validators/no-secrets/rules/no-secrets.md`.

For a store that holds only what the current binary wrote, deinit removes the `.validators` directory itself.

## Why this matters now

^35tgz1c deleted the retired-validator mechanism, on the decision that "we SHOULD NOT have 'retirement' code at all, we have deinit." That decision stands. This card does not ask for the snapshot mechanism back — a byte-frozen copy of every file ever deleted is the cost the decision refused.

But the premise "deinit clears a store" is true only for a store the current binary wrote. A user who installed an older version and then upgraded keeps running any rule that version deployed, and deinit does not remove it.

## What to do

Make deinit clear the validator content it did not write, not only the sets the current roster names. Read the store rather than the embed. Decide what "the store" means from the code: the `.validators` directory that mirdan itself deploys.

Weigh which shape is right, and state the reason:
- Remove the whole `.validators` directory, since mirdan owns it.
- Remove every set directory in the store, whichever binary wrote it.

Consider what happens to a file a user edited or added by hand. State the answer rather than leaving it to chance.

## Done when

- `sah deinit` clears a store that holds a set no current roster names, measured against a throwaway HOME.
- A test holds the behaviour, so it cannot regress.
- `cargo nextest run --workspace` green; fmt and clippy clean.

## Found by

The reviewer of `6b6fe8cf1` (^35tgz1c). Correctly NOT recorded as a finding there: `deinit_profile_validators` stands on no line of that diff, and a defect on an unchanged line is not a finding under a diff-scoped review.

#tool-validators