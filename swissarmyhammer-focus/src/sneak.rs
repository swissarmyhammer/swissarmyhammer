//! Sneak code generator — short, prefix-free key codes for the Jump-To
//! overlay (vim-sneak / jumpy / AceJump-style labels).
//!
//! Produces lowercase ASCII strings drawn from an ergonomic 23-letter
//! [`SNEAK_ALPHABET`]. Codes are ordered by ergonomic priority — shortest
//! and easiest first — and are guaranteed to be prefix-free so the
//! consumer can disambiguate inputs incrementally without lookahead.
//!
//! The algorithm is pure — no I/O, no `unsafe`, no dependencies beyond
//! [`thiserror`] for the error type. It lives here in
//! `swissarmyhammer-focus` because spatial-nav vocabulary in this
//! workspace is Rust-authoritative (see [`crate::types::Direction`] and
//! [`crate::types::FullyQualifiedMoniker`]); putting it here means any
//! future consumer (mirdan-app, CLI front-ends, tests) gets it via dep
//! rather than copy-pasting TypeScript.

use thiserror::Error;

/// Ergonomic alphabet, ordered by priority — home row first
/// (`a s d f j k g h`), then top row (`w e r u p q t y`), then bottom
/// row (`z x c v n m b`).
///
/// Skips letters with high visual confusion (`i`/`1`/`l`, `o`/`0`) and
/// the four corner letters that require an awkward stretch on a
/// staggered keyboard. The result is 23 letters — comfortably above the
/// number of jump targets the overlay realistically presents while
/// keeping the average code short.
///
/// Each letter appears exactly once; iteration order is the priority
/// order.
pub const SNEAK_ALPHABET: &[char] = &[
    // Home row — index fingers and middle fingers on both hands.
    'a', 's', 'd', 'f', 'j', 'k', 'g', 'h', // Top row — same fingers, one row up.
    'w', 'e', 'r', 'u', 'p', 'q', 't', 'y', // Bottom row — same fingers, one row down.
    'z', 'x', 'c', 'v', 'n', 'm', 'b',
];

/// Errors returned by [`generate_sneak_codes`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SneakError {
    /// The caller asked for more codes than the alphabet can encode in
    /// at most two letters.
    ///
    /// `requested` is the requested count; `capacity` is the maximum
    /// (`SNEAK_ALPHABET.len().pow(2)`). Hitting this in practice
    /// indicates an upstream bug — the Jump-To overlay never presents
    /// hundreds of targets.
    #[error("too many jump targets: {requested} exceeds capacity {capacity}")]
    TooManyTargets {
        /// Number of codes the caller requested.
        requested: usize,
        /// Maximum number of codes the generator can produce
        /// (`MAX_SNEAK_CODES`).
        capacity: usize,
    },
}

/// Maximum number of codes [`generate_sneak_codes`] can produce.
///
/// Equal to `SNEAK_ALPHABET.len().pow(2)` — the size of the all-pairs
/// two-letter space. With a 23-letter alphabet this is 529, well above
/// any realistic Jump-To target count.
pub const MAX_SNEAK_CODES: usize = SNEAK_ALPHABET.len() * SNEAK_ALPHABET.len();

/// Generate `count` distinct, prefix-free key codes drawn from
/// [`SNEAK_ALPHABET`].
///
/// Codes are ordered by ergonomic priority — single-letter codes first
/// (in alphabet order), then two-letter codes (also in alphabet order
/// of the prefix letter, then of the second letter). Returned codes
/// are lowercase ASCII strings.
///
/// # Algorithm
///
/// The alphabet is partitioned into two disjoint buckets:
///
/// - **Single-letter prefixes** (`S` = first `alphabet.len() - k`
///   letters) — each emitted as a one-letter code.
/// - **Two-letter prefixes** (`P` = last `k` letters) — each combined
///   with every letter of the alphabet to form `k * alphabet.len()`
///   two-letter codes.
///
/// `k` is the smallest value such that
/// `(alphabet.len() - k) + k * alphabet.len() >= count`. The two
/// buckets are disjoint by construction, so no single-letter code is a
/// prefix of any two-letter code, and (since each letter pair is
/// unique) no two-letter code is a prefix of another.
///
/// # Errors
///
/// Returns [`SneakError::TooManyTargets`] when `count > MAX_SNEAK_CODES`.
///
/// # Examples
///
/// ```
/// use swissarmyhammer_focus::generate_sneak_codes;
///
/// let codes = generate_sneak_codes(4).unwrap();
/// assert_eq!(codes, vec!["a", "s", "d", "f"]);
/// ```
pub fn generate_sneak_codes(count: usize) -> Result<Vec<String>, SneakError> {
    if count == 0 {
        return Ok(Vec::new());
    }
    if count > MAX_SNEAK_CODES {
        return Err(SneakError::TooManyTargets {
            requested: count,
            capacity: MAX_SNEAK_CODES,
        });
    }

    let alphabet_len = SNEAK_ALPHABET.len();
    let k = pick_two_letter_prefix_count(count, alphabet_len);
    let single_count = alphabet_len - k;

    let mut codes = Vec::with_capacity(count);

    // Single-letter codes — first `single_count` letters of the
    // alphabet. Skipped entirely when `k == alphabet_len` (max capacity).
    for &letter in SNEAK_ALPHABET.iter().take(single_count) {
        if codes.len() == count {
            return Ok(codes);
        }
        codes.push(letter.to_string());
    }

    // Two-letter codes — each prefix from the last `k` letters of the
    // alphabet combined with every letter of the alphabet, in order.
    for &prefix in SNEAK_ALPHABET.iter().skip(single_count) {
        for &second in SNEAK_ALPHABET.iter() {
            if codes.len() == count {
                return Ok(codes);
            }
            let mut s = String::with_capacity(2);
            s.push(prefix);
            s.push(second);
            codes.push(s);
        }
    }

    Ok(codes)
}

/// Pick the smallest `k` (number of two-letter prefix letters) such
/// that the resulting capacity covers `count`.
///
/// Capacity for a given `k` is
/// `(alphabet_len - k) + k * alphabet_len`, which simplifies to
/// `alphabet_len + k * (alphabet_len - 1)`. Solving for the minimum
/// `k`:
///
/// - `k = 0` works when `count <= alphabet_len` (single-letter codes
///   only).
/// - Otherwise `k = ceil((count - alphabet_len) / (alphabet_len - 1))`.
///
/// Caller must ensure `count <= MAX_SNEAK_CODES`.
fn pick_two_letter_prefix_count(count: usize, alphabet_len: usize) -> usize {
    if count <= alphabet_len {
        return 0;
    }
    // Ceiling division — the smallest k that satisfies
    // `alphabet_len + k * (alphabet_len - 1) >= count`.
    let extra = count - alphabet_len;
    let denom = alphabet_len - 1;
    extra.div_ceil(denom)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `generate_sneak_codes(0)` returns an empty vec.
    #[test]
    fn generates_empty_for_zero_count() {
        let codes = generate_sneak_codes(0).expect("zero count must succeed");
        assert!(codes.is_empty(), "expected empty vec, got {:?}", codes);
    }

    /// For a representative range of counts, every returned pair is
    /// distinct.
    #[test]
    fn generates_distinct_codes() {
        for n in [1, 5, 10, 23, 50, 200, 500] {
            let codes = generate_sneak_codes(n)
                .unwrap_or_else(|e| panic!("generate_sneak_codes({n}) failed: {e}"));
            assert_eq!(codes.len(), n, "wrong length for n={n}");
            let mut seen: std::collections::HashSet<&str> =
                std::collections::HashSet::with_capacity(n);
            for c in &codes {
                assert!(seen.insert(c.as_str()), "duplicate code {c:?} at n={n}");
            }
        }
    }

    /// No code in the returned vec is a prefix of any other.
    ///
    /// Brute-force pair check — for each pair `(a, b)` with `a != b`,
    /// neither `a.starts_with(b)` nor `b.starts_with(a)`.
    #[test]
    fn prefix_free_invariant() {
        for n in [1, 5, 10, 23, 50, 200, 500] {
            let codes = generate_sneak_codes(n)
                .unwrap_or_else(|e| panic!("generate_sneak_codes({n}) failed: {e}"));
            for (i, a) in codes.iter().enumerate() {
                for (j, b) in codes.iter().enumerate() {
                    if i == j {
                        continue;
                    }
                    assert!(
                        !a.starts_with(b.as_str()),
                        "code {a:?} starts with {b:?} (n={n}, indices {i}/{j})",
                    );
                }
            }
        }
    }

    /// For `N=4`, returned codes match the first 4 letters of
    /// [`SNEAK_ALPHABET`] — the home-row priority.
    #[test]
    fn single_letter_codes_use_home_row_first() {
        let codes = generate_sneak_codes(4).expect("count=4 must succeed");
        let expected: Vec<String> = SNEAK_ALPHABET
            .iter()
            .take(4)
            .map(|c| c.to_string())
            .collect();
        assert_eq!(codes, expected);
    }

    /// Asking for one more than the maximum capacity returns
    /// [`SneakError::TooManyTargets`].
    #[test]
    fn errors_when_count_exceeds_capacity() {
        let over = MAX_SNEAK_CODES + 1;
        let result = generate_sneak_codes(over);
        match result {
            Err(SneakError::TooManyTargets {
                requested,
                capacity,
            }) => {
                assert_eq!(requested, over);
                assert_eq!(capacity, MAX_SNEAK_CODES);
            }
            other => panic!("expected TooManyTargets, got {other:?}"),
        }
    }

    /// Boundary check — the maximum count itself succeeds and produces
    /// exactly `MAX_SNEAK_CODES` distinct codes.
    #[test]
    fn generates_at_max_capacity() {
        let codes = generate_sneak_codes(MAX_SNEAK_CODES).expect("max capacity must succeed");
        assert_eq!(codes.len(), MAX_SNEAK_CODES);
        let unique: std::collections::HashSet<&str> = codes.iter().map(String::as_str).collect();
        assert_eq!(unique.len(), MAX_SNEAK_CODES);
    }
}
