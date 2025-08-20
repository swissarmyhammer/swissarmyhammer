use anyhow::Result;
use rmcp::model::{CallToolResult, RawContent};


/// Formatter for MCP tool responses optimized for CLI display
///
/// The ResponseFormatter handles the conversion of MCP tool responses
/// into user-friendly CLI output, supporting multiple content types
/// and output formats.
pub struct ResponseFormatter;

impl ResponseFormatter {
    /// Format MCP tool response for CLI display
    ///
    /// Converts CallToolResult into a string suitable for terminal output,
    /// handling both success and error cases appropriately.
    pub fn format_response(result: &CallToolResult) -> Result<String> {
        if result.is_error.unwrap_or(false) {
            // Extract error message from content
            Self::format_success_content(&result.content)
                .map(|text| format!("Error: {}", text))
        } else {
            Self::format_success_content(&result.content)
        }
    }

    /// Format successful response content
    ///
    /// Processes the content array from a successful MCP tool response,
    /// handling different content types and combining them into a single
    /// formatted string.
    fn format_success_content(content: &[rmcp::model::Annotated<RawContent>]) -> Result<String> {
        let mut output = String::new();

        for (i, annotated_item) in content.iter().enumerate() {
            if i > 0 {
                output.push('\n');
            }

            match &annotated_item.raw {
                RawContent::Text(text_content) => {
                    output.push_str(&text_content.text);
                }
                RawContent::Image(_) => {
                    output.push_str("[Image content - not displayable in CLI]");
                }
                RawContent::Resource(_) => {
                    output.push_str("[Resource content]");
                }
                RawContent::Audio(_) => {
                    output.push_str("[Audio content - not playable in CLI]");
                }
            }
        }

        Ok(output)
    }


}



#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::model::{Annotated, RawContent, RawTextContent};


    fn create_success_result(text: &str) -> CallToolResult {
        CallToolResult {
            content: vec![Annotated::new(
                RawContent::Text(RawTextContent {
                    text: text.to_string(),
                }),
                None,
            )],
            is_error: Some(false),
        }
    }

    fn create_error_result(error: &str) -> CallToolResult {
        CallToolResult {
            content: vec![Annotated::new(
                RawContent::Text(RawTextContent {
                    text: error.to_string(),
                }),
                None,
            )],
            is_error: Some(true),
        }
    }

    #[test]
    fn test_format_success_response() {
        let result = create_success_result("Hello, world!");
        let formatted = ResponseFormatter::format_response(&result).unwrap();
        assert_eq!(formatted, "Hello, world!");
    }

    #[test]
    fn test_format_error_response() {
        let result = create_error_result("Something went wrong");
        let formatted = ResponseFormatter::format_response(&result).unwrap();
        assert_eq!(formatted, "Error: Something went wrong");
    }

    #[test]
    fn test_format_multiple_content_items() {
        let result = CallToolResult {
            content: vec![
                Annotated::new(
                    RawContent::Text(RawTextContent {
                        text: "First item".to_string(),
                    }),
                    None,
                ),
                Annotated::new(
                    RawContent::Text(RawTextContent {
                        text: "Second item".to_string(),
                    }),
                    None,
                ),
            ],
            is_error: Some(false),
        };

        let formatted = ResponseFormatter::format_response(&result).unwrap();
        assert_eq!(formatted, "First item\nSecond item");
    }

    #[test]
    fn test_format_image_content() {
        let result = CallToolResult {
            content: vec![Annotated::new(
                RawContent::Image(rmcp::model::RawImageContent {
                    data: "base64data".to_string(),
                    mime_type: "image/png".to_string(),
                }),
                None,
            )],
            is_error: Some(false),
        };

        let formatted = ResponseFormatter::format_response(&result).unwrap();
        assert_eq!(formatted, "[Image content - not displayable in CLI]");
    }



}