//! Simple integration test to validate core functionality

#[cfg(test)]
mod tests {
    use crate::{ConfigProvider, TemplateContext};
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_basic_functionality() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();

        // Create a basic config file
        let sah_dir = temp_dir.path().join(".swissarmyhammer");
        fs::create_dir_all(&sah_dir).unwrap();

        fs::write(
            sah_dir.join("sah.toml"),
            r#"
app_name = "Test App"
version = "1.0.0"

[database]
host = "localhost"
port = 5432
"#,
        )
        .unwrap();

        std::env::set_current_dir(temp_dir.path()).unwrap();

        let provider = ConfigProvider::new();
        let context = provider.load_template_context().unwrap();

        std::env::set_current_dir(original_dir).unwrap();

        // Verify basic values are loaded
        assert_eq!(
            context.get("app_name"),
            Some(&serde_json::Value::String("Test App".to_string()))
        );
        assert_eq!(
            context.get("version"),
            Some(&serde_json::Value::String("1.0.0".to_string()))
        );

        // Check nested object
        if let Some(database) = context.get("database") {
            assert_eq!(
                database["host"],
                serde_json::Value::String("localhost".to_string())
            );
            assert_eq!(database["port"], serde_json::Value::Number(5432.into()));
        } else {
            panic!("Database configuration should be present");
        }
    }

    #[test]
    fn test_template_context_operations() {
        let mut ctx = TemplateContext::new();

        // Test basic operations
        ctx.set(
            "test_key".to_string(),
            serde_json::Value::String("test_value".to_string()),
        );
        assert_eq!(
            ctx.get("test_key"),
            Some(&serde_json::Value::String("test_value".to_string()))
        );

        // Test merge
        let mut other_ctx = TemplateContext::new();
        other_ctx.set(
            "other_key".to_string(),
            serde_json::Value::String("other_value".to_string()),
        );

        ctx.merge(&other_ctx);
        assert_eq!(
            ctx.get("test_key"),
            Some(&serde_json::Value::String("test_value".to_string()))
        );
        assert_eq!(
            ctx.get("other_key"),
            Some(&serde_json::Value::String("other_value".to_string()))
        );
    }

    #[test]
    fn test_env_var_substitution_basic() {
        std::env::set_var("TEST_ENV_VAR", "test_env_value");

        let mut ctx = TemplateContext::new();
        ctx.set(
            "config_var".to_string(),
            serde_json::Value::String("${TEST_ENV_VAR}".to_string()),
        );

        ctx.substitute_env_vars().unwrap();

        assert_eq!(
            ctx.get("config_var"),
            Some(&serde_json::Value::String("test_env_value".to_string()))
        );

        std::env::remove_var("TEST_ENV_VAR");
    }
}
