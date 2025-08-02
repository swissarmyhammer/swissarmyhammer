---
title: "review code"
description: "Improved the current code changes"
---

## Guidelines

{% render "principals" %}
{% render "coding_standards" %}

DO NOT code now, the goal is just to create `./CODE_REVIEW.md` todo items.

## Process

{% render "issue/on_worktree" %}
- If there is an existing ./CODE_REVIEW.md
  - Remove any done todo items
- Focus on the files that have changed just on the current branch, this is your working set
- For each file in the working set
  - Keep track of each file in this working set in your
  - Think deeply about the issue, does the file do a good job resolving the issue?
  - Are there any placeholders that need to be actually coded, if so add them to ./CODE_REVIEW.md
  - Are there any TODO commments that need to be actually coded, if so add them to ./CODE_REVIEW.md
  - Append any improvement ideas to do to ./CODE_REVIEW.md
  - Run a language appropriate lint
    - Append any lint warnings or errors to do to ./CODE_REVIEW.md
- Report your progress

{% render "review_format" %}
