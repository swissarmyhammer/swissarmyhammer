//! Display objects for agent command output
//!
//! Provides clean display objects with `Serialize` derives for consistent
//! output formatting across table, JSON, and YAML formats.

use serde::{Deserialize, Serialize};
use swissarmyhammer_config::model::ModelInfo;

/// Basic agent information for standard list output
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AgentRow {
    pub name: String,
    pub description: String,
    pub source: String,
}

/// Detailed agent information for verbose list output
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VerboseAgentRow {
    pub name: String,
    pub description: String,
    pub source: String,
    pub content_size: String,
}

/// Extract common fields from ModelInfo to reduce duplication
fn extract_common_fields(agent: &ModelInfo) -> (String, String, String) {
    (
        agent.name.clone(),
        agent
            .description
            .as_deref()
            .unwrap_or("No description")
            .to_string(),
        agent.source.display_emoji().to_string(),
    )
}

impl From<&ModelInfo> for AgentRow {
    fn from(agent: &ModelInfo) -> Self {
        let (name, description, source) = extract_common_fields(agent);
        Self {
            name,
            description,
            source,
        }
    }
}

impl From<&ModelInfo> for VerboseAgentRow {
    fn from(agent: &ModelInfo) -> Self {
        let (name, description, source) = extract_common_fields(agent);
        Self {
            name,
            description,
            source,
            content_size: format!("{} chars", agent.content.len()),
        }
    }
}

/// Convert agents to appropriate display format based on verbose flag
pub fn agents_to_display_rows(agents: Vec<ModelInfo>, verbose: bool) -> DisplayRows {
    if verbose {
        DisplayRows::Verbose(agents.iter().map(VerboseAgentRow::from).collect())
    } else {
        DisplayRows::Standard(agents.iter().map(AgentRow::from).collect())
    }
}

/// Enum to handle different display row types
#[derive(Debug)]
pub enum DisplayRows {
    Standard(Vec<AgentRow>),
    Verbose(Vec<VerboseAgentRow>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use swissarmyhammer_config::model::ModelConfigSource;

    fn create_test_agent() -> ModelInfo {
        ModelInfo {
            name: "test-agent".to_string(),
            description: Some("Test description".to_string()),
            source: ModelConfigSource::Builtin,
            content: "Test agent configuration content".to_string(),
        }
    }

    fn create_agent_with_all_fields() -> ModelInfo {
        ModelInfo {
            name: "complete-agent".to_string(),
            description: Some("Complete agent description".to_string()),
            source: ModelConfigSource::Project,
            content: "Complete agent configuration with lots of content here".to_string(),
        }
    }

    fn create_minimal_agent() -> ModelInfo {
        ModelInfo {
            name: "minimal-agent".to_string(),
            description: None,
            source: ModelConfigSource::User,
            content: "".to_string(),
        }
    }

    /// Helper to test AgentRow conversion with expected values
    fn assert_agent_row(
        agent: &ModelInfo,
        expected_name: &str,
        expected_desc: &str,
        expected_source: &str,
    ) {
        let row = AgentRow::from(agent);
        assert_eq!(row.name, expected_name);
        assert_eq!(row.description, expected_desc);
        assert_eq!(row.source, expected_source);
    }

    /// Helper to test VerboseAgentRow conversion with expected values
    fn assert_verbose_agent_row(
        agent: &ModelInfo,
        expected_name: &str,
        expected_desc: &str,
        expected_source: &str,
        expected_content_size: &str,
    ) {
        let row = VerboseAgentRow::from(agent);
        assert_eq!(row.name, expected_name);
        assert_eq!(row.description, expected_desc);
        assert_eq!(row.source, expected_source);
        assert_eq!(row.content_size, expected_content_size);
    }

    #[test]
    fn test_agent_row_conversion() {
        let agent = create_test_agent();
        assert_agent_row(&agent, "test-agent", "Test description", "📦 Built-in");
    }

    #[test]
    fn test_agent_row_from_complete_agent() {
        let agent = create_agent_with_all_fields();
        assert_agent_row(
            &agent,
            "complete-agent",
            "Complete agent description",
            "📁 Project",
        );
    }

    #[test]
    fn test_agent_row_from_minimal_agent() {
        let agent = create_minimal_agent();
        assert_agent_row(&agent, "minimal-agent", "No description", "👤 User");
    }

    #[test]
    fn test_verbose_agent_row_conversion() {
        let agent = create_test_agent();
        assert_verbose_agent_row(
            &agent,
            "test-agent",
            "Test description",
            "📦 Built-in",
            "32 chars",
        );
    }

    #[test]
    fn test_verbose_agent_row_from_complete_agent() {
        let agent = create_agent_with_all_fields();
        assert_verbose_agent_row(
            &agent,
            "complete-agent",
            "Complete agent description",
            "📁 Project",
            "54 chars",
        );
    }

    #[test]
    fn test_verbose_agent_row_from_minimal_agent() {
        let agent = create_minimal_agent();
        assert_verbose_agent_row(
            &agent,
            "minimal-agent",
            "No description",
            "👤 User",
            "0 chars",
        );
    }

    #[test]
    fn test_agents_to_display_rows_standard() {
        let agents = vec![create_test_agent()];
        let rows = agents_to_display_rows(agents, false);

        match rows {
            DisplayRows::Standard(standard_rows) => {
                assert_eq!(standard_rows.len(), 1);
                assert_eq!(standard_rows[0].name, "test-agent");
            }
            DisplayRows::Verbose(_) => panic!("Expected Standard rows"),
        }
    }

    #[test]
    fn test_agents_to_display_rows_verbose() {
        let agents = vec![create_test_agent()];
        let rows = agents_to_display_rows(agents, true);

        match rows {
            DisplayRows::Verbose(verbose_rows) => {
                assert_eq!(verbose_rows.len(), 1);
                assert_eq!(verbose_rows[0].name, "test-agent");
                assert_eq!(verbose_rows[0].description, "Test description");
                assert_eq!(verbose_rows[0].content_size, "32 chars");
            }
            DisplayRows::Standard(_) => panic!("Expected Verbose rows"),
        }
    }

    #[test]
    fn test_agents_to_display_rows_multiple_agents() {
        let agents = vec![
            create_test_agent(),
            create_agent_with_all_fields(),
            create_minimal_agent(),
        ];

        let standard_rows = agents_to_display_rows(agents.clone(), false);
        match standard_rows {
            DisplayRows::Standard(rows) => {
                assert_eq!(rows.len(), 3);
                assert_eq!(rows[0].name, "test-agent");
                assert_eq!(rows[1].name, "complete-agent");
                assert_eq!(rows[2].name, "minimal-agent");
            }
            _ => panic!("Expected Standard rows"),
        }

        let verbose_rows = agents_to_display_rows(agents, true);
        match verbose_rows {
            DisplayRows::Verbose(rows) => {
                assert_eq!(rows.len(), 3);
                assert_eq!(rows[0].name, "test-agent");
                assert_eq!(rows[1].name, "complete-agent");
                assert_eq!(rows[2].name, "minimal-agent");
            }
            _ => panic!("Expected Verbose rows"),
        }
    }

    #[test]
    fn test_agents_to_display_rows_empty_list() {
        let agents = vec![];

        let standard_rows = agents_to_display_rows(agents.clone(), false);
        match standard_rows {
            DisplayRows::Standard(rows) => assert!(rows.is_empty()),
            _ => panic!("Expected Standard rows"),
        }

        let verbose_rows = agents_to_display_rows(agents, true);
        match verbose_rows {
            DisplayRows::Verbose(rows) => assert!(rows.is_empty()),
            _ => panic!("Expected Verbose rows"),
        }
    }

    #[test]
    fn test_serialization_agent_row() {
        let row = AgentRow {
            name: "test".to_string(),
            description: "Test Description".to_string(),
            source: "📦 Built-in".to_string(),
        };

        let json = serde_json::to_string(&row).expect("Should serialize to JSON");
        assert!(json.contains("test"));
        assert!(json.contains("Test Description"));
        assert!(json.contains("📦 Built-in"));

        let deserialized: AgentRow =
            serde_json::from_str(&json).expect("Should deserialize from JSON");
        assert_eq!(deserialized.name, "test");
        assert_eq!(deserialized.description, "Test Description");
        assert_eq!(deserialized.source, "📦 Built-in");
    }

    #[test]
    fn test_serialization_verbose_agent_row() {
        let row = VerboseAgentRow {
            name: "test".to_string(),
            description: "Test Description".to_string(),
            source: "📦 Built-in".to_string(),
            content_size: "100 chars".to_string(),
        };

        let json = serde_json::to_string(&row).expect("Should serialize to JSON");
        assert!(json.contains("test"));
        assert!(json.contains("Test Description"));
        assert!(json.contains("📦 Built-in"));
        assert!(json.contains("100 chars"));

        let deserialized: VerboseAgentRow =
            serde_json::from_str(&json).expect("Should deserialize from JSON");
        assert_eq!(deserialized.name, "test");
        assert_eq!(deserialized.description, "Test Description");
        assert_eq!(deserialized.source, "📦 Built-in");
        assert_eq!(deserialized.content_size, "100 chars");
    }

    #[test]
    fn test_display_rows_debug_format() {
        let agents = vec![create_test_agent()];
        let rows = agents_to_display_rows(agents, false);

        let debug_str = format!("{:?}", rows);
        assert!(debug_str.contains("Standard"));
        assert!(debug_str.contains("test-agent"));
    }

    #[test]
    fn test_all_source_types() {
        let test_cases = [
            (ModelConfigSource::Builtin, "📦 Built-in"),
            (ModelConfigSource::Project, "📁 Project"),
            (ModelConfigSource::User, "👤 User"),
        ];

        for (source, expected_emoji) in test_cases {
            let agent = ModelInfo {
                name: format!("{:?}-agent", source).to_lowercase(),
                description: Some(format!("{:?} description", source)),
                source,
                content: "content".to_string(),
            };

            assert_eq!(AgentRow::from(&agent).source, expected_emoji);
            assert_eq!(VerboseAgentRow::from(&agent).source, expected_emoji);
        }
    }
}
