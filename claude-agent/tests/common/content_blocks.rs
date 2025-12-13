//! Content block creation helpers for testing
//!
//! This module provides helper functions to create various types of content blocks
//! with sensible defaults for testing purposes.

use super::test_data;
use agent_client_protocol::{AudioContent, ContentBlock, ImageContent, ResourceLink, TextContent};

/// Create a text content block with the given text
pub fn text(content: &str) -> ContentBlock {
    ContentBlock::Text(TextContent {
        text: content.to_string(),
        annotations: None,
        meta: None,
    })
}

/// Create an image content block with the given MIME type and base64 data
pub fn image(mime_type: &str, data: &str) -> ContentBlock {
    ContentBlock::Image(ImageContent {
        data: data.to_string(),
        mime_type: mime_type.to_string(),
        uri: None,
        annotations: None,
        meta: None,
    })
}

/// Create a valid PNG image content block using test data
pub fn image_png() -> ContentBlock {
    image("image/png", test_data::VALID_PNG_BASE64)
}

/// Create an audio content block with the given MIME type and base64 data
pub fn audio(mime_type: &str, data: &str) -> ContentBlock {
    ContentBlock::Audio(AudioContent {
        data: data.to_string(),
        mime_type: mime_type.to_string(),
        annotations: None,
        meta: None,
    })
}

/// Create a valid WAV audio content block using test data
pub fn audio_wav() -> ContentBlock {
    audio("audio/wav", test_data::VALID_WAV_BASE64)
}

/// Create a resource link content block with customizable fields
pub fn resource_link(uri: &str, name: &str) -> ContentBlock {
    ContentBlock::ResourceLink(ResourceLink {
        uri: uri.to_string(),
        name: name.to_string(),
        description: None,
        mime_type: None,
        title: None,
        size: None,
        annotations: None,
        meta: None,
    })
}

/// Create a resource link content block with all fields populated
pub fn resource_link_full(
    uri: &str,
    name: &str,
    description: &str,
    mime_type: &str,
    title: &str,
    size: u64,
) -> ContentBlock {
    ContentBlock::ResourceLink(ResourceLink {
        uri: uri.to_string(),
        name: name.to_string(),
        description: Some(description.to_string()),
        mime_type: Some(mime_type.to_string()),
        title: Some(title.to_string()),
        size: Some(size.try_into().unwrap()),
        annotations: None,
        meta: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_creation() {
        let content = text("Hello, world!");
        match content {
            ContentBlock::Text(text_content) => {
                assert_eq!(text_content.text, "Hello, world!");
            }
            _ => panic!("Expected Text content block"),
        }
    }

    #[test]
    fn test_image_creation() {
        let content = image("image/png", test_data::VALID_PNG_BASE64);
        match content {
            ContentBlock::Image(img) => {
                assert_eq!(img.mime_type, "image/png");
                assert_eq!(img.data, test_data::VALID_PNG_BASE64);
            }
            _ => panic!("Expected Image content block"),
        }
    }

    #[test]
    fn test_image_png_helper() {
        let content = image_png();
        match content {
            ContentBlock::Image(img) => {
                assert_eq!(img.mime_type, "image/png");
                assert_eq!(img.data, test_data::VALID_PNG_BASE64);
            }
            _ => panic!("Expected Image content block"),
        }
    }

    #[test]
    fn test_audio_creation() {
        let content = audio("audio/wav", test_data::VALID_WAV_BASE64);
        match content {
            ContentBlock::Audio(audio_content) => {
                assert_eq!(audio_content.mime_type, "audio/wav");
                assert_eq!(audio_content.data, test_data::VALID_WAV_BASE64);
            }
            _ => panic!("Expected Audio content block"),
        }
    }

    #[test]
    fn test_audio_wav_helper() {
        let content = audio_wav();
        match content {
            ContentBlock::Audio(audio_content) => {
                assert_eq!(audio_content.mime_type, "audio/wav");
                assert_eq!(audio_content.data, test_data::VALID_WAV_BASE64);
            }
            _ => panic!("Expected Audio content block"),
        }
    }

    #[test]
    fn test_resource_link_creation() {
        let content = resource_link("https://example.com", "test resource");
        match content {
            ContentBlock::ResourceLink(link) => {
                assert_eq!(link.uri, "https://example.com");
                assert_eq!(link.name, "test resource");
                assert!(link.description.is_none());
            }
            _ => panic!("Expected ResourceLink content block"),
        }
    }

    #[test]
    fn test_resource_link_full_creation() {
        let content = resource_link_full(
            "https://example.com/doc",
            "Document",
            "A test document",
            "text/plain",
            "Test Document",
            1024,
        );
        match content {
            ContentBlock::ResourceLink(link) => {
                assert_eq!(link.uri, "https://example.com/doc");
                assert_eq!(link.name, "Document");
                assert_eq!(link.description, Some("A test document".to_string()));
                assert_eq!(link.mime_type, Some("text/plain".to_string()));
                assert_eq!(link.title, Some("Test Document".to_string()));
                assert_eq!(link.size, Some(1024));
            }
            _ => panic!("Expected ResourceLink content block"),
        }
    }
}
