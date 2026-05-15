//! Shared test data constants
//!
//! This module provides commonly used test data constants to avoid duplication
//! across test files.

/// Valid 1x1 PNG image (base64 encoded)
/// This is a minimal valid PNG file used for testing image content
pub const VALID_PNG_BASE64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";

/// Valid WAV audio file (base64 encoded)
/// This is a minimal valid WAV file used for testing audio content
pub const VALID_WAV_BASE64: &str = "UklGRiQAAABXQVZFZm10IBAAAAABAAEAQB8AAEAfAAABAAgAZGF0YQAAAAAA";

/// PE executable header (base64 encoded) for malicious content detection testing
/// This represents the "MZ" signature of Windows PE executables
pub const MALICIOUS_PE_BASE64: &str =
    "TVqQAAMAAAAEAAAA//8AALgAAAAAAAAAQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

/// ELF executable header (base64 encoded) for malicious content detection testing
/// This represents the ELF magic number (0x7F 'E' 'L' 'F')
pub const MALICIOUS_ELF_BASE64: &str = "f0VMRgIBAQAAAAAAAAAAAA==";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_png_base64_is_valid() {
        // Verify it's valid base64
        let decoded =
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, VALID_PNG_BASE64);
        assert!(decoded.is_ok());

        // Verify it starts with PNG signature
        let bytes = decoded.unwrap();
        assert!(bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47])); // PNG signature
    }

    #[test]
    fn test_wav_base64_is_valid() {
        // Verify it's valid base64
        let decoded =
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, VALID_WAV_BASE64);
        assert!(decoded.is_ok());

        // Verify it starts with RIFF signature
        let bytes = decoded.unwrap();
        assert!(bytes.starts_with(b"RIFF")); // RIFF signature
    }

    #[test]
    fn test_pe_base64_is_valid() {
        // Verify it's valid base64
        let decoded = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            MALICIOUS_PE_BASE64,
        );
        assert!(decoded.is_ok());

        // Verify it starts with MZ signature
        let bytes = decoded.unwrap();
        assert!(bytes.starts_with(b"MZ")); // PE signature
    }

    #[test]
    fn test_elf_base64_is_valid() {
        // Verify it's valid base64
        let decoded = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            MALICIOUS_ELF_BASE64,
        );
        assert!(decoded.is_ok());

        // Verify it starts with ELF signature
        let bytes = decoded.unwrap();
        assert!(bytes.starts_with(&[0x7F, b'E', b'L', b'F'])); // ELF signature
    }
}
