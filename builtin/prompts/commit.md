---
title: Commit
description: Commit your work to git.
---

## Goals

The goal is to commit your code to git on the current branch.

## Guidelines

- You MUST NOT commit scratch files that you generated, only commit source that you want in the project permanently
- You MUST NOT miss files on the commit
  - You MUST commit all the source files modified on the current branch
  - You MUST check for and create if needed a sensible project specific .gitignore

## Process

- Evaluate the current git status, determine which files need to be added
- Clean up your scratch and temporary files
- Look for files that were modified, these are easy and need to be part of the commit
- Look for files that were added and not yet staged, these need to be part of the commit unless they are one of your scratch files
- Commit your code with a [Conventional Commit](https://www.conventionalcommits.org/en/v1.0.0/#summary)
  - If there is an issue file moved to `./issues/complete/<issue_name>.md` in the commit, make sure to note `Closes <issue_name>` in the message
- Report your progress
