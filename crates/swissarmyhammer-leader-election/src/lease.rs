//! Lease-based leadership with heartbeat and takeover.
//!
//! The plain `flock` held by [`crate::election`] is held until the owning
//! process dies — there is no lease, heartbeat, or takeover, so a
//! stale-but-alive leader (e.g. a wedged `sah serve`) keeps the flock forever
//! and a live session can never take over its indexing duties.
//!
//! This module layers a *lease* on top of the same lock file. The lease is a
//! small JSON record written into the lock file's bytes; it carries the leader's
//! PID, a random `nonce` identifying the current leadership term, and the
//! millisecond timestamp of the last heartbeat. A leader periodically refreshes
//! the heartbeat under its `nonce` ([`heartbeat`]); a candidate that observes a
//! stale heartbeat can *claim* the lease by writing a fresh `nonce`
//! ([`try_claim_lease`]) — taking over leadership by lease even while the old
//! leader still holds the flock.
//!
//! Time is injected through the [`Clock`] trait so tests are deterministic and
//! never sleep a real TTL ([`TestClock`]); production uses [`SystemClock`].

use std::fs::{self, OpenOptions};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// How often a leader refreshes its lease heartbeat.
pub const HEARTBEAT_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);

/// How long a lease is considered valid since its last heartbeat. A candidate
/// may take over once `now - last_heartbeat_ms > LEASE_TTL`.
pub const LEASE_TTL: std::time::Duration = std::time::Duration::from_secs(15);

/// How long an `O_EXCL` takeover-claim marker is honored before it is treated
/// as abandoned (the claimer crashed mid-takeover) and may be removed by the
/// next candidate. Short, because a real takeover completes in well under a
/// second; this only bounds recovery from a crash *during* the claim.
pub const CLAIM_TTL: std::time::Duration = std::time::Duration::from_secs(2);

/// A source of wall-clock time in milliseconds since the Unix epoch.
pub trait Clock {
    /// Milliseconds since the Unix epoch.
    fn now_millis(&self) -> u64;
}

/// Production [`Clock`] backed by the system wall clock.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_millis(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
}

/// Deterministic [`Clock`] for tests.
#[derive(Debug, Clone)]
pub struct TestClock {
    now_ms: std::cell::Cell<u64>,
}

impl TestClock {
    /// Create a test clock fixed at `now_ms`.
    pub fn new(now_ms: u64) -> Self {
        Self {
            now_ms: std::cell::Cell::new(now_ms),
        }
    }

    /// Advance the clock by `delta`.
    pub fn advance(&self, delta: std::time::Duration) {
        self.now_ms
            .set(self.now_ms.get() + delta.as_millis() as u64);
    }
}

impl Clock for TestClock {
    fn now_millis(&self) -> u64 {
        self.now_ms.get()
    }
}

/// A leadership lease persisted as JSON inside the election lock file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lease {
    /// PID of the process that currently holds the lease.
    pub pid: u32,
    /// Random identifier of the current leadership term.
    pub nonce: u64,
    /// Milliseconds since the Unix epoch of the most recent heartbeat.
    pub heartbeat_ms: u64,
}

impl Lease {
    /// Whether this lease is stale at `now_ms` given `ttl`.
    ///
    /// Stale means the last heartbeat is strictly older than the TTL window, so
    /// a candidate may take over. Using a saturating subtraction means a clock
    /// that appears to move backwards (now < heartbeat) yields `0` elapsed and
    /// is treated as fresh, never as stale.
    pub fn is_stale(&self, now_ms: u64, ttl: std::time::Duration) -> bool {
        let elapsed = now_ms.saturating_sub(self.heartbeat_ms);
        elapsed > ttl.as_millis() as u64
    }
}

/// The result of attempting to claim a lease via [`try_claim_lease`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaimOutcome {
    /// The caller claimed (or renewed) the lease and now owns it under `nonce`.
    Won {
        /// The nonce the caller now holds the lease under.
        nonce: u64,
    },
    /// A live, fresh lease is held by someone else; the caller did not take over.
    Lost,
}

/// Generate a fresh, pseudo-random nonce for a new leadership term.
///
/// Mixes the current nanosecond timestamp, the process id, and a
/// process-local monotonic counter so two nonces drawn in quick succession
/// (even within one process) differ. Collision only needs to be improbable,
/// not impossible: the flock and the staleness check are the real safety net,
/// the nonce only distinguishes leadership terms.
pub fn new_nonce() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    nanos
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add((std::process::id() as u64) << 32)
        .wrapping_add(seq.wrapping_mul(0xD1B5_4A32_D192_ED03))
}

/// Read and parse the lease stored at `path`.
///
/// Returns `None` when the file is missing, empty, or does not contain a
/// parseable [`Lease`] JSON record (e.g. a legacy bare-PID lock file from a
/// process that predates leases).
pub fn read_lease(path: &Path) -> Option<Lease> {
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Write `lease` into the lock file at `path`, IN PLACE.
///
/// The lease must be written into the *same inode* that the election holds its
/// `flock` on. A previous design renamed a temp file over `path`, but `rename`
/// swaps the inode at the path, which orphans the leader's flock (flock is
/// per-inode): the old handle keeps a lock on the now-unlinked inode while a new
/// unlocked inode appears at the path, so a second contender's `try_lock` would
/// wrongly succeed. So this opens the existing path (creating it if absent),
/// writes the new record from offset 0, then truncates to that record's length —
/// leaving the inode (and thus any flock held on it) intact.
///
/// This in-place rewrite is not atomic against a concurrent reader, but the
/// only readers are [`read_lease`]/[`lease_held_by`], which treat a torn parse
/// as "no lease" and re-read on the next poll. The real leader's writes are
/// serialized by its flock; a candidate writes only when taking over a stale
/// lease, so writers do not meaningfully race.
pub fn write_lease_atomic(path: &Path, lease: &Lease) -> std::io::Result<()> {
    use std::io::{Seek, SeekFrom, Write};
    let json = serde_json::to_string(lease)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    // Write the full record FIRST, then trim to its length. A concurrent reader
    // therefore sees either the previous full JSON or the new full JSON — never a
    // zero-length file — so it cannot misread a live lease as "absent" mid-write.
    file.seek(SeekFrom::Start(0))?;
    file.write_all(json.as_bytes())?;
    file.set_len(json.len() as u64)?;
    file.flush()
}

/// Whether the lease at `path` is currently held under `nonce`.
///
/// True only when a lease exists and its stored nonce matches. A leader uses
/// this to confirm it still owns the current term.
pub fn lease_held_by(path: &Path, nonce: u64) -> bool {
    read_lease(path).is_some_and(|l| l.nonce == nonce)
}

/// Attempt to claim the lease at `path`.
///
/// - If no lease exists, or the existing lease is stale (older than `ttl` at
///   `clock.now_millis()`), the caller writes a fresh lease under a new nonce
///   and wins: [`ClaimOutcome::Won`].
/// - If a fresh lease is held by someone else, the caller does not take over:
///   [`ClaimOutcome::Lost`].
///
/// A failed write (e.g. read-only filesystem) is treated as `Lost` — the caller
/// could not establish ownership, so it must not believe it leads.
pub fn try_claim_lease(path: &Path, clock: &dyn Clock, ttl: std::time::Duration) -> ClaimOutcome {
    let now = clock.now_millis();

    // Fast path: a fresh lease means a live, working leader — never preempt it,
    // and never even contend for the claim marker.
    if let Some(existing) = read_lease(path) {
        if !existing.is_stale(now, ttl) {
            return ClaimOutcome::Lost;
        }
    }

    // The lease is stale (or absent). Gate the takeover on an O_EXCL marker so
    // that, among any number of concurrent candidates, exactly ONE proceeds to
    // rewrite the lease. Without this gate two candidates could both pass the
    // staleness check above and both write, producing a two-writer window.
    if !acquire_claim_marker(path) {
        return ClaimOutcome::Lost;
    }

    // We hold the claim marker. Re-check the lease under the gate: a healthy
    // leader may have heartbeated between our first read and acquiring the
    // marker, in which case we must NOT preempt it.
    if let Some(existing) = read_lease(path) {
        if !existing.is_stale(clock.now_millis(), ttl) {
            let _ = fs::remove_file(claim_marker_path(path));
            return ClaimOutcome::Lost;
        }
    }

    // Win: install our lease, then drop the claim gate.
    let nonce = new_nonce();
    let lease = Lease {
        pid: std::process::id(),
        nonce,
        heartbeat_ms: clock.now_millis(),
    };
    let outcome = match write_lease_atomic(path, &lease) {
        Ok(()) => ClaimOutcome::Won { nonce },
        Err(e) => {
            tracing::debug!(
                path = %path.display(),
                error = %e,
                "could not write lease while claiming (treating as Lost)",
            );
            ClaimOutcome::Lost
        }
    };
    let _ = fs::remove_file(claim_marker_path(path));
    outcome
}

/// Path of the O_EXCL takeover-claim marker for a given lock path. A sibling of
/// the lock file (`<lock>.claim`), so creating/removing it never touches the
/// flock'd inode of the lock file itself.
fn claim_marker_path(lock_path: &Path) -> std::path::PathBuf {
    lock_path.with_extension("claim")
}

/// Try to acquire the O_EXCL claim marker, reclaiming an abandoned one.
///
/// Returns `true` when this call created the marker (won the gate), `false` when
/// a *live* marker is already held by another candidate. A marker whose mtime is
/// older than [`CLAIM_TTL`] is treated as abandoned (the claimer crashed
/// mid-takeover): it is removed and creation retried exactly once.
///
/// Staleness is keyed on the file's MTIME, not its contents. `create_new` is the
/// atomic single-winner gate; the file is briefly empty between creation and the
/// (informational) pid write, so a *content*-based staleness check would let a
/// concurrent candidate reclaim a live, just-created marker (a torn read) and
/// produce two winners. MTIME is set atomically by the OS at create, so a
/// just-created marker is always fresh.
fn acquire_claim_marker(lock_path: &Path) -> bool {
    let path = claim_marker_path(lock_path);
    if create_claim_marker(&path) {
        return true;
    }
    if claim_marker_is_abandoned(&path) {
        let _ = fs::remove_file(&path);
        return create_claim_marker(&path);
    }
    false
}

/// Create the claim marker with `O_EXCL`. Writes the pid for diagnostics only
/// (staleness is mtime-based). Returns `true` on creation, `false` if it
/// already existed.
fn create_claim_marker(path: &Path) -> bool {
    use std::io::Write;
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut f) => {
            let _ = write!(f, "{}", std::process::id());
            true
        }
        Err(_) => false,
    }
}

/// Whether the claim marker at `path` is abandoned — its mtime is older than
/// [`CLAIM_TTL`] against the real wall clock. A missing marker, or one whose
/// mtime cannot be read, is treated as abandoned (reclaimable). Uses real time
/// (not the injected [`Clock`]) because the OS stamps mtime in real time; this
/// path only handles crash recovery and never gates the normal single-winner
/// race (a live marker's mtime is always recent).
fn claim_marker_is_abandoned(path: &Path) -> bool {
    let Ok(modified) = fs::metadata(path).and_then(|m| m.modified()) else {
        return true;
    };
    match modified.elapsed() {
        Ok(age) => age > CLAIM_TTL,
        // A negative elapsed (mtime in the future, clock skew) means very
        // recent — treat as fresh, not abandoned.
        Err(_) => false,
    }
}

/// Refresh the heartbeat of the lease at `path` under `nonce`.
///
/// Returns `true` if we still own the lease (the stored nonce matches and the
/// heartbeat was refreshed) and `false` if we were preempted (the stored nonce
/// differs, the file is missing, or the refresh write failed). A `false` return
/// is the signal for a leader to step down.
pub fn heartbeat(path: &Path, nonce: u64, clock: &dyn Clock) -> bool {
    let Some(current) = read_lease(path) else {
        return false;
    };
    if current.nonce != nonce {
        // Someone else took over this lease — we lost the term.
        return false;
    }
    let refreshed = Lease {
        pid: std::process::id(),
        nonce,
        heartbeat_ms: clock.now_millis(),
    };
    match write_lease_atomic(path, &refreshed) {
        Ok(()) => true,
        Err(e) => {
            tracing::debug!(
                path = %path.display(),
                error = %e,
                "could not refresh lease heartbeat (treating as lost)",
            );
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn lease_path(dir: &TempDir) -> std::path::PathBuf {
        dir.path().join("test.lock")
    }

    #[test]
    fn test_system_clock_is_nonzero() {
        let clock = SystemClock;
        let t = clock.now_millis();
        assert!(t > 0, "system clock should return a positive epoch-ms");
    }

    #[test]
    fn test_test_clock_fixed_then_advances() {
        let clock = TestClock::new(1_000);
        assert_eq!(clock.now_millis(), 1_000);
        clock.advance(std::time::Duration::from_millis(500));
        assert_eq!(clock.now_millis(), 1_500);
    }

    #[test]
    fn test_new_nonce_varies() {
        let a = new_nonce();
        let b = new_nonce();
        assert_ne!(a, b, "consecutive nonces should differ");
    }

    #[test]
    fn test_lease_is_stale_within_ttl_is_false() {
        let lease = Lease {
            pid: 1,
            nonce: 42,
            heartbeat_ms: 1_000,
        };
        assert!(!lease.is_stale(6_000, LEASE_TTL));
    }

    #[test]
    fn test_lease_is_stale_past_ttl_is_true() {
        let lease = Lease {
            pid: 1,
            nonce: 42,
            heartbeat_ms: 1_000,
        };
        assert!(lease.is_stale(21_000, LEASE_TTL));
    }

    #[test]
    fn test_lease_is_stale_exactly_at_ttl_is_not_stale() {
        let lease = Lease {
            pid: 1,
            nonce: 42,
            heartbeat_ms: 1_000,
        };
        assert!(!lease.is_stale(1_000 + LEASE_TTL.as_millis() as u64, LEASE_TTL));
    }

    #[test]
    fn test_write_then_read_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = lease_path(&dir);
        let lease = Lease {
            pid: 4321,
            nonce: 99,
            heartbeat_ms: 7_777,
        };
        write_lease_atomic(&path, &lease).unwrap();
        let read = read_lease(&path).expect("lease should read back");
        assert_eq!(read, lease);
    }

    #[test]
    fn test_read_missing_file_is_none() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nope.lock");
        assert!(read_lease(&path).is_none());
    }

    #[test]
    fn test_read_legacy_bare_pid_is_none() {
        let dir = TempDir::new().unwrap();
        let path = lease_path(&dir);
        fs::write(&path, "12345\n").unwrap();
        assert!(read_lease(&path).is_none());
    }

    #[test]
    fn test_lease_held_by_matches_nonce() {
        let dir = TempDir::new().unwrap();
        let path = lease_path(&dir);
        let lease = Lease {
            pid: 1,
            nonce: 7,
            heartbeat_ms: 100,
        };
        write_lease_atomic(&path, &lease).unwrap();
        assert!(lease_held_by(&path, 7));
        assert!(!lease_held_by(&path, 8));
    }

    #[test]
    fn test_lease_held_by_missing_file_is_false() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nope.lock");
        assert!(!lease_held_by(&path, 1));
    }

    #[test]
    fn test_claim_when_no_lease_wins() {
        let dir = TempDir::new().unwrap();
        let path = lease_path(&dir);
        let clock = TestClock::new(10_000);
        match try_claim_lease(&path, &clock, LEASE_TTL) {
            ClaimOutcome::Won { nonce } => {
                assert!(lease_held_by(&path, nonce), "winner owns the lease");
                let lease = read_lease(&path).unwrap();
                assert_eq!(lease.heartbeat_ms, 10_000, "lease stamped at clock now");
                assert_eq!(lease.pid, std::process::id());
            }
            ClaimOutcome::Lost => panic!("claiming an absent lease must win"),
        }
    }

    #[test]
    fn test_claim_when_fresh_lease_held_loses() {
        let dir = TempDir::new().unwrap();
        let path = lease_path(&dir);
        let fresh = Lease {
            pid: 9999,
            nonce: 555,
            heartbeat_ms: 10_000,
        };
        write_lease_atomic(&path, &fresh).unwrap();
        let clock = TestClock::new(12_000);
        assert_eq!(
            try_claim_lease(&path, &clock, LEASE_TTL),
            ClaimOutcome::Lost
        );
        assert!(lease_held_by(&path, 555));
    }

    #[test]
    fn test_claim_when_stale_lease_held_wins_and_takes_over() {
        let dir = TempDir::new().unwrap();
        let path = lease_path(&dir);
        let stale = Lease {
            pid: 9999,
            nonce: 555,
            heartbeat_ms: 10_000,
        };
        write_lease_atomic(&path, &stale).unwrap();
        let clock = TestClock::new(30_000);
        match try_claim_lease(&path, &clock, LEASE_TTL) {
            ClaimOutcome::Won { nonce } => {
                assert_ne!(nonce, 555, "takeover writes a fresh nonce");
                assert!(lease_held_by(&path, nonce));
                assert!(!lease_held_by(&path, 555));
            }
            ClaimOutcome::Lost => panic!("a stale lease must be claimable"),
        }
    }

    #[test]
    fn test_heartbeat_under_owned_nonce_returns_true_and_refreshes() {
        let dir = TempDir::new().unwrap();
        let path = lease_path(&dir);
        let clock = TestClock::new(1_000);
        let nonce = match try_claim_lease(&path, &clock, LEASE_TTL) {
            ClaimOutcome::Won { nonce } => nonce,
            ClaimOutcome::Lost => panic!("first claim must win"),
        };
        clock.advance(std::time::Duration::from_secs(3));
        assert!(heartbeat(&path, nonce, &clock));
        let lease = read_lease(&path).unwrap();
        assert_eq!(lease.heartbeat_ms, 4_000, "heartbeat stamps the new time");
        assert_eq!(lease.nonce, nonce, "heartbeat keeps our nonce");
    }

    #[test]
    fn test_heartbeat_after_preemption_returns_false() {
        let dir = TempDir::new().unwrap();
        let path = lease_path(&dir);
        let clock = TestClock::new(1_000);
        let our_nonce = match try_claim_lease(&path, &clock, LEASE_TTL) {
            ClaimOutcome::Won { nonce } => nonce,
            ClaimOutcome::Lost => panic!("first claim must win"),
        };
        let foreign = Lease {
            pid: 8888,
            nonce: our_nonce.wrapping_add(1),
            heartbeat_ms: 2_000,
        };
        write_lease_atomic(&path, &foreign).unwrap();
        assert!(!heartbeat(&path, our_nonce, &clock));
    }

    #[test]
    fn test_heartbeat_missing_file_returns_false() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nope.lock");
        let clock = TestClock::new(1_000);
        assert!(!heartbeat(&path, 1, &clock));
    }

    // Exactly one winner under concurrent claim attempts on a stale lease — the
    // core single-writer invariant the O_EXCL gate enforces.
    #[test]
    fn test_concurrent_claims_yield_exactly_one_winner() {
        use std::sync::Arc;
        let dir = TempDir::new().unwrap();
        let path = Arc::new(lease_path(&dir));
        // Stale lease already on disk so every thread is a genuine candidate.
        write_lease_atomic(
            &path,
            &Lease {
                pid: 1,
                nonce: 1,
                heartbeat_ms: 0,
            },
        )
        .unwrap();

        // A fixed clock far past the heartbeat → the lease is stale for all.
        // SystemClock can't be shared as a fixed instant across threads, so use
        // a struct clock wrapping an AtomicU64 read-only here.
        #[derive(Clone)]
        struct FixedClock(std::sync::Arc<std::sync::atomic::AtomicU64>);
        impl Clock for FixedClock {
            fn now_millis(&self) -> u64 {
                self.0.load(std::sync::atomic::Ordering::SeqCst)
            }
        }
        let clock = FixedClock(std::sync::Arc::new(std::sync::atomic::AtomicU64::new(
            10_000_000,
        )));

        let handles: Vec<_> = (0..8)
            .map(|_| {
                let p = Arc::clone(&path);
                let c = clock.clone();
                std::thread::spawn(move || {
                    matches!(try_claim_lease(&p, &c, LEASE_TTL), ClaimOutcome::Won { .. })
                })
            })
            .collect();
        let wins = handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .filter(|&won| won)
            .count();
        assert_eq!(
            wins, 1,
            "exactly one candidate may win a concurrent takeover"
        );
    }

    // A crashed claimer's stale marker must not wedge takeover — the gate self-heals.
    #[test]
    fn test_stale_claim_marker_is_reclaimed() {
        let dir = TempDir::new().unwrap();
        let path = lease_path(&dir);
        let clock = TestClock::new(1_000_000);
        // Stale lease present (heartbeat_ms 0 → always stale vs the clock).
        write_lease_atomic(
            &path,
            &Lease {
                pid: 1,
                nonce: 1,
                heartbeat_ms: 0,
            },
        )
        .unwrap();
        // An abandoned claim marker whose MTIME is well older than CLAIM_TTL.
        let marker = path.with_extension("claim");
        fs::write(&marker, "999").unwrap();
        let old = filetime::FileTime::from_unix_time(0, 0); // epoch — far in the past
        filetime::set_file_mtime(&marker, old).unwrap();
        // The marker is abandoned by mtime → reclaimed → we win the takeover.
        assert!(matches!(
            try_claim_lease(&path, &clock, LEASE_TTL),
            ClaimOutcome::Won { .. }
        ));
        assert!(!marker.exists(), "winner must clean up the claim marker");
    }
}
