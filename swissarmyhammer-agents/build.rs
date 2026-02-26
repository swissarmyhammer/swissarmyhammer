use swissarmyhammer_build::BuiltinGenerator;

fn main() {
    BuiltinGenerator::new("agents")
        .source_dir("../builtin/agents")
        .extensions(&["md"])
        .function_name("get_builtin_agents")
        .generate();
}
