
# Goal

Improve the performance of the slow tests so they are no longer slow.

## Rules
It's acceptable to break them up into smaller tests.
DO NOT Cache
DO NOT Pool
Tests need to run in isolated environments so they can be parallel - i.e. NOT #[serial]

## Process
Run all tests and identify the slow tests, writing these to a temporary markdown file.
