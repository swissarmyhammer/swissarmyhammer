I got this in a log:

2025-07-23T14:01:31.180526Z ERROR swissarmyhammer::common::abort_handler: Detected ABORT ERROR in output, triggering immediate shutdown
2025-07-23T14:01:31.180548Z  INFO swissarmyhammer::workflow::executor::core: Failed: ABORT ERROR: Found ABORT ERROR in output: ABORT ERROR: not on an issue branch

You need to be on an issue branch (like `issue/<issue_name>`) to work on an issue. Please switch to the appropriate issue branch first.


The app * did not shut down *. You didn't do this correctly, the app needs to log and then exit on an ABORT ERROR.

Third time I've asked. Don't make me delete you.
