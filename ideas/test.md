I want to make 'sah test' a top level command.

I in order to do this we need to:

- rename builting/workflows/tdd.md to be test.md
- implment a new cli command inspired by `sah implement`

Keep this really simple for now. We just need to be able to run `sah test` and have it run the tdd workflow similar to how `sah implement` runs the implement workflow.
