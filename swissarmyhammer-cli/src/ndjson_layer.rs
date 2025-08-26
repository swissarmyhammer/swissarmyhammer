//! NDJSON Tracing Layer
//!
//! A tracing layer that outputs log events in NDJSON (Newline Delimited JSON) format.

// Use std time instead of chrono for timestamp
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fmt;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{Event, Subscriber};
use tracing_subscriber::{field::Visit, layer::Context, Layer};

/// Writer that routes debug logs to workflow-specific directories
pub struct WorkflowDebugWriter {
    current_file: Arc<Mutex<Option<std::fs::File>>>,
}

impl WorkflowDebugWriter {
    pub fn new() -> Self {
        Self {
            current_file: Arc::new(Mutex::new(None)),
        }
    }

    fn setup_workflow_file(&self, run_id: &str) -> std::io::Result<()> {
        let workflow_runs_path = PathBuf::from(".swissarmyhammer/workflow-runs");
        let run_dir = workflow_runs_path
            .join("runs")
            .join(format!("WorkflowRunId({run_id})"));

        std::fs::create_dir_all(&run_dir)?;
        let debug_log_path = run_dir.join("run_logs.ndjson");

        let debug_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&debug_log_path)?;

        *self.current_file.lock().unwrap() = Some(debug_file);
        Ok(())
    }
}

impl Write for WorkflowDebugWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        // Look for debug setup message to extract run_id and setup proper file
        if let Ok(content) = std::str::from_utf8(buf) {
            if content.contains("Debug logging enabled for workflow run") {
                if let Ok(json) = serde_json::from_str::<Value>(content.trim()) {
                    if let Some(fields) = json.get("fields") {
                        if let Some(run_id) = fields.get("run_id") {
                            if let Some(run_id_str) = run_id.as_str() {
                                let _ = self.setup_workflow_file(run_id_str);
                            }
                        }
                    }
                }
            }
        }

        // Write to workflow-specific file if available, otherwise fallback
        if let Ok(mut file_opt) = self.current_file.lock() {
            if let Some(ref mut file) = *file_opt {
                return file.write(buf);
            }
        }

        // Fallback: write to .swissarmyhammer/debug.ndjson
        let fallback_path = PathBuf::from(".swissarmyhammer/debug.ndjson");
        if let Some(parent) = fallback_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut fallback_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(fallback_path)?;
        fallback_file.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if let Ok(mut file_opt) = self.current_file.lock() {
            if let Some(ref mut file) = *file_opt {
                return file.flush();
            }
        }
        Ok(())
    }
}

/// A tracing layer that writes log events to NDJSON format
pub struct NdjsonLayer<W> {
    writer: Arc<Mutex<W>>,
}

impl<W> NdjsonLayer<W>
where
    W: Write + Send + Sync + 'static,
{
    /// Create a new NDJSON layer with the given writer
    pub fn new(writer: W) -> Self {
        Self {
            writer: Arc::new(Mutex::new(writer)),
        }
    }
}

/// Field visitor for extracting custom fields from tracing events
#[derive(Default)]
struct FieldVisitor {
    fields: HashMap<String, Value>,
    message: Option<String>,
}

impl Visit for FieldVisitor {
    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        if field.name() == "message" {
            self.message = Some(value.to_string());
        } else {
            self.fields.insert(field.name().to_string(), json!(value));
        }
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        if field.name() == "message" {
            self.message = Some(value.to_string());
        } else {
            self.fields.insert(field.name().to_string(), json!(value));
        }
    }

    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        if field.name() == "message" {
            self.message = Some(value.to_string());
        } else {
            self.fields.insert(field.name().to_string(), json!(value));
        }
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        if field.name() == "message" {
            self.message = Some(value.to_string());
        } else {
            self.fields.insert(field.name().to_string(), json!(value));
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message = Some(value.to_string());
        } else {
            self.fields.insert(field.name().to_string(), json!(value));
        }
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn fmt::Debug) {
        if field.name() == "message" {
            self.message = Some(format!("{value:?}"));
        } else {
            self.fields
                .insert(field.name().to_string(), json!(format!("{value:?}")));
        }
    }
}

impl<S, W> Layer<S> for NdjsonLayer<W>
where
    S: Subscriber + for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
    W: Write + Send + Sync + 'static,
{
    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        // Extract fields from the event
        let mut visitor = FieldVisitor::default();
        event.record(&mut visitor);

        // Extract span context to get run_id and other span fields
        if let Some(current_span) = ctx.lookup_current() {
            // Get span name to check if it's our workflow execution span
            if current_span.metadata().name() == "workflow_execution" {
                // The span was created with run_id field, try to access it
                // For now, we'll use a simpler approach and just mark that we're in a workflow context
                visitor
                    .fields
                    .insert("workflow_context".to_string(), json!(true));
            }
        }

        // Get message from visitor or fallback to target
        let message = visitor
            .message
            .unwrap_or_else(|| event.metadata().target().to_string());

        // Build NDJSON object with ISO 8601 timestamp
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let iso_timestamp = format!(
            "2025-08-26T{:02}:{:02}:{:02}.{:03}Z",
            (timestamp / 3600000) % 24,
            (timestamp / 60000) % 60,
            (timestamp / 1000) % 60,
            timestamp % 1000
        );

        let json_object = json!({
            "timestamp": iso_timestamp,
            "level": event.metadata().level().to_string(),
            "target": event.metadata().target(),
            "message": message,
            "fields": if visitor.fields.is_empty() { Value::Null } else { json!(visitor.fields) }
        });

        // Write to output (ignore errors to prevent logging from blocking execution)
        if let Ok(mut writer) = self.writer.lock() {
            let _ = writeln!(writer, "{json_object}");
            let _ = writer.flush();
        }
    }
}
