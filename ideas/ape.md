# heb: Hook Event Bus

## Motivation

AVP hooks are the integration point where Claude Code tells us what's happening.
Right now that information dies inside the hook process. We want other agents,
programs, and UIs to observe hook events in real time — without requiring a
daemon to be running.

Hence's HEB solves a similar problem but assumes a long-lived Mirdan daemon
owns the broadcast channel and SQLite connection. That's wrong for AVP's model
where hooks are short-lived processes and there may be no central coordinator.

## Architecture: Leader-Owned ZMQ XPUB/XSUB Proxy

The leader election process — which already exists in `swissarmyhammer-leader-election`
— takes on one more responsibility: running a ZMQ XPUB/XSUB forwarding proxy.

```
                    ┌─────────────────────────────────┐
                    │         Leader Process           │
                    │                                  │
  Publishers        │   XSUB (frontend)   XPUB (backend)   Subscribers
  ─────────────     │   bind(front_addr)  bind(back_addr)   ─────────────
                    │         │                │
  Hook A ─(PUB)────connect───┤                ├───connect──(SUB)─ Agent X
  Hook B ─(PUB)────connect───┤    forward     ├───connect──(SUB)─ UI
  Agent Y ─(PUB)───connect───┘    loop        └───connect──(SUB)─ Logger
                    │                                  │
                    └─────────────────────────────────┘
```

**Every process also writes to SQLite.** The ZMQ proxy is the live delivery
path. SQLite is the durable log. Both always happen.

### How It Works

1. **Leader wins election** (existing flock mechanism)
2. Leader binds XSUB on `front_addr`, XPUB on `back_addr`
3. Leader writes a **discovery file** with both addresses
4. Leader spawns a proxy thread: `loop { recv(xsub) → send(xpub) }`
5. Publishers read discovery file, `connect(front_addr)` with a PUB socket
6. Subscribers read discovery file, `connect(back_addr)` with a SUB socket
7. **Leader dies** → `LeaderGuard::drop()` cleans up discovery file + sockets
8. **Follower promotes** via `try_promote()` → spins up new proxy, writes new discovery file
9. ZMQ auto-reconnect on publisher/subscriber sockets handles the brief gap

### Discovery File

Written by the leader to a well-known path provided by the HEB context:

```
$XDG_RUNTIME_DIR/heb/{context_hash}.addr
```

Contents (plain text, two lines):

```
ipc:///tmp/heb-{hash}-front.sock
ipc:///tmp/heb-{hash}-back.sock
```

Or on systems where `XDG_RUNTIME_DIR` isn't set, fall back to temp dir
using the same hash scheme leader election already uses.

**No discovery file means no leader.** Any process that opens a `HebContext`
and finds no discovery file should contest the election. This means every
participant — hooks, agents, UIs — is a potential leader. Whoever wins
starts the proxy and writes the file. Losers read the file and connect.

If a short-lived hook is the only process running, it becomes the leader,
runs the proxy for its lifetime, and tears it down on exit. The next process
to come along re-contests and takes over.

The discovery file existing IS the liveness signal for the bus.

- **No file** → contest election → win: start proxy + write file / lose: read file, connect
- **File exists** → connect to advertised addresses
- **File exists but proxy dead** (stale from a crash) → flock is released,
  so election will succeed → new leader overwrites file

### Integration with Leader Election

Leader election becomes a **typed pub/sub bus** parameterized by message type.
The XPUB/XSUB proxy is intrinsic — every leader election instance runs a bus.

```rust
/// The trait that message types must implement to ride the bus.
pub trait BusMessage: Send + 'static {
    /// The topic/category for ZMQ prefix filtering.
    /// Subscribers use this to filter at the ZMQ level.
    fn topic(&self) -> &[u8];

    /// Serialize to wire format (ZMQ frames after the topic frame).
    fn to_frames(&self) -> Result<Vec<Vec<u8>>>;

    /// Deserialize from wire format.
    fn from_frames(topic: &[u8], frames: &[Vec<u8>]) -> Result<Self> where Self: Sized;
}

/// Leader election is generic over the message type.
pub struct LeaderElection<M: BusMessage> {
    config: ElectionConfig,
    workspace_root: PathBuf,
    _phantom: PhantomData<M>,
}

pub struct LeaderGuard<M: BusMessage> {
    lock_file: File,
    proxy_handle: JoinHandle<()>,
    zmq_ctx: zmq::Context,
    /// Leader can also publish — connects PUB to its own frontend
    publisher: Publisher<M>,
    /// Registered callbacks invoked by the proxy thread for each message
    callbacks: Arc<Mutex<Vec<Box<dyn Fn(&M) + Send>>>>,
}

pub struct FollowerGuard<M: BusMessage> {
    lock_path: PathBuf,
    /// Followers publish and subscribe through the leader's proxy
    publisher: Publisher<M>,
}

pub enum ElectionOutcome<M: BusMessage> {
    Leader(LeaderGuard<M>),
    Follower(FollowerGuard<M>),
}
```

Both leaders and followers get a `Publisher<M>` on construction — everyone
can publish from the moment `elect()` returns. The difference is the leader
also owns the proxy thread.

### Message Callbacks

Callbacks let participants react to messages flowing through the bus without
managing their own subscriber loop. Register before or after election:

```rust
let config = ElectionConfig::new()
    .with_prefix("heb")
    .on_message(|event: &HebEvent| {
        // called for every message that flows through the bus
        // runs in the subscriber thread — keep it fast
        println!("event: {} {}", event.header.category, event.header.event_type);
    });

let outcome = LeaderElection::<HebEvent>::with_config(workspace_root, config).elect()?;
```

Under the hood, callbacks are serviced by an internal SUB socket connected
to the proxy backend, running in its own thread. This is separate from the
proxy forwarding thread — the proxy just forwards, the callback thread
consumes.

```
Proxy thread:     XSUB ──forward──> XPUB     (dumb pipe, always running)
Callback thread:  SUB(connect back) ──recv──> invoke callbacks  (if any registered)
```

### Publish from Any Role

```rust
// Both LeaderGuard and FollowerGuard expose publish()
impl<M: BusMessage> LeaderGuard<M> {
    pub fn publish(&self, msg: &M) -> Result<()> { self.publisher.send(msg) }
}

impl<M: BusMessage> FollowerGuard<M> {
    pub fn publish(&self, msg: &M) -> Result<()> { self.publisher.send(msg) }
    pub fn try_promote(&self) -> Result<Option<LeaderGuard<M>>> { ... }
}
```

### What This Means for heb

heb becomes a thin layer that defines the message type and the context:

```rust
/// heb's message type — header + body envelope
pub struct HebEvent {
    pub header: EventHeader,
    pub body: Vec<u8>,
}

impl BusMessage for HebEvent {
    fn topic(&self) -> &[u8] { self.header.category.as_bytes() }

    fn to_frames(&self) -> Result<Vec<Vec<u8>>> {
        Ok(vec![
            serde_json::to_vec(&self.header)?,
            self.body.clone(),
        ])
    }

    fn from_frames(topic: &[u8], frames: &[Vec<u8>]) -> Result<Self> {
        Ok(HebEvent {
            header: serde_json::from_slice(&frames[0])?,
            body: frames[1].clone(),
        })
    }
}
```

And `HebContext::open()` just configures and runs the election:

```rust
impl HebContext {
    pub fn open(workspace_root: &Path) -> Result<Self> {
        let config = ElectionConfig::new()
            .with_prefix("heb")
            .on_message(|event: &HebEvent| {
                // persist every event that flows through the bus
                let _ = log_event(&db_path, &event.header, &event.body);
            });

        let outcome = LeaderElection::<HebEvent>::with_config(workspace_root, config).elect()?;
        // ...
    }
}
```

Note: the `on_message` callback on the **leader** sees every message — including
its own. This is how heb gets SQLite persistence for free: register a callback
that writes to the WAL. Every message on the bus gets logged, regardless of
who published it.

### The Proxy Thread

~30 lines. This is the entire broker:

```rust
fn run_proxy(front_addr: &str, back_addr: &str, stop: Arc<AtomicBool>) -> Result<()> {
    let ctx = zmq::Context::new();

    let frontend = ctx.socket(zmq::XSUB)?;
    frontend.bind(front_addr)?;

    let backend = ctx.socket(zmq::XPUB)?;
    backend.bind(back_addr)?;

    // zmq_proxy() blocks forever, forwarding messages between frontend and backend.
    // It also forwards subscription messages from XPUB back to XSUB.
    // When stop is signaled, we terminate the context which unblocks proxy.
    let ctx_clone = ctx.clone();
    std::thread::spawn(move || {
        while !stop.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(100));
        }
        ctx_clone.destroy().ok();
    });

    zmq::proxy(&frontend, &backend)?; // blocks until context destroyed
    Ok(())
}
```

## Event Envelope: Header / Body

Events use a ZMQ multipart message: topic frame, header frame, body frame.
Same structure persists to SQLite.

### ZMQ Wire Format (3 frames)

```
Frame 0: topic   — category string, e.g. b"hook" (used for SUB filtering)
Frame 1: header  — JSON-encoded EventHeader
Frame 2: body    — opaque bytes (usually JSON, interpretation depends on event_type)
```

### Header

```rust
pub struct EventHeader {
    /// Monotonic sequence (assigned on persist, 0 on wire before persist)
    pub seq: u64,
    /// When the event was created
    pub timestamp: DateTime<Utc>,
    /// Originating Claude Code session ID
    pub session_id: String,
    /// Working directory of the session that produced this event
    pub cwd: PathBuf,
    /// Coarse category for topic-based ZMQ filtering
    pub category: EventCategory,
    /// Fine-grained event type (e.g. "pre_tool_use", "post_tool_use")
    pub event_type: String,
    /// What produced this event (e.g. "avp-hook", "agent:xyz")
    pub source: String,
}
```

### Body

```rust
/// Opaque payload. Usually JSON. Interpretation depends on (category, event_type).
pub type EventBody = Vec<u8>;
```

### Topic Categories

```
"hook"     → hook lifecycle (pre_tool_use, post_tool_use, etc.)
"session"  → session start/end
"agent"    → agent spawned/completed
"card"     → kanban mutations
"system"   → health, errors
```

Subscribers filter: `socket.set_subscribe(b"hook")` or `socket.set_subscribe(b"")`
for everything.

## Durable Log: SQLite WAL, Open/Write/Close

Every process writes to SQLite on every publish. This is independent of
whether the ZMQ proxy is running.

```rust
pub fn log_event(db_path: &Path, header: &EventHeader, body: &[u8]) -> Result<u64> {
    let conn = Connection::open(db_path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;

    conn.execute(
        "INSERT INTO events (timestamp, session_id, cwd, category, event_type, source, header_json, body)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            header.timestamp.to_rfc3339(),
            header.session_id,
            header.cwd.display().to_string(),
            header.category.as_str(),
            header.event_type,
            header.source,
            serde_json::to_string(header)?,
            body,
        ],
    )?;

    let seq = conn.last_insert_rowid() as u64;
    // conn drops here — connection closed
    Ok(seq)
}
```

### Schema

```sql
CREATE TABLE IF NOT EXISTS events (
    seq         INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp   TEXT    NOT NULL,
    session_id  TEXT    NOT NULL,
    cwd         TEXT    NOT NULL,
    category    TEXT    NOT NULL,
    event_type  TEXT    NOT NULL,
    source      TEXT    NOT NULL,
    header_json TEXT    NOT NULL,
    body        BLOB
);

CREATE INDEX IF NOT EXISTS idx_events_session ON events (session_id, seq);
CREATE INDEX IF NOT EXISTS idx_events_category ON events (category, seq);
CREATE INDEX IF NOT EXISTS idx_events_cwd ON events (cwd, seq);
```

### DB Location

XDG-aware context: `$XDG_DATA_HOME/heb/events.db` (default `~/.local/share/heb/events.db`).
Single file, all projects. Scoped by `cwd` and `session_id`.

## HEB Context

The context object is the entry point. It owns XDG path resolution,
discovery file management, and provides the publish/subscribe API.

```rust
pub struct HebContext {
    /// XDG_DATA_HOME/heb/ — database lives here
    data_dir: PathBuf,
    /// XDG_RUNTIME_DIR/heb/ — discovery file and IPC sockets live here
    runtime_dir: PathBuf,
    /// Hash of the workspace root — scopes discovery + IPC paths
    context_hash: String,
    /// Our role — leader (owns proxy) or follower (connected to proxy)
    role: HebRole,
}

enum HebRole {
    Leader {
        guard: LeaderGuard,
        proxy_handle: JoinHandle<()>,
    },
    Follower {
        guard: FollowerGuard,
    },
}

impl HebContext {
    /// Open a HEB context for the given workspace.
    ///
    /// This is not passive — it participates in leader election.
    /// If no discovery file exists (no leader), this process contests
    /// the election. The winner starts the proxy and writes the
    /// discovery file. The loser reads it and connects.
    ///
    /// Every process that touches heb is a potential leader.
    pub fn open(workspace_root: &Path) -> Result<Self> {
        let ctx = Self::resolve_paths(workspace_root);

        let config = ElectionConfig::new().with_prefix("heb");
        let outcome = LeaderElection::with_config(workspace_root, config).elect()?;

        match outcome {
            ElectionOutcome::Leader(guard) => {
                // We're the leader — start proxy, write discovery file
                let proxy_handle = start_proxy(&ctx.front_addr(), &ctx.back_addr())?;
                write_discovery_file(&ctx.discovery_path(), &ctx.front_addr(), &ctx.back_addr())?;
                // ...
            }
            ElectionOutcome::Follower(guard) => {
                // Someone else is leader — discovery file should exist (or appear shortly)
                // Connect PUB/SUB sockets using addresses from discovery file
                // ...
            }
        }

        Ok(ctx)
    }

    pub fn db_path(&self) -> PathBuf { self.data_dir.join("events.db") }
    pub fn discovery_path(&self) -> PathBuf { self.runtime_dir.join(format!("{}.addr", self.context_hash)) }
    pub fn front_addr(&self) -> String { format!("ipc://{}/{}-front.sock", self.runtime_dir.display(), self.context_hash) }
    pub fn back_addr(&self) -> String { format!("ipc://{}/{}-back.sock", self.runtime_dir.display(), self.context_hash) }

    /// Publish: write to SQLite + send via ZMQ
    pub fn publish(&self, header: &EventHeader, body: &[u8]) -> Result<u64> { ... }

    /// Subscribe: connect SUB socket to proxy backend
    pub fn subscribe(&self, categories: &[EventCategory]) -> Result<HebSubscriber> { ... }

    /// Replay from SQLite (catch-up after reconnect or leader transition)
    pub fn replay(&self, since_seq: u64, filter: Option<&str>) -> Result<Vec<(EventHeader, Vec<u8>)>> { ... }
}
```

## Publish Flow

```
publish(header, body)
  │
  ├─1─ log_event(db_path, header, body)    ← always, open/write/close
  │    └─ returns seq
  │
  └─2─ send via PUB socket                 ← always (connected since open())
       └─ [topic, header_json, body]
```

The PUB socket is connected during `HebContext::open()` and cached for the
process lifetime. Both leaders and followers connect PUB to the frontend —
the leader connects to its own proxy.

## Subscribe Flow

```
subscribe(categories)
  │
  ├─1─ connect SUB socket to back_addr (known since open())
  ├─2─ set_subscribe for each category (or "" for all)
  └─3─ return HebSubscriber (iterator/stream over events)
```

ZMQ is the primary delivery path. Replay from SQLite is only for catch-up
after a leader transition gap, not for normal operation.

## Lifecycle During Leader Transition

```
Time ──────────────────────────────────────────────────>

Leader A:  [====proxy running====]  ← dies
                                    │
                                    ├─ LeaderGuard::drop()
                                    │  ├─ signal proxy stop
                                    │  ├─ remove discovery file
                                    │  └─ remove IPC socket files
                                    │
Follower B:                         ├─ try_promote() succeeds
                                    │  ├─ bind new XSUB + XPUB
                                    │  ├─ write new discovery file
                                    │  └─ start proxy thread
                                    │
                                    └─ [====proxy running====]

Publishers:  ...connected to A... ──zmq reconnect──> ...connected to B...
Subscribers: ...connected to A... ──zmq reconnect──> ...connected to B...

Gap: a few hundred ms. Events during gap still go to SQLite.
     Subscribers can replay missed seqs on reconnect.
```

## What's Different from Hence

| Aspect | Hence | heb |
|--------|-------|-----|
| Transport | Unix socket to daemon | ZMQ XPUB/XSUB via leader election |
| Broker | Mirdan daemon | Leader process runs proxy |
| Failover | Daemon restart | Follower promotion, auto-reconnect |
| DB connection | Persistent (daemon holds it) | Open/write/close per event |
| Event shape | Flat struct with `data: Value` | Header/body, 3-frame ZMQ message |
| Required fields | `project`, `summary` | `session_id`, `cwd` |
| Filtering | In-memory EventFilter | ZMQ topic prefix on category |
| Replay | Daemon serves via SSE | Direct SQLite read (any process) |
| Path resolution | Hardcoded paths | XDG-aware context object |
| Discovery | N/A (known daemon socket) | Leader writes discovery file |

## Crate Structure

Leader election becomes the bus. heb is a consumer of it.

```
swissarmyhammer-leader-election/
  Cargo.toml          -- gains zmq (C bindings) dependency
  src/
    lib.rs            -- re-exports
    election.rs       -- LeaderElection<M>, ElectionConfig, elect()
    bus.rs            -- BusMessage trait, Publisher<M>, Subscriber<M>
    proxy.rs          -- XPUB/XSUB proxy thread
    discovery.rs      -- discovery file read/write
    error.rs          -- error types

heb/
  Cargo.toml          -- depends on swissarmyhammer-leader-election, rusqlite, serde, chrono
  src/
    lib.rs            -- pub API: HebContext, HebEvent
    context.rs        -- HebContext: XDG paths, open(), publish(), subscribe()
    header.rs         -- EventHeader, EventCategory
    store.rs          -- SQLite WAL open/write/close
```

### Testing with Existing Users

The current leader election users (code-context) can serve as the first
test bed. Define a trivial message type — even a `Vec<u8>` — and verify
that the generic election + proxy works without changing code-context's
behavior. The bus is always there; if nobody publishes or subscribes,
it's just an idle proxy thread forwarding nothing. Zero cost to existing
consumers.

```rust
// Minimal message type for code-context (doesn't need heb's event model)
pub struct RawMessage(pub Vec<u8>);

impl BusMessage for RawMessage {
    fn topic(&self) -> &[u8] { b"" }
    fn to_frames(&self) -> Result<Vec<Vec<u8>>> { Ok(vec![self.0.clone()]) }
    fn from_frames(_topic: &[u8], frames: &[Vec<u8>]) -> Result<Self> {
        Ok(RawMessage(frames[0].clone()))
    }
}

// code-context just uses it as before, bus is there but silent
let outcome = LeaderElection::<RawMessage>::with_config(root, config).elect()?;
```

## Open Questions

1. **Body encoding**: JSON to start. Add `content_type` header field later if
   we need msgpack/CBOR.

2. **Retention**: Auto-prune events older than N days on publish? Or separate
   `heb gc` command? Probably a simple `DELETE WHERE timestamp < ?` on publish
   every Nth event.

3. **Cross-machine**: Swap `ipc://` for `tcp://` in the discovery file. The
   architecture supports it but park for now.

4. **ZMQ HWM (high water mark)**: Default is 1000 messages. If a subscriber
   is slow and hits HWM, messages drop. Fine for our use case — they're in
   SQLite anyway. But worth tuning if events are bursty.

5. **zmq is now a dependency of leader election.** Every election instance
   runs a bus. This is a feature, not a cost — it gives every leader election
   user a free communication channel. The `BusMessage` trait keeps the message
   types decoupled. If zmq C dependency is a problem for some build target,
   we can feature-gate it, but for now it's always-on.
