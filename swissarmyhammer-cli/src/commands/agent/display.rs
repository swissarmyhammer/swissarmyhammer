//! Display objects for agent command output
//!
//! Provides clean display objects with `Tabled` and `Serialize` derives for consistent
//! output formatting across table, JSON, and YAML formats.

use serde::{Deserialize, Serialize};
use swissarmyhammer_config::agent::AgentInfo;
use tabled::Tabled;

/// Basic agent information for standard list output
#[derive(Tabled, Serialize, Deserialize, Debug, Clone)]
pub struct AgentRow {
    #[tabled(rename = "Name")]
    pub name: String,

    #[tabled(rename = "Description")]
    pub description: String,

    #[tabled(rename = "Source")]
    pub source: String,
}

/// Detailed agent information for verbose list output  
#[derive(Tabled, Serialize, Deserialize, Debug, Clone)]
pub struct VerboseAgentRow {
    #[tabled(rename = "Name")]
    pub name: String,

    #[tabled(rename = "Description")]
    pub description: String,

    #[tabled(rename = "Source")]
    pub source: String,

    #[tabled(rename = "Content Size")]
    pub content_size: String,
}

impl From<&AgentInfo> for AgentRow {
    fn from(agent: &AgentInfo) -> Self {
        Self {
            name: agent.name.clone(),
            description: agent
                .description
                .as_deref()
                .unwrap_or("No description")
                .to_string(),
            source: agent.source.display_emoji().to_string(),
        }
    }
}

impl From<&AgentInfo> for VerboseAgentRow {
    fn from(agent: &AgentInfo) -> Self {
        Self {
            name: agent.name.clone(),
            description: agent
                .description
                .as_deref()
                .unwrap_or("No description")
                .to_string(),
            source: agent.source.display_emoji().to_string(),
            content_size: format!("{} chars", agent.content.len()),
        }
    }
}

/// Convert agents to appropriate display format based on verbose flag
pub fn agents_to_display_rows(agents: Vec<AgentInfo>, verbose: bool) -> DisplayRows {
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
    use swissarmyhammer_config::agent::AgentSource;

    fn create_test_agent() -> AgentInfo {
        AgentInfo {
            name: "test-agent".to_string(),
            description: Some("Test description".to_string()),
            source: AgentSource::Builtin,
            content: "Test agent configuration content".to_string(),
        }
    }

    fn create_agent_with_all_fields() -> AgentInfo {
        AgentInfo {
            name: "complete-agent".to_string(),
            description: Some("Complete agent description".to_string()),
            source: AgentSource::Project,
            content: "Complete agent configuration with lots of content here".to_string(),
        }
    }

    fn create_minimal_agent() -> AgentInfo {
        AgentInfo {
            name: "minimal-agent".to_string(),
            description: None,
            source: AgentSource::User,
            content: "".to_string(),
        }
    }

    #[test]
    fn test_agent_row_conversion() {
        let agent = create_test_agent();
        let row = AgentRow::from(&agent);
        assert_eq!(row.name, "test-agent");
        assert_eq!(row.description, "Test description");
        assert_eq!(row.source, "📦 Built-in");
    }

    #[test]
    fn test_agent_row_from_complete_agent() {
        let agent = create_agent_with_all_fields();
        let row = AgentRow::from(&agent);
        assert_eq!(row.name, "complete-agent");
        assert_eq!(row.description, "Complete agent description");
        assert_eq!(row.source, "📁 Project");
    }

    #[test]
    fn test_agent_row_from_minimal_agent() {
        let agent = create_minimal_agent();
        let row = AgentRow::from(&agent);
        assert_eq!(row.name, "minimal-agent");
        assert_eq!(row.description, "No description");
        assert_eq!(row.source, "👤 User");
    }

    #[test]
    fn test_verbose_agent_row_conversion() {
        let agent = create_test_agent();
        let row = VerboseAgentRow::from(&agent);
        assert_eq!(row.name, "test-agent");
        assert_eq!(row.description, "Test description");
        assert_eq!(row.source, "📦 Built-in");
        assert_eq!(row.content_size, "32 chars");
    }

    #[test]
    fn test_verbose_agent_row_from_complete_agent() {
        let agent = create_agent_with_all_fields();
        let row = VerboseAgentRow::from(&agent);
        assert_eq!(row.name, "complete-agent");
        assert_eq!(row.description, "Complete agent description");
        assert_eq!(row.source, "📁 Project");
        assert_eq!(row.content_size, "54 chars");
    }

    #[test]
    fn test_verbose_agent_row_from_minimal_agent() {
        let agent = create_minimal_agent();
        let row = VerboseAgentRow::from(&agent);
        assert_eq!(row.name, "minimal-agent");
        assert_eq!(row.description, "No description");
        assert_eq!(row.source, "👤 User");
        assert_eq!(row.content_size, "0 chars");
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
        let builtin_agent = AgentInfo {
            name: "builtin-agent".to_string(),
            description: Some("Builtin description".to_string()),
            source: AgentSource::Builtin,
            content: "builtin content".to_string(),
        };

        let project_agent = AgentInfo {
            name: "project-agent".to_string(),
            description: Some("Project description".to_string()),
            source: AgentSource::Project,
            content: "project content".to_string(),
        };

        let user_agent = AgentInfo {
            name: "user-agent".to_string(),
            description: Some("User description".to_string()),
            source: AgentSource::User,
            content: "user content".to_string(),
        };

        // Test all source types convert correctly
        assert_eq!(AgentRow::from(&builtin_agent).source, "📦 Built-in");
        assert_eq!(AgentRow::from(&project_agent).source, "📁 Project");
        assert_eq!(AgentRow::from(&user_agent).source, "👤 User");

        assert_eq!(VerboseAgentRow::from(&builtin_agent).source, "📦 Built-in");
        assert_eq!(VerboseAgentRow::from(&project_agent).source, "📁 Project");
        assert_eq!(VerboseAgentRow::from(&user_agent).source, "👤 User");
    }
}
