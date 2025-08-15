OK -- bad n

Two problems:


```
2025-07-21T20:45:13.778591Z  INFO swissarmyhammer::workflow::executor::core: Failed: Claude command failed: Failed to render prompt 'review/branch': 2025-07-21T20:45:13.745066Z  INFO swissarmyhammer: Running prompt command
2025-07-21T20:45:13.777794Z ERROR swissarmyhammer::common::abort_handler: Detected ABORT ERROR in output, triggering immediate shutdown
2025-07-21T20:45:13.777886Z ERROR swissarmyhammer::error: Error: Prompt execution aborted: ABORT ERROR: Found ABORT ERROR in output:

- If you are on the main branch
  - return an ABORT ERROR: branch review is not available on the main branch
- If there is an existing ./CODE_REVIEW.md
  - Remove any done todo items
```

DO NOT -- look at the result of a rendered prompt for ABORT ERROR, how else would we instruct LLM to abort?

AND

you didn't actually abort, you just logged an error. I really mean abort, log an error and exit the program.