use swissarmyhammer_build::BuiltinGenerator;

fn main() {
    BuiltinGenerator::new("validators")
        .extensions(&["md"])
        .generate();
}
