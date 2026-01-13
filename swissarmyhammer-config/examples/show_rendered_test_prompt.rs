/// Example showing what the rendered test prompt looks like with project detection
use swissarmyhammer_config::TemplateContext;

fn main() {
    println!("=== Rendered Test Prompt with Project Detection ===\n");

    // Create context with project detection
    let mut ctx = TemplateContext::new();
    ctx.set_default_variables();

    // Show what variables are available
    if let Some(project_types) = ctx.get("project_types") {
        let projects = project_types.as_array().expect("project_types should be array");
        println!("✅ Found {} detected projects in CEL context\n", projects.len());

        // Show first few projects
        for (i, project) in projects.iter().take(3).enumerate() {
            if let Some(obj) = project.as_object() {
                println!("Project {}:", i + 1);
                println!("  Type: {}", obj.get("type").unwrap());
                println!("  Path: {}", obj.get("path").unwrap());
                if let Some(commands) = obj.get("commands").and_then(|c| c.as_object()) {
                    if let Some(test_cmd) = commands.get("test") {
                        println!("  Test command: {}", test_cmd);
                    }
                }
                println!();
            }
        }
    } else {
        println!("❌ project_types not found in context!");
    }

    println!("The test.md prompt will render with:");
    println!("  • Detected Project Types section");
    println!("  • Project-specific guidelines (e.g., Rust → cargo nextest instructions)");
    println!("  • Commands for build, test, check, format");
    println!("  • Clear instructions to NOT use glob patterns");
    println!("\nTo see the actual workflow run:");
    println!("  cargo run -- --model GLM-4.7 test");
}
