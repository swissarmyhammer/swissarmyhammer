//! The `db` surface adapter — the deterministic, no-agent database path.
//!
//! Per `ideas/expect.md` §"Surface adapters" (the db row) and §"Provisioning and
//! Isolation": the adapter **provisions** a fresh SQLite database and loads the
//! spec's `setup:` fixture, **drives** it by running statements through the
//! in-process Rust `rusqlite` client (no external server — `bundled` ships SQLite
//! itself), **observes** a SQL snapshot of the database state, and **tears it
//! down** by dropping the connection and its temp-file scratch dir.
//!
//! The locator dialect for the observed state — a SQL query projecting one scalar
//! ("the locator *is* SQL — very stable") — lives in the
//! [assertion compiler](crate::assertion); this module only produces the
//! [`DbState`] snapshot those locators query (by reloading it into an ephemeral
//! in-memory database).

use std::fmt;
use std::path::Path;

use rusqlite::types::Value;
use rusqlite::Connection;
use tempfile::TempDir;

use crate::error::ExpectError;
use crate::spec::Setup;
use crate::surface::{setup_commands, SurfaceAdapter};
use crate::types::{DbState, SurfaceState};

/// The filename of the temp-file database inside a [`DbSut`]'s scratch dir.
const DB_FILENAME: &str = "expect.sqlite";

/// The `db` surface adapter: provisions a fresh SQLite database, loads the spec's
/// `setup:` fixture, runs statements, and snapshots the result.
///
/// Construct a real, owned-on-disk database with [`DbAdapter::new`] (a temp-file
/// database created at provision and deleted at teardown), or a fully ephemeral
/// one with [`DbAdapter::in_memory`]. The adapter is deterministic and mechanical
/// — a db step is always concrete SQL — so it resolves every step itself and never
/// reaches the agent fallback (the trait's default
/// [`resolves_mechanically`](SurfaceAdapter::resolves_mechanically) of `true`).
#[derive(Debug, Clone, Default)]
pub struct DbAdapter {
    in_memory: bool,
}

impl DbAdapter {
    /// A db adapter backed by a fresh temp-file database, created at provision and
    /// deleted at teardown — a real database `expect` owns end to end.
    pub fn new() -> Self {
        Self { in_memory: false }
    }

    /// A db adapter backed by an in-memory database (no file on disk) — fully
    /// ephemeral, handy for fast tests.
    pub fn in_memory() -> Self {
        Self { in_memory: true }
    }
}

/// The provisioned db system under test: an open connection plus the scratch dir
/// backing its file (`None` for an in-memory database).
pub struct DbSut {
    /// The open database connection `drive` runs statements against and `observe`
    /// snapshots.
    conn: Connection,
    /// The scratch dir holding the database file; dropping it deletes the file
    /// (the teardown for a temp-file database). `None` for in-memory.
    scratch: Option<TempDir>,
}

impl fmt::Debug for DbSut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DbSut")
            .field("on_disk", &self.scratch.is_some())
            .finish()
    }
}

impl SurfaceAdapter for DbAdapter {
    type ProvisionedSut = DbSut;

    fn provision(&self, setup: Option<&Setup>, _repo_root: &Path) -> Result<DbSut, ExpectError> {
        let (conn, scratch) = if self.in_memory {
            (Connection::open_in_memory().map_err(map_db_err)?, None)
        } else {
            let scratch = TempDir::new()?;
            let conn = Connection::open(scratch.path().join(DB_FILENAME)).map_err(map_db_err)?;
            (conn, Some(scratch))
        };

        // Load the `setup:` fixture: each command is a SQL batch (schema + seed
        // data) run in order to arrange the database before it is driven.
        if let Some(setup) = setup {
            for statement in setup_commands(setup) {
                conn.execute_batch(statement).map_err(map_db_err)?;
            }
        }

        Ok(DbSut { conn, scratch })
    }

    fn drive(&self, sut: &mut DbSut, when_step: &str) -> Result<(), ExpectError> {
        let sql = when_step.trim();
        if sql.is_empty() {
            // An empty step drives nothing (mirrors the cli/http empty step).
            return Ok(());
        }
        sut.conn.execute_batch(sql).map_err(map_db_err)?;
        Ok(())
    }

    fn observe(&self, sut: &DbSut) -> Result<SurfaceState, ExpectError> {
        let snapshot = dump_database(&sut.conn)?;
        Ok(SurfaceState::Db(DbState { snapshot }))
    }

    fn teardown(&self, sut: DbSut) -> Result<(), ExpectError> {
        // Dropping the connection closes it; dropping the scratch `TempDir`
        // deletes the database file and its directory — "teardown drops it".
        drop(sut);
        Ok(())
    }
}

/// Serialize a live database to a SQL script (schema + data) — the snapshot the
/// SQL-projection locator reloads and queries.
///
/// Emits every user object's `CREATE` statement (in creation order) followed by an
/// `INSERT` per row of each table, the classic `.dump` shape minus the surrounding
/// pragmas/transaction. Built-in `sqlite_*` objects are skipped. The result is a
/// self-contained, human-readable capture that round-trips through an in-memory
/// database without the live one.
///
/// # Errors
///
/// Returns [`ExpectError::Surface`] when the schema or any table cannot be read.
fn dump_database(conn: &Connection) -> Result<String, ExpectError> {
    let entries = schema_entries(conn)?;

    let mut out = String::new();
    for entry in &entries {
        out.push_str(entry.create_sql.trim());
        out.push_str(";\n");
    }
    for entry in &entries {
        if entry.kind == "table" {
            append_table_rows(conn, &entry.name, &mut out)?;
        }
    }
    Ok(out)
}

/// One user-defined schema object: its kind (`table`, `index`, …), name, and the
/// `CREATE` statement that defines it.
struct SchemaEntry {
    kind: String,
    name: String,
    create_sql: String,
}

/// Read every user-defined schema object's `CREATE` statement, in creation order.
fn schema_entries(conn: &Connection) -> Result<Vec<SchemaEntry>, ExpectError> {
    let mut stmt = conn
        .prepare(
            "SELECT type, name, sql FROM sqlite_master \
             WHERE sql IS NOT NULL AND name NOT LIKE 'sqlite_%' ORDER BY rowid",
        )
        .map_err(map_db_err)?;
    let rows = stmt
        .query_map([], |row| {
            Ok(SchemaEntry {
                kind: row.get(0)?,
                name: row.get(1)?,
                create_sql: row.get(2)?,
            })
        })
        .map_err(map_db_err)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(map_db_err)
}

/// Append one `INSERT` statement per row of `table` to `out`.
///
/// Rows are ordered by every column (by ordinal) so the emitted `INSERT` order is
/// deterministic regardless of physical scan order — a snapshot is stable across
/// runs (rowid and `WITHOUT ROWID` tables alike), which keeps the committed golden
/// free of incidental row-order churn.
fn append_table_rows(conn: &Connection, table: &str, out: &mut String) -> Result<(), ExpectError> {
    let quoted = quote_identifier(table);
    let column_count = conn
        .prepare(&format!("SELECT * FROM {quoted}"))
        .map_err(map_db_err)?
        .column_count();
    let order_by = (1..=column_count)
        .map(|ordinal| ordinal.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let query = if order_by.is_empty() {
        format!("SELECT * FROM {quoted}")
    } else {
        format!("SELECT * FROM {quoted} ORDER BY {order_by}")
    };
    let mut stmt = conn.prepare(&query).map_err(map_db_err)?;
    let mut rows = stmt.query([]).map_err(map_db_err)?;
    while let Some(row) = rows.next().map_err(map_db_err)? {
        let mut values = Vec::with_capacity(column_count);
        for index in 0..column_count {
            let value: Value = row.get(index).map_err(map_db_err)?;
            values.push(render_sql_value(&value));
        }
        out.push_str(&format!(
            "INSERT INTO {quoted} VALUES ({});\n",
            values.join(", ")
        ));
    }
    Ok(())
}

/// Quote a SQL identifier in double quotes, escaping any embedded double quote.
fn quote_identifier(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// Render one [`Value`] as a SQL literal for an `INSERT` statement.
fn render_sql_value(value: &Value) -> String {
    match value {
        Value::Null => "NULL".to_string(),
        Value::Integer(integer) => integer.to_string(),
        Value::Real(real) => render_real(*real),
        Value::Text(text) => format!("'{}'", text.replace('\'', "''")),
        Value::Blob(bytes) => {
            let mut hex = String::with_capacity(bytes.len() * 2);
            for byte in bytes {
                hex.push_str(&format!("{byte:02x}"));
            }
            format!("X'{hex}'")
        }
    }
}

/// Render a `REAL` value as a SQL literal, round-tripping non-finite values.
///
/// A finite value uses the shortest round-tripping form (`{:?}`). SQLite stores
/// `±infinity` in a REAL column (it coerces only NaN to NULL, so NaN never reaches
/// here), and `inf`/`-inf` are not valid SQLite literals — `9e999`/`-9e999`
/// overflow to `±Inf` on reload, keeping the snapshot self-reloadable.
fn render_real(real: f64) -> String {
    if real.is_finite() {
        format!("{real:?}")
    } else if real > 0.0 {
        "9e999".to_string()
    } else {
        "-9e999".to_string()
    }
}

/// Map a `rusqlite` failure to an [`ExpectError::Surface`].
fn map_db_err(err: rusqlite::Error) -> ExpectError {
    ExpectError::Surface(format!("db error: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fixture schema reused across the unit tests.
    const SCHEMA: &str = "CREATE TABLE t (id INTEGER PRIMARY KEY, n INTEGER, s TEXT);";

    fn provisioned(adapter: &DbAdapter) -> DbSut {
        let repo = TempDir::new().unwrap();
        adapter
            .provision(Some(&Setup::Command(SCHEMA.to_string())), repo.path())
            .expect("provision")
    }

    #[test]
    fn dump_round_trips_schema_and_rows_through_an_in_memory_database() {
        let adapter = DbAdapter::in_memory();
        let mut sut = provisioned(&adapter);
        adapter
            .drive(&mut sut, "INSERT INTO t (id, n, s) VALUES (1, 7, 'it''s');")
            .expect("drive");

        let SurfaceState::Db(db) = adapter.observe(&sut).expect("observe") else {
            panic!("expected db state");
        };

        // The snapshot reloads into a fresh database and answers the same query —
        // the escaped quote in the text survived the dump/reload round-trip.
        let reloaded = Connection::open_in_memory().unwrap();
        reloaded
            .execute_batch(&db.snapshot)
            .expect("reload snapshot");
        let (n, s): (i64, String) = reloaded
            .query_row("SELECT n, s FROM t WHERE id = 1", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .expect("query reloaded");
        assert_eq!(n, 7);
        assert_eq!(s, "it's");
    }

    #[test]
    fn render_sql_value_renders_each_storage_class() {
        assert_eq!(render_sql_value(&Value::Null), "NULL");
        assert_eq!(render_sql_value(&Value::Integer(42)), "42");
        assert_eq!(render_sql_value(&Value::Real(1.5)), "1.5");
        assert_eq!(render_sql_value(&Value::Text("a'b".to_string())), "'a''b'");
        assert_eq!(render_sql_value(&Value::Blob(vec![0x00, 0xff])), "X'00ff'");
    }

    #[test]
    fn non_finite_reals_round_trip_through_a_reloadable_snapshot() {
        // SQLite stores ±infinity in a REAL column; the rendered literal must
        // reload (not the invalid `inf`) and read back as the same infinities.
        let adapter = DbAdapter::in_memory();
        let repo = TempDir::new().unwrap();
        let mut sut = adapter
            .provision(
                Some(&Setup::Command("CREATE TABLE r (v REAL);".to_string())),
                repo.path(),
            )
            .expect("provision");
        adapter
            .drive(&mut sut, "INSERT INTO r (v) VALUES (9e999), (-9e999);")
            .expect("drive infinities");

        let SurfaceState::Db(db) = adapter.observe(&sut).expect("observe") else {
            panic!("expected db state");
        };
        let reloaded = Connection::open_in_memory().unwrap();
        reloaded
            .execute_batch(&db.snapshot)
            .expect("snapshot reloads");
        let values: Vec<f64> = reloaded
            .prepare("SELECT v FROM r ORDER BY v")
            .unwrap()
            .query_map([], |row| row.get::<_, f64>(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(values, vec![f64::NEG_INFINITY, f64::INFINITY]);
    }

    #[test]
    fn an_empty_step_drives_nothing() {
        let adapter = DbAdapter::in_memory();
        let mut sut = provisioned(&adapter);
        adapter
            .drive(&mut sut, "   ")
            .expect("empty step is a no-op");
        let SurfaceState::Db(db) = adapter.observe(&sut).expect("observe") else {
            panic!("expected db state");
        };
        assert!(db.snapshot.contains("CREATE TABLE t"));
        assert!(!db.snapshot.contains("INSERT INTO"));
    }

    #[test]
    fn a_malformed_statement_is_a_surface_error() {
        let adapter = DbAdapter::in_memory();
        let mut sut = provisioned(&adapter);
        let err = adapter
            .drive(&mut sut, "INSERT INTO nope VALUES (1)")
            .expect_err("unknown table must error");
        assert!(matches!(err, ExpectError::Surface(_)), "got {err:?}");
    }
}
