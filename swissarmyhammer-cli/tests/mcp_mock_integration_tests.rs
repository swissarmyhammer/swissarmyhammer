use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use swissarmyhammer::PromptLibrary;
use tempfile::TempDir;
use tokio::sync::RwLock;

/// Mock MCP client for testing
struct MockMcpClient {
    prompts: Arc<RwLock<Vec<MockPrompt>>>,
}

#[derive(Clone, Debug)]
struct MockPrompt {
    name: String,
    description: String,
    parameters: Vec<MockArgument>,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
struct MockArgument {
    name: String,
    description: String,
    required: bool,
}

impl MockMcpClient {
    fn new() -> Self {
        Self {
            prompts: Arc::new(RwLock::new(Vec::new())),
        }
    }

    async fn list_prompts(&self) -> Vec<MockPrompt> {
        self.prompts.read().await.clone()
    }

    async fn get_prompt(
        &self,
        name: &str,
        args: Option<HashMap<String, String>>,
    ) -> Result<String> {
        let prompts = self.prompts.read().await;
        let _prompt = prompts
            .iter()
            .find(|p| p.name == name)
            .ok_or_else(|| anyhow::anyhow!("Prompt not found: {}", name))?;

        // Simulate template rendering
        match name {
            "simple" => Ok("Hello, world!".to_string()),
            "with_args" => {
                if let Some(args) = args {
                    let name = args
                        .get("name")
                        .ok_or_else(|| anyhow::anyhow!("Missing required argument: name"))?;
                    let age = args
                        .get("age")
                        .ok_or_else(|| anyhow::anyhow!("Missing required argument: age"))?;
                    Ok(format!("Hello {name}, you are {age} years old"))
                } else {
                    Err(anyhow::anyhow!("Missing required arguments"))
                }
            }
            "optional_args" => {
                if let Some(args) = args {
                    let name = args
                        .get("name")
                        .ok_or_else(|| anyhow::anyhow!("Missing required argument: name"))?;
                    let default_greeting = "Hello".to_string();
                    let default_punctuation = "!".to_string();
                    let greeting = args.get("greeting").unwrap_or(&default_greeting);
                    let punctuation = args.get("punctuation").unwrap_or(&default_punctuation);
                    Ok(format!("{greeting} {name}{punctuation}"))
                } else {
                    Err(anyhow::anyhow!("Missing required argument: name"))
                }
            }
            _ => Ok(format!("Rendered prompt: {name}")),
        }
    }

    async fn add_prompt(&self, prompt: MockPrompt) {
        self.prompts.write().await.push(prompt);
    }
}

async fn setup_test_environment() -> Result<(MockMcpClient, PromptLibrary, TempDir)> {
    let temp_dir = TempDir::new()?;
    let prompts_dir = temp_dir.path().join("prompts");
    std::fs::create_dir_all(&prompts_dir)?;

    // Create test prompts
    let test_prompts = vec![
        ("simple", "Hello, world!", vec![]),
        (
            "with_args",
            "Hello {{name}}, you are {{age}} years old",
            vec![("name", "User's name", true), ("age", "User's age", true)],
        ),
        (
            "optional_args",
            "{{greeting|Hello}} {{name}}{{punctuation|!}}",
            vec![
                ("name", "User's name", true),
                ("greeting", "Custom greeting", false),
                ("punctuation", "End punctuation", false),
            ],
        ),
    ];

    let mut library = PromptLibrary::new();
    let client = MockMcpClient::new();

    for (name, template, args) in test_prompts {
        // Create prompt file
        let prompt_file = prompts_dir.join(format!("{name}.prompt"));
        let mut yaml_content = String::from("---\n");
        yaml_content.push_str(&format!("name: {name}\n"));
        yaml_content.push_str(&format!("description: Test prompt for {name}\n"));

        if !args.is_empty() {
            yaml_content.push_str("parameters:\n");
            for (arg_name, desc, required) in &args {
                yaml_content.push_str(&format!("  - name: {arg_name}\n"));
                yaml_content.push_str(&format!("    description: {desc}\n"));
                yaml_content.push_str(&format!("    required: {required}\n"));
            }
        }

        yaml_content.push_str("---\n");
        yaml_content.push_str(template);

        std::fs::write(&prompt_file, yaml_content)?;

        // Add to mock client
        let mock_prompt = MockPrompt {
            name: name.to_string(),
            description: format!("Test prompt for {name}"),
            parameters: args
                .iter()
                .map(|(name, desc, required)| MockArgument {
                    name: name.to_string(),
                    description: desc.to_string(),
                    required: *required,
                })
                .collect(),
        };
        client.add_prompt(mock_prompt).await;
    }

    // Load prompts into library
    library.add_directory(&prompts_dir)?;

    Ok((client, library, temp_dir))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_list_prompts() -> Result<()> {
        let (client, _library, _temp_dir) = setup_test_environment().await?;

        let client_prompts = client.list_prompts().await;

        // The mock client should have the prompts we added
        assert_eq!(client_prompts.len(), 3);

        // Verify all prompts are present
        let names: Vec<_> = client_prompts.iter().map(|p| &p.name).collect();
        assert!(names.contains(&&"simple".to_string()));
        assert!(names.contains(&&"with_args".to_string()));
        assert!(names.contains(&&"optional_args".to_string()));

        Ok(())
    }

    #[tokio::test]
    async fn test_get_prompt_simple() -> Result<()> {
        let (client, _library, _temp_dir) = setup_test_environment().await?;

        let result = client.get_prompt("simple", None).await?;
        assert_eq!(result, "Hello, world!");

        Ok(())
    }

    #[tokio::test]
    async fn test_get_prompt_with_args() -> Result<()> {
        let (client, _library, _temp_dir) = setup_test_environment().await?;

        let mut args = HashMap::new();
        args.insert("name".to_string(), "Alice".to_string());
        args.insert("age".to_string(), "30".to_string());

        let result = client.get_prompt("with_args", Some(args)).await?;
        assert_eq!(result, "Hello Alice, you are 30 years old");

        Ok(())
    }

    #[tokio::test]
    async fn test_get_prompt_missing_required_args() -> Result<()> {
        let (client, _library, _temp_dir) = setup_test_environment().await?;

        let mut args = HashMap::new();
        args.insert("name".to_string(), "Bob".to_string());
        // Missing required "age" argument

        let result = client.get_prompt("with_args", Some(args)).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Missing required argument: age"));

        Ok(())
    }

    #[tokio::test]
    async fn test_get_prompt_with_optional_args() -> Result<()> {
        let (client, _library, _temp_dir) = setup_test_environment().await?;

        // Test with only required args
        let mut args1 = HashMap::new();
        args1.insert("name".to_string(), "Charlie".to_string());

        let result1 = client.get_prompt("optional_args", Some(args1)).await?;
        assert_eq!(result1, "Hello Charlie!");

        // Test with all args
        let mut args2 = HashMap::new();
        args2.insert("name".to_string(), "David".to_string());
        args2.insert("greeting".to_string(), "Hi".to_string());
        args2.insert("punctuation".to_string(), "...".to_string());

        let result2 = client.get_prompt("optional_args", Some(args2)).await?;
        assert_eq!(result2, "Hi David...");

        Ok(())
    }

    #[tokio::test]
    async fn test_prompt_not_found() -> Result<()> {
        let (client, _library, _temp_dir) = setup_test_environment().await?;

        let result = client.get_prompt("nonexistent", None).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Prompt not found"));

        Ok(())
    }

    #[tokio::test]
    async fn test_concurrent_access() -> Result<()> {
        let (client, _library, _temp_dir) = setup_test_environment().await?;
        let client = Arc::new(client);

        let mut handles = vec![];

        // Spawn multiple tasks to access the client concurrently
        for i in 0..10 {
            let client_clone = client.clone();
            let handle = tokio::spawn(async move {
                let mut args = HashMap::new();
                args.insert("name".to_string(), format!("User{i}"));
                args.insert("age".to_string(), format!("{}", 20 + i));
                client_clone.get_prompt("with_args", Some(args)).await
            });
            handles.push(handle);
        }

        // All should succeed
        for (i, handle) in handles.into_iter().enumerate() {
            let result = handle.await??;
            assert_eq!(
                result,
                format!("Hello User{}, you are {} years old", i, 20 + i)
            );
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_real_time_updates() -> Result<()> {
        let client = MockMcpClient::new();

        // Initial state
        let prompts = client.list_prompts().await;
        assert_eq!(prompts.len(), 0);

        // Add a prompt
        client
            .add_prompt(MockPrompt {
                name: "dynamic".to_string(),
                description: "Dynamically added prompt".to_string(),
                parameters: vec![],
            })
            .await;

        // Verify it's available
        let prompts = client.list_prompts().await;
        assert_eq!(prompts.len(), 1);
        assert_eq!(prompts[0].name, "dynamic");

        Ok(())
    }

    #[tokio::test]
    async fn test_template_validation() -> Result<()> {
        let (_client, mut library, temp_dir) = setup_test_environment().await?;

        // Create a prompt with invalid template syntax
        let prompts_dir = temp_dir.path().join("prompts");
        let invalid_prompt_file = prompts_dir.join("invalid.prompt");
        let content = r#"---
name: invalid
description: Invalid template
---
This has invalid syntax {{unclosed"#;
        std::fs::write(&invalid_prompt_file, content)?;

        // Try to load it
        let result = library.add_directory(&prompts_dir);
        // The library should still load, but rendering might fail
        assert!(result.is_ok());

        Ok(())
    }

    #[tokio::test]
    async fn test_performance_with_many_prompts() -> Result<()> {
        let client = MockMcpClient::new();

        // Add many prompts
        let start = std::time::Instant::now();
        for i in 0..1000 {
            client
                .add_prompt(MockPrompt {
                    name: format!("prompt_{i}"),
                    description: format!("Test prompt {i}"),
                    parameters: vec![
                        MockArgument {
                            name: "arg1".to_string(),
                            description: "First argument".to_string(),
                            required: true,
                        },
                        MockArgument {
                            name: "arg2".to_string(),
                            description: "Second argument".to_string(),
                            required: false,
                        },
                    ],
                })
                .await;
        }
        let add_duration = start.elapsed();

        // Should add quickly
        assert!(
            add_duration.as_secs() < 5,
            "Adding prompts took too long: {add_duration:?}"
        );

        // List all prompts
        let start = std::time::Instant::now();
        let prompts = client.list_prompts().await;
        let list_duration = start.elapsed();

        assert_eq!(prompts.len(), 1000);
        assert!(
            list_duration.as_millis() < 100,
            "Listing prompts took too long: {list_duration:?}"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_argument_validation() -> Result<()> {
        let (client, _library, _temp_dir) = setup_test_environment().await?;

        let prompts = client.list_prompts().await;
        let with_args_prompt = prompts.iter().find(|p| p.name == "with_args").unwrap();

        // Verify required arguments
        let required_args: Vec<_> = with_args_prompt
            .parameters
            .iter()
            .filter(|a| a.required)
            .map(|a| &a.name)
            .collect();

        assert_eq!(required_args.len(), 2);
        assert!(required_args.iter().any(|n| n.as_str() == "name"));
        assert!(required_args.iter().any(|n| n.as_str() == "age"));

        Ok(())
    }

    #[tokio::test]
    async fn test_prompt_metadata() -> Result<()> {
        let (client, _library, _temp_dir) = setup_test_environment().await?;

        let prompts = client.list_prompts().await;

        for prompt in prompts {
            assert!(!prompt.name.is_empty());
            assert!(!prompt.description.is_empty());

            // Check argument metadata
            if prompt.name == "optional_args" {
                assert_eq!(prompt.parameters.len(), 3);

                let name_arg = prompt.parameters.iter().find(|a| a.name == "name").unwrap();
                assert!(name_arg.required);

                let greeting_arg = prompt
                    .parameters
                    .iter()
                    .find(|a| a.name == "greeting")
                    .unwrap();
                assert!(!greeting_arg.required);
            }
        }

        Ok(())
    }
}
