//! Monotonic ULID generator utility
//!
//! This module provides a thread-safe, monotonic ULID generator that ensures
//! all generated ULIDs are ordered and unique. This replaces the use of
//! `Ulid::new()` which doesn't guarantee monotonic ordering.
//!
//! ## Usage
//!
//! ```rust
//! use swissarmyhammer::common::ulid_generator::generate_monotonic_ulid;
//!
//! let ulid1 = generate_monotonic_ulid();
//! let ulid2 = generate_monotonic_ulid();
//!
//! // Guaranteed that ulid1 < ulid2
//! assert!(ulid1 < ulid2);
//! ```

use std::sync::Mutex;
use std::time::SystemTime;
use ulid::{Generator, Ulid};

/// Global monotonic ULID generator
static ULID_GENERATOR: std::sync::OnceLock<Mutex<Generator>> = std::sync::OnceLock::new();

/// Get the global ULID generator instance
fn get_generator() -> &'static Mutex<Generator> {
    ULID_GENERATOR.get_or_init(|| Mutex::new(Generator::new()))
}

/// Generate a monotonic ULID
///
/// This function ensures that each call returns a ULID that is strictly greater
/// than the previous call, providing monotonic ordering guarantees.
///
/// # Returns
///
/// A new ULID that is guaranteed to be larger than any previously generated ULID
/// from this generator instance.
///
/// # Panics
///
/// This function will panic if the internal mutex is poisoned or if ULID generation
/// fails (which should be extremely rare in practice).
///
/// # Examples
///
/// ```rust
/// use swissarmyhammer::common::ulid_generator::generate_monotonic_ulid;
///
/// let id1 = generate_monotonic_ulid();
/// let id2 = generate_monotonic_ulid();
///
/// // ULIDs are monotonic
/// assert!(id1 < id2);
///
/// // Convert to string for storage/display
/// let id_string = id1.to_string();
/// assert_eq!(id_string.len(), 26);
/// ```
pub fn generate_monotonic_ulid() -> Ulid {
    let generator = get_generator();
    let mut gen = generator.lock().expect("ULID generator mutex poisoned");
    // needs to be from datetime to be strictly monotonic increasing
    gen.generate_from_datetime(SystemTime::now())
        .expect("ULID generation failed")
}

/// Generate a monotonic ULID as a string
///
/// Convenience function that generates a monotonic ULID and immediately
/// converts it to a string representation.
///
/// # Returns
///
/// A string representation of a monotonic ULID (26 characters, base32 encoded)
///
/// # Examples
///
/// ```rust
/// use swissarmyhammer::common::ulid_generator::generate_monotonic_ulid_string;
///
/// let id_str = generate_monotonic_ulid_string();
/// assert_eq!(id_str.len(), 26);
/// ```
pub fn generate_monotonic_ulid_string() -> String {
    generate_monotonic_ulid().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_monotonic_ulid_generation() {
        let id1 = generate_monotonic_ulid();
        let id2 = generate_monotonic_ulid();
        let id3 = generate_monotonic_ulid();

        // Test monotonic ordering
        assert!(id1 < id2);
        assert!(id2 < id3);
        assert!(id1 < id3);

        // Test uniqueness
        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_monotonic_ulid_string_generation() {
        let id1_str = generate_monotonic_ulid_string();
        let id2_str = generate_monotonic_ulid_string();

        // Check length (ULID should be 26 characters)
        assert_eq!(id1_str.len(), 26);
        assert_eq!(id2_str.len(), 26);

        // Check uniqueness
        assert_ne!(id1_str, id2_str);

        // Test ordering (string comparison should match ULID ordering)
        let id1 = Ulid::from_string(&id1_str).unwrap();
        let id2 = Ulid::from_string(&id2_str).unwrap();
        assert!(id1 < id2);
    }

    #[test]
    fn test_ulid_batch_generation() {
        let mut ids = Vec::new();
        for _ in 0..100 {
            ids.push(generate_monotonic_ulid());
        }

        // Check all IDs are unique
        let unique_ids: HashSet<_> = ids.iter().cloned().collect();
        assert_eq!(unique_ids.len(), ids.len());

        // Check monotonic ordering
        for i in 1..ids.len() {
            assert!(ids[i - 1] < ids[i], "IDs not monotonic at index {i}");
        }
    }

    #[test]
    fn test_concurrent_ulid_generation() {
        use std::thread;

        let handles: Vec<_> = (0..10)
            .map(|_| {
                thread::spawn(|| {
                    let mut thread_ids = Vec::new();
                    for _ in 0..10 {
                        thread_ids.push(generate_monotonic_ulid());
                    }
                    thread_ids
                })
            })
            .collect();

        let mut all_ids = Vec::new();
        for handle in handles {
            let thread_ids = handle.join().unwrap();
            all_ids.extend(thread_ids);
        }

        // All IDs should be unique across threads
        let unique_ids: HashSet<_> = all_ids.iter().cloned().collect();
        assert_eq!(unique_ids.len(), all_ids.len());

        // Sort and verify global monotonic property
        all_ids.sort();
        for i in 1..all_ids.len() {
            assert!(all_ids[i - 1] < all_ids[i]);
        }
    }

    #[test]
    fn test_ulid_properties() {
        let ulid = generate_monotonic_ulid();
        let ulid_str = ulid.to_string();

        // ULID should be 26 characters
        assert_eq!(ulid_str.len(), 26);

        // Should be parseable back to ULID
        let parsed = Ulid::from_string(&ulid_str).unwrap();
        assert_eq!(ulid, parsed);

        // Should only contain valid base32 characters
        for c in ulid_str.chars() {
            assert!(
                (c.is_ascii_alphanumeric() && c.is_ascii_uppercase()) || "0123456789".contains(c),
                "Invalid character '{c}' in ULID"
            );
        }
    }
}
