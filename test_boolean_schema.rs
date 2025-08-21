use serde_json::json;
use swissarmyhammer_cli::schema_conversion::SchemaConverter;

fn main() {
    let schema = json!({
        "type": "object",
        "properties": {
            "string_field": {"type": "string", "description": "A string"},
            "bool_field": {"type": "boolean", "description": "A boolean"}
        },
        "required": ["string_field", "bool_field"]
    });
    
    let args = SchemaConverter::schema_to_clap_args(&schema).unwrap();
    
    for arg in &args {
        println!("Arg: {} - Required: {}", arg.get_id(), arg.is_required_set());
    }
}
