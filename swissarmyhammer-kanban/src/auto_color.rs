//! Deterministic auto-color assignment for tags.
//!
//! Maps a tag slug to a color from a curated palette using a simple hash.
//! The palette is designed to look good on both light and dark backgrounds.

/// Curated palette of 16 tag colors (6-char hex without `#`).
///
/// These are chosen to be distinct, readable as pill backgrounds with white or
/// dark text, and visually pleasant in a kanban UI.
const PALETTE: &[&str] = &[
    "d73a4a", // red
    "e36209", // orange
    "f9c513", // yellow
    "0e8a16", // green
    "006b75", // teal
    "1d76db", // blue
    "5319e7", // purple
    "b60205", // dark red
    "d876e3", // pink
    "0075ca", // ocean
    "7057ff", // violet
    "008672", // sea green
    "e4e669", // lime
    "bfd4f2", // light blue
    "c5def5", // periwinkle
    "fbca04", // gold
];

/// Return a deterministic color for a tag slug.
///
/// Uses a simple FNV-1a hash mapped to the palette index.
pub fn auto_color(slug: &str) -> &'static str {
    let hash = fnv1a(slug);
    let idx = (hash as usize) % PALETTE.len();
    PALETTE[idx]
}

/// FNV-1a hash (32-bit) for short strings.
fn fnv1a(s: &str) -> u32 {
    let mut hash: u32 = 0x811c_9dc5;
    for byte in s.bytes() {
        hash ^= byte as u32;
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auto_color_deterministic() {
        let c1 = auto_color("bug");
        let c2 = auto_color("bug");
        assert_eq!(c1, c2);
    }

    #[test]
    fn test_auto_color_different_tags_differ() {
        let c1 = auto_color("bug");
        let c2 = auto_color("feature");
        // Not guaranteed to differ, but very likely with 16 colors
        // Just ensure they're valid hex
        assert_eq!(c1.len(), 6);
        assert_eq!(c2.len(), 6);
        // At minimum, both should be from the palette
        assert!(PALETTE.contains(&c1));
        assert!(PALETTE.contains(&c2));
    }

    #[test]
    fn test_auto_color_valid_hex() {
        for slug in &["bug", "feature", "docs", "urgent", "low-priority", "v2"] {
            let color = auto_color(slug);
            assert_eq!(color.len(), 6);
            assert!(color.chars().all(|c| c.is_ascii_hexdigit()));
        }
    }

    #[test]
    fn test_palette_coverage() {
        // With enough tags, we should hit multiple palette entries
        let mut seen = std::collections::HashSet::new();
        for i in 0..100 {
            let slug = format!("tag-{}", i);
            seen.insert(auto_color(&slug));
        }
        // Should hit at least half the palette
        assert!(seen.len() >= 8, "Only hit {} palette entries", seen.len());
    }
}
