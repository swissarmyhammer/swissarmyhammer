use swissarmyhammer_build::BuiltinGenerator;

fn main() {
    // RuleSets are now loaded directly from builtin/validators/ at runtime
    // No build-time generation needed for validators

    // Generate builtin YAML includes (file_groups, etc.)
    // These are loaded from the root builtin/ directory
    // Any .yaml/.yml file anywhere in builtin/ can be referenced with @path/name
    // Skip models/ as those are multi-document YAML for a different purpose
    BuiltinGenerator::new("includes")
        .source_dir("../builtin")
        .extensions(&["yaml", "yml"])
        .skip_dirs(&["models", "prompts", "validators", "rules", "flows"])
        .generate();
}
