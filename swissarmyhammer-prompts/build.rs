use swissarmyhammer_build::BuiltinGenerator;

fn main() {
    BuiltinGenerator::new("prompts")
        .extensions(&["md", "liquid"])
        .skip_dirs(&["workflows"])
        .generate();
}
