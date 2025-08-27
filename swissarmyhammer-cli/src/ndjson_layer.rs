//! NDJSON Tracing Layer
//!
//! A tracing layer that outputs log events in NDJSON (Newline Delimited JSON) format.

use serde_json::{json, Value};
use std::collections::HashMap;
use std::fmt;
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{Event, Subscriber};
use tracing_subscriber::{field::Visit, layer::Context, Layer};

/// Writer that writes debug logs directly to the workflow-specific file
pub struct WorkflowDebugWriter;

impl WorkflowDebugWriter {
    pub fn new() -> Self {
        Self
    }
}

impl Write for WorkflowDebugWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        // Get the global debug file if it exists
        if let Some(debug_file) = crate::commands::flow::DEBUG_FILE.get() {
            if let Ok(mut file) = debug_file.lock() {
                return file.write(buf);
            }
        }
        
        // If no debug file is set up, just return success (don't write anything)
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        // Get the global debug file if it exists
        if let Some(debug_file) = crate::commands::flow::DEBUG_FILE.get() {
            if let Ok(mut file) = debug_file.lock() {
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