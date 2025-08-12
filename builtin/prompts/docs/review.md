---

title: "review documentation"
description: "Improved the documentation for the project"
---

## Goal

Improve the documentation quality.

{% render "principals" %}
{% render "documentation" %}

## Process

- If there is an existing ./DOCUMENTATION_REVIEW.md
  - Remove any done todo items
- Look over each file in the `./doc` folder
  - Read the file and check that everything is correct
  - Read the code to verify if each part of the documentation is correct
  - Append any errors to correct to the ./DOCUMENTATION_REVIEW.md
  - Ask youself, how can I do a better job on this documentation file?
  - Append any improvements to do to the ./DOCUMENTATION_REVIEW.md
- Look over each source code file
  - Read the file and check that everything is correct
  - Cross reference the documentation to make sure it accurately reflects what the code really does
  - Append any errors to correct to the ./DOCUMENTATION_REVIEW.md
  - Ask youself, how can I do a better job on this documentation file?
  - Append any improvements to do to the ./DOCUMENTATION_REVIEW.md

{% render "review_format" %}
