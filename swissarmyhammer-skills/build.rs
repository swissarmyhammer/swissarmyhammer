use swissarmyhammer_build::BuiltinGenerator;

fn main() {
    BuiltinGenerator::new("skills")
        .source_dir("../builtin/skills")
        .extensions(&["md"])
        .function_name("get_builtin_skills")
        .generate();
}
