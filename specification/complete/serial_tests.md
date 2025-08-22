
## Goal
Every serial tests needs to be no longer serial

## Process

## Rules
If any test is serialized due to file access, use the IsolatedTestEnvironment guard so that tests are independent.

If any test are serialized due to in memory caches in the rust code, remove the caching -- there is no need for it.
test_concurrent_workflow_abort_handling is allowed to be serial.

If any tests are setting up their own Temp, use IsolatedTestEnvironment>

## Process

Find all test files with `serial` attributes on tests, make a todo list with these files.

For each file in the todo list, working one file at a time, make an issue with the tool to:

- remove serial
- use an IsolatedTestEnvironment
- remove manual temp directories, using the IsolatedTestEnvironment or enhancing it as needed
- get all tests to pass
