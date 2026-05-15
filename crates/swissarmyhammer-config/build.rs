use swissarmyhammer_build::BuiltinGenerator;

fn main() {
    BuiltinGenerator::new("models")
        .source_dir("../../builtin/models")
        .extensions(&["yaml"])
        .generate();
}
