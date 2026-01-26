use swissarmyhammer_build::BuiltinGenerator;

fn main() {
    // Generate builtin validators
    BuiltinGenerator::new("validators")
        .extensions(&["md"])
        .generate();

    // Generate builtin YAML includes (file_groups, etc.)
    // These are loaded from the root builtin/ directory
    // Any .yaml/.yml file anywhere in builtin/ can be referenced with @path/name
    BuiltinGenerator::new("includes")
        .source_dir("../builtin")
        .extensions(&["yaml", "yml"])
        .generate();
}
