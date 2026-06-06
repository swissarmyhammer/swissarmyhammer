use swissarmyhammer_build::BuiltinGenerator;

fn main() {
    // Embed the builtin validators (VALIDATOR.md + rules/*.md, per set) so the
    // profile installer can materialize them onto disk at
    // `$XDG_DATA_HOME/validators/` — the same store-then-deploy contract the
    // builtin skills/agents use. `preserve_extensions` keeps the real filenames
    // (e.g. `code-quality/VALIDATOR.md`) so the multi-file set structure is
    // reproduced verbatim when written to disk.
    BuiltinGenerator::new("validators")
        .source_dir("../../builtin/validators")
        .extensions(&["md"])
        .function_name("get_builtin_validators")
        .preserve_extensions()
        .generate();
}
