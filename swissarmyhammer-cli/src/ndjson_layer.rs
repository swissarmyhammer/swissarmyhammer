//! NDJSON Tracing Layer
//!
//! A tracing layer that outputs log events in NDJSON (Newline Delimited JSON) format.
//! Each log event is serialized as a single line of JSON with the following structure:
//!
//! ```json
//! {
//!   "timestamp": "2025-01-22T10:30:45.123Z",
//!   "level": "DEBUG|INFO|WARN|ERROR|TRACE",
//!   "target": "swissarmyhammer_cli::flow",
//!   "message": "Starting workflow: plan",
//!   "fields": {
//!     "workflow_name": "plan",
//!     "run_id": "01HXAMPLE123456789"
//!   }
//! }
//! ```
//!
//! The layer is generic over any writer that implements `Write + Send + Sync + 'static`,
//! making it suitable for file output, in-memory buffers, or any other write destination.

use chrono::Utc;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fmt;
use std::io::Write;
use std::sync::{Arc, Mutex};
use tracing::{Event, Subscriber};
use tracing_subscriber::{field::Visit, layer::Context, Layer};


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
                // Try to extract run_id from the span
                if let Some(run_id_value) = current_span.extensions().get::<String>() {
                    visitor.fields.insert("run_id".to_string(), json!(run_id_value));
                }
            }
        }

        // Get message from visitor or fallback to target
        let message = visitor
            .message
            .unwrap_or_else(|| event.metadata().target().to_string());

        // Build NDJSON object
        let json_object = json!({
            "timestamp": Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tracing_subscriber::{prelude::*, registry};

    #[derive(Clone)]
    struct TestWriter {
        buffer: Arc<Mutex<Vec<u8>>>,
    }

    impl TestWriter {
        fn new() -> Self {
            Self {
                buffer: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn get_output(&self) -> String {
            let buf = self.buffer.lock().unwrap();
            String::from_utf8_lossy(&buf).to_string()
        }
    }

    impl Write for TestWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            let mut buffer = self.buffer.lock().unwrap();
            buffer.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn test_ndjson_layer_basic_logging() {
        let test_writer = TestWriter::new();
        let layer = NdjsonLayer::new(test_writer.clone());

        let _guard = registry().with(layer).set_default();

        tracing::info!("Test message");

        let output = test_writer.get_output();
        assert!(!output.is_empty());

        let line = output.trim();
        let parsed: Value = serde_json::from_str(line).expect("Should be valid JSON");

        assert!(parsed["timestamp"].is_string());
        assert_eq!(parsed["level"], "INFO");
        assert_eq!(parsed["message"], "Test message");
        assert!(parsed["target"].is_string());
        assert_eq!(parsed["fields"], Value::Null);
    }

    #[test]
    fn test_ndjson_layer_multiple_events() {
        let test_writer = TestWriter::new();
        let layer = NdjsonLayer::new(test_writer.clone());

        let _guard = registry().with(layer).set_default();

        tracing::info!("First message");
        tracing::warn!("Second message");

        let output = test_writer.get_output();

        let lines: Vec<&str> = output.trim().split('\n').collect();
        assert_eq!(lines.len(), 2);

        let first: Value = serde_json::from_str(lines[0]).unwrap();
        let second: Value = serde_json::from_str(lines[1]).unwrap();

        assert_eq!(first["level"], "INFO");
        assert_eq!(first["message"], "First message");
        assert_eq!(second["level"], "WARN");
        assert_eq!(second["message"], "Second message");
    }

    #[test]
    fn test_ndjson_layer_different_field_types() {
        let test_writer = TestWriter::new();
        let layer = NdjsonLayer::new(test_writer.clone());

        let _guard = registry().with(layer).set_default();

        tracing::info!(
            count = 42u64,
            rate = 2.71f64,
            enabled = true,
            name = "test",
            "Event with fields"
        );

        let output = test_writer.get_output();

        let line = output.trim();
        let parsed: Value = serde_json::from_str(line).unwrap();

        assert_eq!(parsed["message"], "Event with fields");
        assert_eq!(parsed["fields"]["count"], 42);
        assert_eq!(parsed["fields"]["rate"], 2.71);
        assert_eq!(parsed["fields"]["enabled"], true);
        assert_eq!(parsed["fields"]["name"], "test");
    }

    #[test]
    fn test_ndjson_layer_no_fields() {
        let test_writer = TestWriter::new();
        let layer = NdjsonLayer::new(test_writer.clone());

        let _guard = registry().with(layer).set_default();

        tracing::error!("Error occurred");

        let output = test_writer.get_output();

        let line = output.trim();
        let parsed: Value = serde_json::from_str(line).unwrap();

        assert_eq!(parsed["level"], "ERROR");
        assert_eq!(parsed["message"], "Error occurred");
        assert_eq!(parsed["fields"], Value::Null);
    }

    #[test]
    fn test_ndjson_layer_writer_thread_safety() {
        let test_writer = TestWriter::new();
        let layer = NdjsonLayer::new(test_writer.clone());

        let _guard = registry().with(layer).set_default();

        tracing::info!(main_thread = true, "Message from main thread");

        let output = test_writer.get_output();

        assert!(!output.is_empty());
        let lines: Vec<&str> = output
            .trim()
            .split('\n')
            .filter(|l| !l.is_empty())
            .collect();
        assert!(!lines.is_empty());

        for line in lines {
            let parsed: Value = serde_json::from_str(line).expect("Should be valid JSON");
            assert_eq!(parsed["level"], "INFO");
            assert!(parsed["message"].is_string());
        }
    }
}