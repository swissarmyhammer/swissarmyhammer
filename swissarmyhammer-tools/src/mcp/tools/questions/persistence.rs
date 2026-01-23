use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use swissarmyhammer_common::SwissarmyhammerDirectory;

/// A single question/answer entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuestionAnswerEntry {
    /// ISO 8601 timestamp
    pub timestamp: String,
    /// The question that was asked
    pub question: String,
    /// The user's answer
    pub answer: String,
}

/// Get the questions directory path
fn get_questions_dir() -> Result<PathBuf> {
    let current_dir = std::env::current_dir().context("Failed to get current directory")?;
    let questions_dir = current_dir
        .join(SwissarmyhammerDirectory::dir_name())
        .join("questions");
    Ok(questions_dir)
}

/// Save a question/answer pair to a YAML file
///
/// # Arguments
///
/// * `question` - The question that was asked
/// * `answer` - The user's answer
///
/// # Returns
///
/// * `Result<PathBuf>` - Path to the created file
///
/// # Errors
///
/// Returns an error if directory creation or file writing fails
pub fn save_question_answer(question: &str, answer: &str) -> Result<PathBuf> {
    let questions_dir = get_questions_dir()?;

    // Create directory if it doesn't exist
    std::fs::create_dir_all(&questions_dir).context("Failed to create questions directory")?;

    // Generate filename with timestamp (including microseconds to avoid collisions)
    let now = Utc::now();
    let timestamp_str = now.format("%Y%m%d_%H%M%S_%6f").to_string();
    let filename = format!("{}_question.yaml", timestamp_str);
    let file_path = questions_dir.join(filename);

    // Create entry
    let entry = QuestionAnswerEntry {
        timestamp: now.to_rfc3339(),
        question: question.to_string(),
        answer: answer.to_string(),
    };

    // Serialize to YAML with header comment
    let yaml_body = serde_yaml::to_string(&entry).context("Failed to serialize entry to YAML")?;
    let yaml_content = format!(
        "# Saved at {}\n{}",
        now.format("%Y-%m-%d %H:%M:%S UTC"),
        yaml_body
    );

    // Write to file
    std::fs::write(&file_path, yaml_content).context("Failed to write question file")?;

    tracing::info!("Saved question/answer to {}", file_path.display());

    Ok(file_path)
}

/// Load all question/answer entries from the questions directory
///
/// # Returns
///
/// * `Result<Vec<QuestionAnswerEntry>>` - All question/answer pairs, sorted by timestamp (oldest first)
///
/// # Errors
///
/// Returns an error if reading files fails. Individual file parsing errors are logged but not fatal.
pub fn load_all_questions() -> Result<Vec<QuestionAnswerEntry>> {
    let questions_dir = get_questions_dir()?;

    // Return empty vector if directory doesn't exist
    if !questions_dir.exists() {
        tracing::debug!(
            "Questions directory does not exist: {}",
            questions_dir.display()
        );
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();

    // Read all YAML files
    for entry_result in
        std::fs::read_dir(&questions_dir).context("Failed to read questions directory")?
    {
        let entry = match entry_result {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!("Failed to read directory entry: {}", e);
                continue;
            }
        };

        let path = entry.path();

        // Only process .yaml files
        if path.extension().is_none_or(|ext| ext != "yaml") {
            continue;
        }

        // Read and parse file
        match std::fs::read_to_string(&path) {
            Ok(content) => match serde_yaml::from_str::<QuestionAnswerEntry>(&content) {
                Ok(qa_entry) => entries.push(qa_entry),
                Err(e) => {
                    tracing::warn!("Failed to parse question file {}: {}", path.display(), e);
                    continue;
                }
            },
            Err(e) => {
                tracing::warn!("Failed to read question file {}: {}", path.display(), e);
                continue;
            }
        }
    }

    // Sort by timestamp (oldest first)
    entries.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use tempfile::TempDir;

    fn setup_test_env() -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();
        temp_dir
    }

    #[test]
    #[serial]
    fn test_save_question_answer() {
        let _temp = setup_test_env();

        let file_path =
            save_question_answer("What is your name?", "Alice").expect("Should save question");

        assert!(file_path.exists());
        assert!(file_path.to_str().unwrap().contains("question.yaml"));

        // Verify content
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("What is your name?"));
        assert!(content.contains("Alice"));
    }

    #[test]
    #[serial]
    fn test_load_all_questions_empty_dir() {
        let _temp = setup_test_env();

        let entries = load_all_questions().expect("Should handle empty directory");
        assert_eq!(entries.len(), 0);
    }

    #[test]
    #[serial]
    fn test_load_all_questions() {
        let _temp = setup_test_env();

        // Create multiple questions
        save_question_answer("Question 1", "Answer 1").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        save_question_answer("Question 2", "Answer 2").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        save_question_answer("Question 3", "Answer 3").unwrap();

        let entries = load_all_questions().expect("Should load all questions");
        assert_eq!(entries.len(), 3);

        // Verify sorting (oldest first)
        assert_eq!(entries[0].question, "Question 1");
        assert_eq!(entries[1].question, "Question 2");
        assert_eq!(entries[2].question, "Question 3");
    }

    #[test]
    #[serial]
    fn test_save_with_special_characters() {
        let _temp = setup_test_env();

        let question = r#"What's your "favorite" thing?"#;
        let answer = r#"My "answer" with quotes"#;

        let _file_path =
            save_question_answer(question, answer).expect("Should handle special characters");

        // Read back and verify
        let entries = load_all_questions().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].question, question);
        assert_eq!(entries[0].answer, answer);
    }
}
