//! Real EchoService tests without mocks
//!
//! These tests verify the EchoService implementation using the rmcp SDK
//! patterns correctly, focusing on testing the actual functionality.

#[cfg(test)]
mod tests {
    use crate::echo::EchoService;
    use rmcp::ServerHandler;
    use rstest::*;

    #[rstest]
    #[tokio::test]
    async fn test_real_echo_service_creation() {
        let service = EchoService::new();

        // Test service implements ServerHandler trait properly
        let info = service.get_info();
        assert_eq!(
            info.protocol_version,
            rmcp::model::ProtocolVersion::V_2024_11_05
        );
        assert!(info.capabilities.tools.is_some());
        assert!(info.capabilities.prompts.is_some());
    }

    #[rstest]
    #[tokio::test]
    async fn test_real_tool_attributes_generation() {
        // Test tool attributes are generated correctly by the macros
        let echo_attr = EchoService::echo_tool_attr();
        assert_eq!(echo_attr.name, "echo");
        assert!(echo_attr.description.is_some());
        assert!(echo_attr.description.unwrap().contains("Echo back"));

        let status_attr = EchoService::status_tool_attr();
        assert_eq!(status_attr.name, "status");
        assert!(status_attr.description.is_some());
        assert!(status_attr.description.unwrap().contains("status"));
    }

    #[rstest]
    #[tokio::test]
    async fn test_real_prompt_attributes_generation() {
        // Test prompt attributes are generated correctly by the macros
        let prompt_attr = EchoService::echo_prompt_prompt_attr();
        assert_eq!(prompt_attr.name, "echo_prompt");
        assert!(prompt_attr.description.is_some());

        // Verify arguments are correctly defined
        if let Some(args) = prompt_attr.arguments {
            assert_eq!(args.len(), 1);
            assert_eq!(args[0].name, "message");
            assert_eq!(args[0].required, Some(true));
        } else {
            panic!("Expected arguments for echo_prompt");
        }
    }

    #[rstest]
    #[tokio::test]
    async fn test_real_server_info() {
        let service = EchoService::new();
        let info = service.get_info();

        // Verify protocol version and capabilities
        assert_eq!(
            info.protocol_version,
            rmcp::model::ProtocolVersion::V_2024_11_05
        );

        // Verify tools capability is enabled
        assert!(info.capabilities.tools.is_some());

        // Verify prompts capability is enabled
        assert!(info.capabilities.prompts.is_some());

        // Verify instructions contain expected tool/prompt info
        if let Some(instructions) = info.instructions {
            assert!(instructions.contains("echo"));
            assert!(instructions.contains("status"));
            assert!(instructions.contains("echo_prompt"));
        }
    }

    #[rstest]
    #[tokio::test]
    async fn test_real_service_cloning() {
        let service1 = EchoService::new();
        let service2 = service1.clone();

        // Both services should have identical info
        let info1 = service1.get_info();
        let info2 = service2.get_info();

        assert_eq!(info1.protocol_version, info2.protocol_version);
        assert_eq!(info1.capabilities.tools, info2.capabilities.tools);
        assert_eq!(info1.capabilities.prompts, info2.capabilities.prompts);
    }
}
