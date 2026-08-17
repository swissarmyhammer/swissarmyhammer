//! The Go probe pieces the shipped golangci-lint rules share.
//!
//! This set ships two rules that drive golangci-lint over one workspace, and
//! each of them is held to the same run shapes: a workspace holding no module,
//! a file golangci-lint cannot parse, and a file it may not read. Each shape
//! is a fact about golangci-lint rather than about one linter, so the bytes
//! that stage it stand here one time and both rules read them.
//!
//! What stays with each rule is the source it JUDGES and the words it breaks
//! with. `funlen` needs a function of 170 statements and `mnd` needs one
//! unnamed literal, and neither module could state the other's finding.

/// Where the module manifest of a probe repository stands, as the work-list
/// holds the path.
pub(super) const GO_MODULE_MANIFEST_PATH: &str = "go.mod";

/// The module manifest of a probe repository.
///
/// golangci-lint loads packages rather than files, so a probe repository with
/// no manifest loads nothing. The `go` directive names an old release for the
/// reason the shipped `go.mod` fixture states: it is the lowest version the
/// probe needs, so the installed toolchain always satisfies it and never
/// downloads another one.
pub(super) const GO_MODULE_MANIFEST: &str = "module function-length-probe\n\ngo 1.21\n";

/// The package clause a probe file of the `probe` package opens with.
pub(super) const GO_PACKAGE_CLAUSE: &str = "package probe\n\n";

/// Where the package holding the file golangci-lint cannot parse stands.
pub(super) const GO_UNPARSABLE_PATH: &str = "broken/broken.go";

/// A Go file golangci-lint cannot parse: the call in the return never closes.
pub(super) const GO_UNPARSABLE_SOURCE: &str = concat!(
    "package broken\n\n",
    "func Broken() int {\n\treturn undefinedSymbol(\n}\n",
);

/// What a run that met a file it could not parse must say, whichever rule
/// drove it.
///
/// golangci-lint reports such a file as a `typecheck` row, and its
/// `invalid_issue` processor then answers with the typecheck rows ALONE, so
/// the run reports no row of the linter the rule enabled.
pub(super) const GO_ANOTHER_LINTER_ERROR: &str = "golangci-lint reported a row of another linter";

/// Where the file nobody may read stands inside the probe module.
///
/// It holds a package of its own, and that package is the only one the probe
/// workspace carries: a file the tool cannot open is a package golangci-lint
/// cannot load, so a workspace holding that package alone measures nothing.
pub(super) const GO_UNREADABLE_PATH: &str = "noread/unreadable.go";

/// What the file nobody may read holds.
///
/// The source is ordinary: it stands under the statement gate `funlen`
/// measures, and its one literal stands in the `ignored-numbers` list `mnd`
/// reads. So a run that DID read it reports no finding for either rule —
/// which is the clean answer neither rule may give for a file it never read.
pub(super) const GO_UNREADABLE_SOURCE: &str =
    "package noread\n\nfunc Short() int {\n\treturn 1\n}\n";

/// The name the shipped scripts call the linter by.
pub(super) const GO_TOOL_BINARY_NAME: &str = "golangci-lint";

/// What a run that could not load one package must say beside the path
/// golangci-lint named, whichever rule drove it.
///
/// golangci-lint answers a status of its own for such a run — 7,
/// `ErrorWasLogged` — and the script names that status rather than reading as
/// a clean tree.
pub(super) const GO_BROKEN_STATUS_ERROR: &str = "golangci-lint exited";
