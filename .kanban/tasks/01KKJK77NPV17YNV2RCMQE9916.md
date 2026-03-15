---
position_column: done
position_ordinal: ffffc280
title: ElectionOutcome, LeaderGuard, FollowerGuard missing Debug derive
---
swissarmyhammer-leader-election/src/election.rs:68, 235, 265\n\nThe three new/modified public types `ElectionOutcome`, `LeaderGuard`, and `FollowerGuard` have no `#[derive(Debug)]`. Per the Rust review guidelines, all public types must implement `Debug`. `ElectionConfig` correctly derives `Debug` and `Clone`; the other types do not.\n\n`LeaderGuard` cannot derive `Debug` automatically (it contains `File`), but a manual impl is trivial: output \"LeaderGuard\" as the struct name. `FollowerGuard` can derive it. `ElectionOutcome` can derive it if both variants implement `Debug`.\n\nSuggestion: Add `#[derive(Debug)]` to `FollowerGuard`. Add manual `impl Debug for LeaderGuard` and `impl Debug for ElectionOutcome`.",
<parameter name="tags">["review-finding"] #review-finding