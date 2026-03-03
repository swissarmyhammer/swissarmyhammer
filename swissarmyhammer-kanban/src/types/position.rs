//! Position types for task ordering using fractional indexing.

use super::ids::{ColumnId, SwimlaneId};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// Full position of a task on the board: column + optional swimlane + ordinal
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub column: ColumnId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub swimlane: Option<SwimlaneId>,
    pub ordinal: Ordinal,
}

impl Position {
    /// Create a new position
    pub fn new(column: ColumnId, swimlane: Option<SwimlaneId>, ordinal: Ordinal) -> Self {
        Self {
            column,
            swimlane,
            ordinal,
        }
    }

    /// Create a position in a column with default ordinal (at the start)
    pub fn in_column(column: ColumnId) -> Self {
        Self {
            column,
            swimlane: None,
            ordinal: Ordinal::first(),
        }
    }
}

/// Ordering within a column/swimlane cell. Uses fractional indexing.
///
/// Ordinals are strings that sort lexicographically to determine display order.
/// This allows inserting between existing items without updating other positions.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Ordinal(String);

impl Ordinal {
    /// Ordinal at the start
    pub fn first() -> Self {
        Self("a0".to_string())
    }

    /// Ordinal after all existing ordinals
    pub fn after(last: &Ordinal) -> Self {
        // Increment the last character or append
        let bytes = last.0.as_bytes();
        let mut result = last.0.clone();

        // Find the last character and increment it
        if let Some(&last_byte) = bytes.last() {
            let new_char = match last_byte {
                b'0'..=b'8' => (last_byte + 1) as char,
                b'9' => {
                    // 9 -> a, but we need to handle rollover
                    result.pop();
                    if result.is_empty() {
                        return Self("b0".to_string());
                    }
                    // Increment the previous character
                    return Self::after(&Ordinal(result)).append_zero();
                }
                b'a'..=b'y' => (last_byte + 1) as char,
                b'z' => {
                    // z -> append 0
                    return Self(format!("{}0", last.0));
                }
                _ => '0',
            };
            result.pop();
            result.push(new_char);
        }

        Self(result)
    }

    /// Helper to append zero
    fn append_zero(self) -> Self {
        Self(format!("{}0", self.0))
    }

    /// Ordinal between two existing ordinals (fractional index)
    pub fn between(before: &Ordinal, after: &Ordinal) -> Self {
        // Simple implementation: find midpoint string
        // For strings of different lengths, pad the shorter one
        let before_bytes = before.0.as_bytes();
        let after_bytes = after.0.as_bytes();

        let max_len = before_bytes.len().max(after_bytes.len());
        let mut result = Vec::with_capacity(max_len + 1);

        for i in 0..max_len {
            let b = before_bytes.get(i).copied().unwrap_or(b'0');
            let a = after_bytes.get(i).copied().unwrap_or(b'z');

            if b < a {
                // Found a position where we can insert
                let mid = b + (a - b) / 2;
                if mid > b {
                    result.push(mid);
                    return Self(String::from_utf8(result).unwrap_or_else(|_| before.0.clone()));
                } else {
                    // Need to go deeper - use current char and continue
                    result.push(b);
                }
            } else {
                result.push(b);
            }
        }

        // Couldn't find midpoint, append a character in the middle
        result.push(b'V'); // Middle of alphabet
        Self(String::from_utf8(result).unwrap_or_else(|_| format!("{}V", before.0)))
    }

    /// Get the inner string value
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl PartialOrd for Ordinal {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Ordinal {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl Default for Ordinal {
    fn default() -> Self {
        Self::first()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ordinal_first() {
        let ord = Ordinal::first();
        assert_eq!(ord.as_str(), "a0");
    }

    #[test]
    fn test_ordinal_after() {
        let first = Ordinal::first();
        let second = Ordinal::after(&first);
        assert!(second > first);

        // Chain multiple
        let third = Ordinal::after(&second);
        assert!(third > second);
        assert!(third > first);
    }

    #[test]
    fn test_ordinal_between() {
        let first = Ordinal::from("a0");
        let third = Ordinal::from("a2");

        let second = Ordinal::between(&first, &third);
        assert!(second > first);
        assert!(second < third);
    }

    #[test]
    fn test_ordinal_ordering() {
        let a = Ordinal::from("a0");
        let b = Ordinal::from("a1");
        let c = Ordinal::from("b0");

        assert!(a < b);
        assert!(b < c);
        assert!(a < c);
    }

    impl From<&str> for Ordinal {
        fn from(s: &str) -> Self {
            Self(s.to_string())
        }
    }
}
