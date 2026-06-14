use swissarmyhammer_build::BuiltinGenerator;

fn main() {
    // Partials live at builtin/_partials/ (shared across prompts, skills, and agents)
    BuiltinGenerator::new("partials")
        .source_dir("../../builtin/_partials")
        .extensions(&["md", "liquid"])
        .function_name("get_builtin_partials")
        .generate();
}
