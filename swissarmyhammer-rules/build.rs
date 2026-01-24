use swissarmyhammer_build::BuiltinGenerator;

fn main() {
    BuiltinGenerator::new("rules")
        .extensions(&["md", "liquid"])
        .generate();
}
