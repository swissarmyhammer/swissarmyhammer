//! Model list command implementation

use crate::cli::OutputFormat;
use crate::context::CliContext;
use anyhow::Result;
use comfy_table::Table;
use swissarmyhammer_config::model::{ModelConfigSource, ModelManager};

/// Execute the model list command - shows all available models
pub async fn execute_list_command(
    format: OutputFormat,
    context: &CliContext,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing::debug!("Starting model list command");

    // Load all models using ModelManager
    let models = match ModelManager::list_agents() {
        Ok(models) => models,
        Err(e) => {
            tracing::error!("Failed to load models: {}", e);
            return Err(format!("Failed to discover models: {}", e).into());
        }
    };

    // Use the provided format directly
    let output_format = format;

    // For table format, show summary information
    if matches!(output_format, OutputFormat::Table) {
        display_model_summary_and_table(&models, context.verbose)?;
    } else {
        // For JSON/YAML formats, just display the data directly
        let display_rows = super::display::agents_to_display_rows(models, context.verbose);
        match display_rows {
            super::display::DisplayRows::Standard(items) => {
                display_items_with_format(&items, output_format)?
            }
            super::display::DisplayRows::Verbose(items) => {
                display_items_with_format(&items, output_format)?
            }
        }
    }

    Ok(())
}

/// Display model summary information followed by a table
fn display_model_summary_and_table(
    models: &[swissarmyhammer_config::model::ModelInfo],
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (builtin_count, project_count, gitroot_count, user_count) = count_models_by_source(models);
    display_model_counts(
        models.len(),
        builtin_count,
        project_count,
        gitroot_count,
        user_count,
    );
    display_model_table(models, verbose)?;
    Ok(())
}

/// Count models by source type
fn count_models_by_source(
    models: &[swissarmyhammer_config::model::ModelInfo],
) -> (usize, usize, usize, usize) {
    let mut builtin_count = 0;
    let mut project_count = 0;
    let mut gitroot_count = 0;
    let mut user_count = 0;

    for model in models {
        match model.source {
            ModelConfigSource::Builtin => builtin_count += 1,
            ModelConfigSource::Project => project_count += 1,
            ModelConfigSource::GitRoot => gitroot_count += 1,
            ModelConfigSource::User => user_count += 1,
        }
    }

    (builtin_count, project_count, gitroot_count, user_count)
}

/// Display model count summary
fn display_model_counts(total: usize, builtin: usize, project: usize, gitroot: usize, user: usize) {
    println!("Models: {}", total);

    let counts = [
        ("Built-in", builtin),
        ("Project", project),
        ("GitRoot", gitroot),
        ("User", user),
    ];

    for (label, count) in counts {
        if count > 0 {
            println!("{}: {}", label, count);
        }
    }

    println!(); // Empty line before table
}

/// Display model table based on display rows
fn display_model_table(
    models: &[swissarmyhammer_config::model::ModelInfo],
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let display_rows = super::display::agents_to_display_rows(models.to_vec(), verbose);

    match display_rows {
        super::display::DisplayRows::Standard(items) => display_standard_table(&items),
        super::display::DisplayRows::Verbose(items) => display_verbose_table(&items),
    }
}

/// Display a standard table with Name, Description, and Source columns
fn display_standard_table(
    items: &[super::display::AgentRow],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if items.is_empty() {
        println!("No models available");
        return Ok(());
    }

    let table = create_table(items, vec!["Name", "Description", "Source"], |item| {
        vec![&item.name, &item.description, &item.source]
    });
    println!("{table}");
    Ok(())
}

/// Display a verbose table with Name, Description, Source, and Content Size columns
fn display_verbose_table(
    items: &[super::display::VerboseAgentRow],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if items.is_empty() {
        println!("No models available");
        return Ok(());
    }

    let table = create_table(
        items,
        vec!["Name", "Description", "Source", "Content Size"],
        |item| {
            vec![
                &item.name,
                &item.description,
                &item.source,
                &item.content_size,
            ]
        },
    );
    println!("{table}");
    Ok(())
}

/// Create a table with given headers and row mapper function
fn create_table<'a, T, F>(items: &'a [T], headers: Vec<&str>, row_mapper: F) -> Table
where
    F: Fn(&'a T) -> Vec<&'a str>,
{
    let mut table = swissarmyhammer_doctor::new_table();
    table.set_header(headers);

    add_table_rows(&mut table, items, row_mapper);

    table
}

/// Add rows to table using the provided mapper function
fn add_table_rows<'a, T, F>(table: &mut Table, items: &'a [T], row_mapper: F)
where
    F: Fn(&'a T) -> Vec<&'a str>,
{
    for item in items {
        table.add_row(row_mapper(item));
    }
}

/// Capitalize the first character of a string.
///
/// Takes a string slice and returns a new String with the first character converted to uppercase
/// while preserving the case of remaining characters. Returns an empty string if the input is empty.
///
/// # Arguments
///
/// * `s` - The string slice to capitalize
///
/// # Returns
///
/// A new String with the first character capitalized, or an empty String if input is empty
///
/// # Examples
///
/// ```
/// let result = capitalize_first("hello");
/// assert_eq!(result, "Hello");
///
/// let result = capitalize_first("WORLD");
/// assert_eq!(result, "WORLD");
///
/// let result = capitalize_first("");
/// assert_eq!(result, "");
/// ```
fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// Format a JSON value as a string.
///
/// Converts a serde_json::Value into a String representation suitable for display.
/// - String values are returned as-is
/// - Numbers are converted to their string representation
/// - Booleans are converted to "true" or "false"
/// - Null values are converted to "null"
/// - Complex types (arrays, objects) are serialized to JSON string format
///
/// # Arguments
///
/// * `v` - The JSON value to format
///
/// # Returns
///
/// A String representation of the JSON value
///
/// # Examples
///
/// ```
/// use serde_json::json;
///
/// let string_val = json!("hello");
/// assert_eq!(format_json_value(&string_val), "hello");
///
/// let number_val = json!(42);
/// assert_eq!(format_json_value(&number_val), "42");
///
/// let bool_val = json!(true);
/// assert_eq!(format_json_value(&bool_val), "true");
///
/// let null_val = json!(null);
/// assert_eq!(format_json_value(&null_val), "null");
/// ```
fn format_json_value(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => "null".to_string(),
        _ => v.to_string(),
    }
}

/// Build a dynamic table from JSON items.
///
/// Creates a formatted table from a JSON array of objects. The function uses the keys from the
/// first object as column headers (capitalized) and populates rows with values from each object.
/// All objects in the array are expected to have the same structure.
///
/// # Arguments
///
/// * `items` - A JSON value that must be an array of objects
///
/// # Returns
///
/// A Result containing the built Table or an error if:
/// - The input is not a JSON array
/// - The array is empty
/// - Array elements are not JSON objects
///
/// # Errors
///
/// Returns an error if:
/// - Input is not an array
/// - Array is empty
/// - Array contains non-object values
///
/// # Examples
///
/// ```
/// use serde_json::json;
///
/// let items = json!([
///     {"name": "Alice", "age": 30},
///     {"name": "Bob", "age": 25}
/// ]);
///
/// let table = build_dynamic_table(&items)?;
/// // Table will have headers "Name" and "Age" with two data rows
/// ```
fn build_dynamic_table(
    items: &serde_json::Value,
) -> Result<Table, Box<dyn std::error::Error + Send + Sync>> {
    let array = extract_json_array(items)?;
    let first_obj = extract_first_object(array)?;

    let mut table = swissarmyhammer_doctor::new_table();

    add_table_headers(&mut table, first_obj);
    add_json_rows(&mut table, array)?;

    Ok(table)
}

/// Extract and validate JSON array from value
fn extract_json_array(
    items: &serde_json::Value,
) -> Result<&Vec<serde_json::Value>, Box<dyn std::error::Error + Send + Sync>> {
    let array = items.as_array().ok_or("Expected JSON array")?;

    if array.is_empty() {
        return Err("No items to display".into());
    }

    Ok(array)
}

/// Extract and validate first object from JSON array
fn extract_first_object(
    array: &[serde_json::Value],
) -> Result<&serde_json::Map<String, serde_json::Value>, Box<dyn std::error::Error + Send + Sync>> {
    let first = array.first().ok_or("Array is empty")?;
    let first_obj = first.as_object().ok_or("Expected JSON object")?;
    Ok(first_obj)
}

/// Add capitalized headers to table from JSON object keys
fn add_table_headers(table: &mut Table, obj: &serde_json::Map<String, serde_json::Value>) {
    let headers: Vec<String> = obj.keys().map(|k| capitalize_first(k)).collect();
    table.set_header(headers);
}

/// Add data rows from JSON array to table
fn add_json_rows(
    table: &mut Table,
    array: &[serde_json::Value],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    for item in array {
        let obj = item.as_object().ok_or("Expected JSON object in array")?;
        let row: Vec<String> = obj.values().map(format_json_value).collect();
        table.add_row(row);
    }
    Ok(())
}

/// Display items as a table.
///
/// Converts a slice of serializable items into a formatted table and prints it to stdout.
/// Items are first serialized to JSON, then converted to a table format with headers derived
/// from the object keys. If the items slice is empty, displays a "No items to display" message.
///
/// # Type Parameters
///
/// * `T` - Any type that implements serde::Serialize
///
/// # Arguments
///
/// * `items` - A slice of items to display
///
/// # Returns
///
/// A Result indicating success or failure. Returns Ok(()) on success.
///
/// # Errors
///
/// Returns an error if:
/// - Serialization to JSON fails
/// - Table building fails (invalid JSON structure)
///
/// # Examples
///
/// ```
/// #[derive(Serialize)]
/// struct Person {
///     name: String,
///     age: u32,
/// }
///
/// let people = vec![
///     Person { name: "Alice".to_string(), age: 30 },
///     Person { name: "Bob".to_string(), age: 25 },
/// ];
///
/// display_as_table(&people)?;
/// // Prints formatted table to stdout
/// ```
fn display_as_table<T>(items: &[T]) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    T: serde::Serialize,
{
    if items.is_empty() {
        println!("No items to display");
        return Ok(());
    }

    let json_items = serde_json::to_value(items)?;
    let table = build_dynamic_table(&json_items)?;
    println!("{table}");
    Ok(())
}

/// Display items as JSON.
///
/// Serializes a slice of items to pretty-printed JSON format and prints it to stdout.
/// The output is formatted with indentation for readability.
///
/// # Type Parameters
///
/// * `T` - Any type that implements serde::Serialize
///
/// # Arguments
///
/// * `items` - A slice of items to serialize and display
///
/// # Returns
///
/// A Result indicating success or failure. Returns Ok(()) on success.
///
/// # Errors
///
/// Returns an error if JSON serialization fails
///
/// # Examples
///
/// ```
/// #[derive(Serialize)]
/// struct Config {
///     name: String,
///     enabled: bool,
/// }
///
/// let configs = vec![
///     Config { name: "debug".to_string(), enabled: true },
///     Config { name: "release".to_string(), enabled: false },
/// ];
///
/// display_as_json(&configs)?;
/// // Prints:
/// // [
/// //   {
/// //     "name": "debug",
/// //     "enabled": true
/// //   },
/// //   {
/// //     "name": "release",
/// //     "enabled": false
/// //   }
/// // ]
/// ```
fn display_as_json<T>(items: &[T]) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    T: serde::Serialize,
{
    let json = serde_json::to_string_pretty(items)?;
    println!("{}", json);
    Ok(())
}

/// Display items as YAML.
///
/// Serializes a slice of items to YAML format and prints it to stdout.
/// The output follows standard YAML formatting conventions.
///
/// # Type Parameters
///
/// * `T` - Any type that implements serde::Serialize
///
/// # Arguments
///
/// * `items` - A slice of items to serialize and display
///
/// # Returns
///
/// A Result indicating success or failure. Returns Ok(()) on success.
///
/// # Errors
///
/// Returns an error if YAML serialization fails
///
/// # Examples
///
/// ```
/// #[derive(Serialize)]
/// struct Config {
///     name: String,
///     enabled: bool,
/// }
///
/// let configs = vec![
///     Config { name: "debug".to_string(), enabled: true },
///     Config { name: "release".to_string(), enabled: false },
/// ];
///
/// display_as_yaml(&configs)?;
/// // Prints:
/// // - name: debug
/// //   enabled: true
/// // - name: release
/// //   enabled: false
/// ```
fn display_as_yaml<T>(items: &[T]) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    T: serde::Serialize,
{
    let yaml = serde_yaml::to_string(items)?;
    println!("{}", yaml);
    Ok(())
}

/// Display items using the specified format.
///
/// Routes to the appropriate display function based on the requested output format.
/// Supports table, JSON, and YAML formats. This is the main dispatch function for
/// displaying data in different formats.
///
/// # Type Parameters
///
/// * `T` - Any type that implements serde::Serialize
///
/// # Arguments
///
/// * `items` - A slice of items to display
/// * `format` - The desired output format (Table, Json, or Yaml)
///
/// # Returns
///
/// A Result indicating success or failure. Returns Ok(()) on success.
///
/// # Errors
///
/// Returns an error if the underlying display function fails due to:
/// - Serialization errors
/// - Table building errors (for Table format)
///
/// # Examples
///
/// ```
/// #[derive(Serialize)]
/// struct Item {
///     id: u32,
///     name: String,
/// }
///
/// let items = vec![
///     Item { id: 1, name: "First".to_string() },
///     Item { id: 2, name: "Second".to_string() },
/// ];
///
/// // Display as table
/// display_items_with_format(&items, OutputFormat::Table)?;
///
/// // Display as JSON
/// display_items_with_format(&items, OutputFormat::Json)?;
///
/// // Display as YAML
/// display_items_with_format(&items, OutputFormat::Yaml)?;
/// ```
fn display_items_with_format<T>(
    items: &[T],
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    T: serde::Serialize,
{
    match format {
        OutputFormat::Table => display_as_table(items),
        OutputFormat::Json => display_as_json(items),
        OutputFormat::Yaml => display_as_yaml(items),
    }
}
