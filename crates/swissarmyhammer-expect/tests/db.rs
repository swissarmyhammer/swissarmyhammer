//! Integration coverage for the `db` surface adapter against a real, in-process
//! SQLite database.
//!
//! The adapter under test is synchronous (it uses the in-process `rusqlite`
//! client — no external server), so the test functions are plain `#[test]`s
//! exercising the production path: provision a fresh database + load the `setup:`
//! fixture, drive (run statements), observe (a SQL snapshot of the result), and
//! teardown (drop the database). The db locator dialect — a SQL query projecting
//! one scalar — is then compiled-free built and evaluated against the captured
//! snapshot, the way a hand-authored locator would be (`ideas/expect.md`
//! §"Locators are a per-surface dialect": the locator *is* SQL).

use std::time::Duration;

use swissarmyhammer_expect::{
    AssertOp, AssertionOutcome, BoundValue, Checkpoint, CompiledAssertion, DbAdapter, Expected,
    Locator, Observation, Setup, SurfaceAdapter, SurfaceState, Trajectory, VerdictTier,
};

/// The fixture schema + the single source of truth the projection assertions are
/// checked against: one `orders` row with a numeric total and a text label.
const FIXTURE_SCHEMA: &str =
    "CREATE TABLE orders (id INTEGER PRIMARY KEY, total INTEGER, label TEXT);";

/// The expected order total the SQL-projection locator must observe.
const EXPECTED_TOTAL: f64 = 40.0;

/// The expected order label (a textual projection).
const EXPECTED_LABEL: &str = "SAVE10";

/// Wrap an observed db state in a single-checkpoint observation so the SQL
/// locator dialect can be built and evaluated against it.
fn observation_of(state: SurfaceState) -> Observation {
    Observation {
        path: "fixture".to_string(),
        checkpoints: vec![Checkpoint {
            after: "final".to_string(),
            state,
            duration: Duration::from_millis(1),
        }],
        trajectory: Trajectory { steps: Vec::new() },
    }
}

/// A Tier-1 equality assertion over `locator` expecting `expected` at the only
/// checkpoint.
fn equals_assertion(locator: Locator, expected: BoundValue) -> CompiledAssertion {
    CompiledAssertion {
        checkpoint: 0,
        locator,
        op: AssertOp::Equals,
        expected: Expected::Literal { value: expected },
        tier: VerdictTier::Deterministic,
        criterion_text: "a db projection holds".to_string(),
    }
}

#[test]
fn provisions_a_fixture_runs_statements_and_a_sql_locator_observes_the_rows() {
    let adapter = DbAdapter::new();
    let repo = tempfile::TempDir::new().unwrap();
    let setup = Setup::Command(FIXTURE_SCHEMA.to_string());

    // Provision a fresh database and load the fixture schema.
    let mut sut = adapter
        .provision(Some(&setup), repo.path())
        .expect("provision");

    // Drive: run statements against the database.
    adapter
        .drive(
            &mut sut,
            "INSERT INTO orders (id, total, label) VALUES (1, 40, 'SAVE10');",
        )
        .expect("drive insert");

    // Observe: capture an authoritative SQL snapshot of the database.
    let state = adapter.observe(&sut).expect("observe");
    let SurfaceState::Db(db) = &state else {
        panic!("expected a db surface state, got {state:?}");
    };
    assert!(
        db.snapshot.contains("CREATE TABLE orders"),
        "snapshot captures the schema: {}",
        db.snapshot
    );
    assert!(
        db.snapshot.contains("INSERT INTO"),
        "snapshot captures the rows: {}",
        db.snapshot
    );

    let observation = observation_of(state.clone());
    let checkpoint_state = &observation.checkpoints[0].state;

    // A SQL-projection locator observes the numeric total.
    let total = Locator::Sql {
        query: "SELECT total FROM orders WHERE id = 1".to_string(),
    };
    assert_eq!(
        total.resolve(checkpoint_state),
        Some(BoundValue::Number(EXPECTED_TOTAL)),
        "the SQL-projection locator observes the expected total"
    );

    // A SQL projection of a text column resolves to text.
    let label = Locator::Sql {
        query: "SELECT label FROM orders WHERE id = 1".to_string(),
    };
    assert_eq!(
        label.resolve(checkpoint_state),
        Some(BoundValue::Text(EXPECTED_LABEL.to_string())),
        "a text projection resolves to a textual value"
    );

    // A count projection observes the number of rows.
    let count = Locator::Sql {
        query: "SELECT COUNT(*) FROM orders".to_string(),
    };
    assert_eq!(
        count.resolve(checkpoint_state),
        Some(BoundValue::Number(1.0)),
        "a count projection observes the expected row count"
    );

    // A Tier-1 equality assertion over the SQL locator holds against the snapshot.
    let assertion = equals_assertion(total, BoundValue::Number(EXPECTED_TOTAL));
    assert_eq!(assertion.evaluate(&observation), AssertionOutcome::Holds);

    adapter.teardown(sut).expect("teardown");
}

#[test]
fn a_sql_locator_reports_drift_when_its_query_no_longer_binds() {
    let adapter = DbAdapter::in_memory();
    let repo = tempfile::TempDir::new().unwrap();
    let setup = Setup::Command(FIXTURE_SCHEMA.to_string());
    let mut sut = adapter
        .provision(Some(&setup), repo.path())
        .expect("provision");
    adapter
        .drive(
            &mut sut,
            "INSERT INTO orders (id, total, label) VALUES (1, 40, 'SAVE10');",
        )
        .expect("drive");
    let observation = observation_of(adapter.observe(&sut).expect("observe"));

    // A query against a table that does not exist no longer binds: structural
    // drift, surfaced as its own outcome rather than a silent mis-read.
    let assertion = equals_assertion(
        Locator::Sql {
            query: "SELECT total FROM missing_table".to_string(),
        },
        BoundValue::Number(EXPECTED_TOTAL),
    );
    assert!(
        matches!(
            assertion.evaluate(&observation),
            AssertionOutcome::Drifted { .. }
        ),
        "a non-binding SQL locator reports drift"
    );

    adapter.teardown(sut).expect("teardown");
}

#[test]
fn each_provision_is_a_fresh_database() {
    let adapter = DbAdapter::in_memory();
    let repo = tempfile::TempDir::new().unwrap();
    let setup = Setup::Command(FIXTURE_SCHEMA.to_string());

    // First instance gets a row.
    let mut first = adapter
        .provision(Some(&setup), repo.path())
        .expect("provision");
    adapter
        .drive(
            &mut first,
            "INSERT INTO orders (id, total, label) VALUES (1, 40, 'SAVE10');",
        )
        .expect("drive");
    adapter.teardown(first).expect("teardown");

    // A second provision is pristine: the table exists from the fixture, but the
    // first instance's row does not bleed into it.
    let second = adapter
        .provision(Some(&setup), repo.path())
        .expect("provision");
    let observation = observation_of(adapter.observe(&second).expect("observe"));
    let count = Locator::Sql {
        query: "SELECT COUNT(*) FROM orders".to_string(),
    };
    assert_eq!(
        count.resolve(&observation.checkpoints[0].state),
        Some(BoundValue::Number(0.0)),
        "a fresh provision sees none of a prior instance's rows"
    );
    adapter.teardown(second).expect("teardown");
}
