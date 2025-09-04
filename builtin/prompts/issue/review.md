---
title: "review code"
description: "Improved the current code changes"
---

## Rules

DO NOT code now, the goal is just to create `./CODE_REVIEW.md` todo items.

{% render "workflow_guards" %}

## Process

- If there is an existing ./CODE_REVIEW.md
  - Remove any done todo items
- Focus on the files that have changed just on the current branch, this is your working set
- For each file in the working set
  - Keep track of each file in this working set in your todo list
  - Think deeply about the issue, does the file do a good job resolving the issue?
  - Are there any placeholders/TODO/not yet implemented that need to be actually coded, if so add them to ./CODE_REVIEW.md
  - Are there any violations of coding standards, if so add them to ./CODE_REVIEW.md
  - Are there any missing tests, if so add them to ./CODE_REVIEW.md
  - Are there any missing documentation comments, if so add them to ./CODE_REVIEW.md
  - Are there any missing comments, if so add them to ./CODE_REVIEW.md
  - Were any tests ignored, commented out, featured flagged away, or otherwise not run, if so add them to ./CODE_REVIEW.md
  - Append any improvement ideas to do to ./CODE_REVIEW.md
  - Run a language appropriate lint
    - Append any lint warnings or errors to do to ./CODE_REVIEW.md
- DO NOT commit to git
- DO NOT mark an issue complete with the issue_mark_complete tool
- Report your progress

{% render "review_format" %}
