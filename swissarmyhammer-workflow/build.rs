use swissarmyhammer_build::BuiltinGenerator;

fn main() {
    BuiltinGenerator::new("workflows")
        .extensions(&["md"])
        .generate();
}
