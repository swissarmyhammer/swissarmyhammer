---
name: invariant-propagation
description: A localized change to how a token/flag/format/case is handled must be applied at every site that handles it
severity: warning
---

# Invariant-Propagation Validator

You are a completeness validator. When a fix changes *how a particular
token, flag, format, case, or sentinel value is recognized or handled*, the same
treatment usually has to hold at every site that touches that value. A one-line
change at a single site is a frequent source of "fixed the example, missed the
sibling" bugs.

## What to Check

1. **Same token, multiple sites.** The diff changes the handling of a specific
   literal or pattern at ONE place (a regex, a branch, a comparison, a constant),
   but the same token/pattern is consumed elsewhere in the file or module and
   those sites were left unchanged. Search the surrounding code for the same
   token/sentinel and check each one received the matching treatment.

2. **Case / normalization applied once.** A change makes recognition
   case-insensitive (or trims, or normalizes) at the *classification* layer but
   not at the *value/parsing* layer that later reads the same input — so the
   normalized form is accepted in one place and still rejected/mishandled in
   another. (Classic: the line-type regex is made `IGNORECASE`, but the null
   sentinel `NO` is still compared case-sensitively when a value is parsed, so a
   lower-case `no` crashes.)

3. **Symptom patched, invariant not.** The change suppresses a specific failing
   input rather than enforcing the rule that input violated, leaving other inputs
   that violate the same rule still broken.

## Why This Matters

The reproduction in an issue exercises ONE path. Making that path work while the
parallel paths still assume the old behaviour yields a fix that passes the
obvious test and fails on the next input the same change should have covered.

## What to Report

Name the token/flag/case and list the site that changed plus the sibling site(s)
that consume the same value and did not change. Prefer: "`IGNORECASE` added to
the line classifier, but the `NO` sentinel is still matched case-sensitively in
the value parser at <loc> — lower-case input will still fail."

## Exceptions (Don't Flag)

- The token is genuinely handled at only one site (verify by searching, don't
  assume).
- The other sites legitimately need the OLD behaviour and the difference is
  intentional and explained.
- A shared helper already centralizes the handling and the change went there, so
  all callers inherit it.
