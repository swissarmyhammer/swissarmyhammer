//! Integration tests for end-to-end configuration workflows
//!
//! This module tests the complete configuration system workflow including file loading,
//! template integration, and complex real-world scenarios.

use crate::toml_config::{load_repo_config, parse_config_string, ConfigValue};
use std::fs;
use tempfile::TempDir;

/// Test end-to-end configuration loading from filesystem
#[test]
fn test_end_to_end_config_loading() {
    let temp_dir = TempDir::new().unwrap();
    
    // Create a complex configuration file using multiline string
    let config_content = concat!(
        "# Project metadata\n",
        "name = \"SwissArmyHammer\"\n",
        "version = \"2.0.0\"\n",
        "description = \"A flexible prompt and workflow management tool\"\n",
        "author = \"The SwissArmyHammer Team\"\n",
        "license = \"MIT\"\n",
        "\n",
        "# Environment variables\n",
        "database_url = \"${DATABASE_URL:-postgresql://localhost:5432/swissarmyhammer}\"\n",
        "api_key = \"${API_KEY:-dev_key_12345}\"\n",
        "debug_mode = \"${DEBUG:-false}\"\n",
        "\n",
        "# Arrays\n",
        "keywords = [\"cli\", \"automation\", \"templates\", \"workflows\"]\n",
        "maintainers = [\"alice@example.com\", \"bob@example.com\", \"carol@example.com\"]\n",
        "supported_formats = [\"json\", \"yaml\", \"toml\"]\n",
        "\n",
        "# Team information\n",
        "[team]\n",
        "lead = \"alice@example.com\"\n",
        "size = 5\n",
        "timezone = \"UTC\"\n",
        "\n",
        "[team.contact]\n",
        "email = \"team@swissarmyhammer.dev\"\n",
        "slack = \"#swissarmyhammer\"\n",
        "\n",
        "# Build configuration\n",
        "[build]\n",
        "language = \"rust\"\n",
        "minimum_version = \"1.70.0\"\n",
        "features = [\"async\", \"cli\", \"templates\", \"workflows\"]\n",
        "targets = [\"x86_64-unknown-linux\", \"x86_64-pc-windows\", \"x86_64-apple-darwin\"]\n",
        "optimized = true\n",
        "profile = \"release\"\n",
        "\n",
        "# Database configuration\n",
        "[database.primary]\n",
        "host = \"localhost\"\n",
        "port = 5432\n",
        "name = \"swissarmyhammer\"\n",
        "ssl = true\n",
        "pool_size = 10\n",
        "\n",
        "[database.replica]\n",
        "host = \"replica.example.com\"\n",
        "port = 5432\n",
        "name = \"swissarmyhammer_readonly\"\n",
        "ssl = true\n",
        "readonly = true\n",
        "\n",
        "# Deployment settings\n",
        "[deployment]\n",
        "environment = \"development\"\n",
        "region = \"us-west-2\"\n",
        "scaling = \"auto\"\n",
        "\n",
        "[deployment.resources]\n",
        "cpu = \"2 cores\"\n",
        "memory = \"4GB\"\n",
        "storage = \"100GB SSD\"\n",
        "\n",
        "# Feature flags\n",
        "[features]\n",
        "new_ui = true\n",
        "beta_features = false\n",
        "telemetry = true\n",
        "analytics = false\n",
        "\n",
        "# External services\n",
        "[services.github]\n",
        "api_url = \"https://api.github.com\"\n",
        "token = \"${GITHUB_TOKEN}\"\n",
        "org = \"swissarmyhammer\"\n",
        "\n",
        "[services.docker]\n",
        "registry = \"docker.io\"\n",
        "namespace = \"swissarmyhammer\"\n",
        "tag_format = \"v{version}\"\n",
        "\n",
        "# Documentation\n",
        "[documentation]\n",
        "site_url = \"https://swissarmyhammer.dev\"\n",
        "api_docs = \"https://docs.swissarmyhammer.dev/api\"\n",
        "guides = \"https://guides.swissarmyhammer.dev\"\n"
    );
    
    let config_path = temp_dir.path().join("sah.toml");
    fs::write(&config_path, config_content).unwrap();
    
    // Set up environment variables for testing
    std::env::set_var("DATABASE_URL", "postgresql://prod.example.com:5432/swissarmyhammer");
    std::env::set_var("API_KEY", "prod_api_key_xyz789");
    std::env::set_var("DEBUG", "true");
    std::env::set_var("GITHUB_TOKEN", "ghp_test_token_123");
    
    // Load and parse configuration
    let mut config = parse_config_string(config_content).unwrap();
    
    // Perform environment variable substitution
    config.substitute_env_vars().unwrap();
    
    // Test basic metadata
    assert_eq!(
        config.get("name"),
        Some(&ConfigValue::String("SwissArmyHammer".to_string()))
    );
    assert_eq!(
        config.get("version"),
        Some(&ConfigValue::String("2.0.0".to_string()))
    );
    assert_eq!(
        config.get("description"),
        Some(&ConfigValue::String("A flexible prompt and workflow management tool".to_string()))
    );
    
    // Test environment variable substitution
    assert_eq!(
        config.get("database_url"),
        Some(&ConfigValue::String("postgresql://prod.example.com:5432/swissarmyhammer".to_string()))
    );
    assert_eq!(
        config.get("api_key"),
        Some(&ConfigValue::String("prod_api_key_xyz789".to_string()))
    );
    assert_eq!(
        config.get("debug_mode"),
        Some(&ConfigValue::String("true".to_string()))
    );
    
    // Test arrays
    if let Some(ConfigValue::Array(keywords)) = config.get("keywords") {
        assert_eq!(keywords.len(), 4);
        assert_eq!(keywords[0], ConfigValue::String("cli".to_string()));
        assert_eq!(keywords[1], ConfigValue::String("automation".to_string()));
        assert_eq!(keywords[2], ConfigValue::String("templates".to_string()));
        assert_eq!(keywords[3], ConfigValue::String("workflows".to_string()));
    } else {
        panic!("Expected array for keywords");
    }
    
    // Test nested structures
    assert_eq!(
        config.get("team.lead"),
        Some(&ConfigValue::String("alice@example.com".to_string()))
    );
    assert_eq!(config.get("team.size"), Some(&ConfigValue::Integer(5)));
    assert_eq!(
        config.get("team.contact.email"),
        Some(&ConfigValue::String("team@swissarmyhammer.dev".to_string()))
    );
    
    // Test deeply nested structures
    assert_eq!(
        config.get("database.primary.host"),
        Some(&ConfigValue::String("localhost".to_string()))
    );
    assert_eq!(config.get("database.primary.port"), Some(&ConfigValue::Integer(5432)));
    assert_eq!(config.get("database.primary.ssl"), Some(&ConfigValue::Boolean(true)));
    assert_eq!(config.get("database.replica.readonly"), Some(&ConfigValue::Boolean(true)));
    
    // Test build configuration array
    if let Some(ConfigValue::Array(targets)) = config.get("build.targets") {
        assert_eq!(targets.len(), 3);
        assert!(targets.contains(&ConfigValue::String("x86_64-unknown-linux".to_string())));
        assert!(targets.contains(&ConfigValue::String("x86_64-pc-windows".to_string())));
        assert!(targets.contains(&ConfigValue::String("x86_64-apple-darwin".to_string())));
    } else {
        panic!("Expected array for build.targets");
    }
    
    // Test feature flags
    assert_eq!(config.get("features.new_ui"), Some(&ConfigValue::Boolean(true)));
    assert_eq!(config.get("features.beta_features"), Some(&ConfigValue::Boolean(false)));
    
    // Test services with environment variables
    assert_eq!(
        config.get("services.github.token"),
        Some(&ConfigValue::String("ghp_test_token_123".to_string()))
    );
    
    // Test configuration validation
    assert!(config.validate().is_ok());
    
    // Test liquid object conversion
    let liquid_object = config.to_liquid_object();
    assert!(!liquid_object.is_empty());
    
    // Clean up environment variables
    std::env::remove_var("DATABASE_URL");
    std::env::remove_var("API_KEY");
    std::env::remove_var("DEBUG");
    std::env::remove_var("GITHUB_TOKEN");
}

/// Test template integration with configuration variables
#[test]
fn test_template_integration() {
    let config_content = concat!(
        "project_name = \"MyAwesomeProject\"\n",
        "version = \"1.2.3\"\n",
        "author = \"John Doe\"\n",
        "license = \"MIT\"\n",
        "\n",
        "repository = \"https://github.com/johndoe/awesome-project\"\n",
        "homepage = \"https://awesome-project.dev\"\n",
        "\n",
        "keywords = [\"rust\", \"cli\", \"awesome\"]\n",
        "\n",
        "[build]\n",
        "target = \"x86_64-unknown-linux-gnu\"\n",
        "features = [\"feature1\", \"feature2\", \"feature3\"]\n",
        "optimized = true\n",
        "\n",
        "[team]\n",
        "lead = \"John Doe\"\n",
        "members = [\"Alice Smith\", \"Bob Johnson\", \"Carol Williams\"]\n",
        "size = 4\n",
        "\n",
        "[deployment]\n",
        "environment = \"production\"\n",
        "region = \"us-east-1\"\n",
        "\n",
        "[database]\n",
        "host = \"db.awesome-project.com\"\n",
        "port = 5432\n",
        "name = \"awesome_db\"\n"
    );
    
    let mut config = parse_config_string(config_content).unwrap();
    config.substitute_env_vars().unwrap();
    
    // Create a liquid template parser
    let liquid_parser = liquid::ParserBuilder::with_stdlib().build().unwrap();
    
    // Test simple variable substitution
    let template1 = liquid_parser.parse("Project: {{ project_name }} v{{ version }}").unwrap();
    let liquid_context = config.to_liquid_object();
    let result1 = template1.render(&liquid_context).unwrap();
    assert_eq!(result1, "Project: MyAwesomeProject v1.2.3");
    
    // Test nested object access
    let template2 = liquid_parser.parse("Build target: {{ build.target }}").unwrap();
    let result2 = template2.render(&liquid_context).unwrap();
    assert_eq!(result2, "Build target: x86_64-unknown-linux-gnu");
    
    // Test boolean values
    let template3 = liquid_parser.parse("Optimized: {{ build.optimized }}").unwrap();
    let result3 = template3.render(&liquid_context).unwrap();
    assert_eq!(result3, "Optimized: true");
    
    // Test array iteration
    let template4 = liquid_parser.parse(
        "Keywords: {% for keyword in keywords %}{{ keyword }}{% unless forloop.last %}, {% endunless %}{% endfor %}"
    ).unwrap();
    let result4 = template4.render(&liquid_context).unwrap();
    assert_eq!(result4, "Keywords: rust, cli, awesome");
    
    // Test array size
    let template5 = liquid_parser.parse("Team size: {{ team.members | size }} people").unwrap();
    let result5 = template5.render(&liquid_context).unwrap();
    assert_eq!(result5, "Team size: 3 people");
    
    // Test conditional rendering
    let template6 = liquid_parser.parse(
        "{% if build.optimized %}This is an optimized build{% else %}This is a debug build{% endif %}"
    ).unwrap();
    let result6 = template6.render(&liquid_context).unwrap();
    assert_eq!(result6, "This is an optimized build");
}

/// Test file discovery from different directory structures
#[test]
fn test_file_discovery_from_different_directories() {
    let temp_dir = TempDir::new().unwrap();
    
    // Create repository structure
    let repo_root = temp_dir.path();
    let git_dir = repo_root.join(".git");
    fs::create_dir(&git_dir).unwrap();
    
    // Create sah.toml in repo root
    let config_path = repo_root.join("sah.toml");
    let config_content = concat!(
        "name = \"DiscoveryTest\"\n",
        "version = \"1.0.0\"\n",
        "\n",
        "[project]\n",
        "type = \"library\"\n",
        "language = \"rust\"\n"
    );
    fs::write(&config_path, config_content).unwrap();
    
    // Create nested directory structure
    let src_dir = repo_root.join("src");
    fs::create_dir(&src_dir).unwrap();
    
    let module_dir = src_dir.join("module");
    fs::create_dir(&module_dir).unwrap();
    
    let deep_dir = module_dir.join("deep");
    fs::create_dir(&deep_dir).unwrap();
    
    let tests_dir = repo_root.join("tests");
    fs::create_dir(&tests_dir).unwrap();
    
    let integration_dir = tests_dir.join("integration");
    fs::create_dir(&integration_dir).unwrap();
    
    // Test discovery from different directories
    let original_dir = std::env::current_dir().unwrap();
    
    // Test from repository root
    std::env::set_current_dir(&repo_root).unwrap();
    let result = load_repo_config();
    assert!(result.is_ok());
    if let Ok(Some(config)) = result {
        assert_eq!(
            config.get("name"),
            Some(&ConfigValue::String("DiscoveryTest".to_string()))
        );
    } else {
        panic!("Should find config from repo root");
    }
    
    // Test from src directory
    std::env::set_current_dir(&src_dir).unwrap();
    let result = load_repo_config();
    assert!(result.is_ok());
    if let Ok(Some(config)) = result {
        assert_eq!(
            config.get("name"),
            Some(&ConfigValue::String("DiscoveryTest".to_string()))
        );
    } else {
        panic!("Should find config from src directory");
    }
    
    // Test from deeply nested directory
    std::env::set_current_dir(&deep_dir).unwrap();
    let result = load_repo_config();
    assert!(result.is_ok());
    if let Ok(Some(config)) = result {
        assert_eq!(
            config.get("name"),
            Some(&ConfigValue::String("DiscoveryTest".to_string()))
        );
    } else {
        panic!("Should find config from deep directory");
    }
    
    // Test from tests directory
    std::env::set_current_dir(&tests_dir).unwrap();
    let result = load_repo_config();
    assert!(result.is_ok());
    if let Ok(Some(config)) = result {
        assert_eq!(
            config.get("name"),
            Some(&ConfigValue::String("DiscoveryTest".to_string()))
        );
    } else {
        panic!("Should find config from tests directory");
    }
    
    // Test from integration tests directory
    std::env::set_current_dir(&integration_dir).unwrap();
    let result = load_repo_config();
    assert!(result.is_ok());
    if let Ok(Some(config)) = result {
        assert_eq!(
            config.get("name"),
            Some(&ConfigValue::String("DiscoveryTest".to_string()))
        );
    } else {
        panic!("Should find config from integration directory");
    }
    
    // Test from directory without .git (should return None)
    let non_repo_dir = TempDir::new().unwrap();
    std::env::set_current_dir(non_repo_dir.path()).unwrap();
    let result = load_repo_config();
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
    
    // Restore original directory
    std::env::set_current_dir(original_dir).unwrap();
}