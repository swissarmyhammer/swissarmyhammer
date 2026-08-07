use swissarmyhammer_build::BuiltinGenerator;

fn main() {
    // Embed the builtin validators so the profile installer can materialize
    // them onto disk at the validators store (`~/.validators/` global or
    // `./.validators/` project) — the same store-then-deploy contract the
    // builtin skills/agents/tools use.
    //
    // `all_extensions` takes every file of a set, not only its markdown: a set
    // carries its `VALIDATOR.md`, its `rules/*.md`, AND its tool-rule fixtures,
    // which are source files in whatever language the tool lints. A fixture
    // that never reaches the store makes doctor report the tool rule as
    // fixture-less, and the rule falls back to its prompt rule.
    //
    // `preserve_extensions` keeps the real filenames
    // (e.g. `code-hygiene/VALIDATOR.md`) so the multi-file set structure is
    // reproduced verbatim when written to disk.
    // `target` is a build artifact a fixture's own tool writes beside it (a
    // cargo fixture package builds there); it is gitignored and never a
    // validator file.
    BuiltinGenerator::new("validators")
        .source_dir("../../builtin/validators")
        .all_extensions()
        .skip_dirs(&["target"])
        .function_name("get_builtin_validators")
        .preserve_extensions()
        .generate();
}
