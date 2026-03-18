//! E2E integration tests for HEB.
//!
//! Each test creates an isolated temp directory with its own:
//! - SQLite database (created, used, torn down with TempDir)
//! - ZMQ IPC sockets (in temp dir, cleaned up on drop)
//! - Leader election lock files (in temp dir)
//!
//! Tests verify the full flow: leader election → proxy → publish → subscribe → SQLite persist.

use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use heb::{EventCategory, EventHeader, HebEvent};
use swissarmyhammer_leader_election::{
    ElectionConfig, ElectionOutcome, LeaderElection, Subscriber,
};
use tempfile::TempDir;

/// Isolated test environment — each test gets its own temp dir with
/// workspace, runtime (IPC sockets), and data (SQLite) subdirectories.
struct TestEnv {
    _dir: TempDir, // dropped last → cleans up everything
    workspace: PathBuf,
    runtime: PathBuf,
    data: PathBuf,
}

impl TestEnv {
    fn new() -> Self {
        let dir = TempDir::new().unwrap();
        let workspace = dir.path().join("ws");
        let runtime = dir.path().join("rt");
        let data = dir.path().join("data");
        std::fs::create_dir_all(&workspace).unwrap();
        std::fs::create_dir_all(&runtime).unwrap();
        std::fs::create_dir_all(&data).unwrap();
        Self {
            _dir: dir,
            workspace,
            runtime,
            data,
        }
    }

    fn db_path(&self) -> PathBuf {
        self.data.join("events.db")
    }

    fn init_db(&self) {
        heb::store::init_schema(&self.db_path()).unwrap();
    }

    fn elect(&self) -> ElectionOutcome<HebEvent> {
        let config = ElectionConfig::new()
            .with_prefix("heb-e2e")
            .with_base_dir(&self.runtime);
        LeaderElection::<HebEvent>::with_config(&self.workspace, config)
            .elect()
            .unwrap()
    }

    fn expect_leader(&self) -> swissarmyhammer_leader_election::LeaderGuard<HebEvent> {
        match self.elect() {
            ElectionOutcome::Leader(g) => g,
            _ => panic!("expected leader"),
        }
    }

    fn expect_follower(&self) -> swissarmyhammer_leader_election::FollowerGuard<HebEvent> {
        match self.elect() {
            ElectionOutcome::Follower(f) => f,
            _ => panic!("expected follower"),
        }
    }

    /// Publish an event to both SQLite (durable) and ZMQ (live).
    fn publish_to_db(&self, header: &EventHeader, body: &[u8]) -> String {
        heb::store::log_event(&self.db_path(), header, body).unwrap()
    }

    fn replay_all(&self) -> Vec<(EventHeader, Vec<u8>)> {
        heb::store::replay(&self.db_path(), "", None).unwrap()
    }

    fn replay_since(&self, since_id: &str) -> Vec<(EventHeader, Vec<u8>)> {
        heb::store::replay(&self.db_path(), since_id, None).unwrap()
    }

    fn replay_category(&self, cat: &str) -> Vec<(EventHeader, Vec<u8>)> {
        heb::store::replay(&self.db_path(), "", Some(cat)).unwrap()
    }
}

fn make_header(event_type: &str, category: EventCategory) -> EventHeader {
    EventHeader::new(
        "e2e-session",
        "/workspace",
        category,
        event_type,
        "e2e-test",
    )
}

fn make_event(event_type: &str, category: EventCategory, body: &[u8]) -> HebEvent {
    HebEvent {
        header: make_header(event_type, category),
        body: body.to_vec(),
    }
}

/// Wait for ZMQ subscription to propagate through the proxy.
fn wait_for_subscriptions() {
    thread::sleep(Duration::from_millis(300));
}

// ─── Bus-only tests (ZMQ pub/sub, no SQLite) ──────────────────────────

#[test]
fn test_leader_publishes_subscriber_receives() {
    let env = TestEnv::new();
    let leader = env.expect_leader();

    let sub: Subscriber<HebEvent> = leader.subscribe(&[]).unwrap();
    wait_for_subscriptions();

    let event = make_event("pre_tool_use", EventCategory::Hook, b"tool: Read");
    leader.publish(&event).unwrap();

    let received = sub.recv_timeout(Duration::from_secs(2));
    assert!(received.is_some(), "should receive message");
    let received = received.unwrap().unwrap();
    assert_eq!(received.header.event_type, "pre_tool_use");
    assert_eq!(received.body, b"tool: Read");
}

#[test]
fn test_follower_publishes_through_leader_proxy() {
    let env = TestEnv::new();
    let leader = env.expect_leader();
    let follower = env.expect_follower();

    let sub: Subscriber<HebEvent> = leader.subscribe(&[]).unwrap();
    wait_for_subscriptions();

    // Follower publishes → goes through leader's proxy → subscriber receives
    let event = make_event("post_tool_use", EventCategory::Hook, b"result: ok");
    follower.publish(&event).unwrap();

    let received = sub.recv_timeout(Duration::from_secs(2));
    assert!(received.is_some(), "should receive follower's message");
    let received = received.unwrap().unwrap();
    assert_eq!(received.header.event_type, "post_tool_use");
    assert_eq!(received.body, b"result: ok");
}

#[test]
fn test_leader_and_follower_hear_each_other() {
    let env = TestEnv::new();
    let leader = env.expect_leader();
    let follower = env.expect_follower();

    // Both subscribe
    let leader_sub: Subscriber<HebEvent> = leader.subscribe(&[]).unwrap();
    let follower_sub: Subscriber<HebEvent> = follower.subscribe(&[]).unwrap();
    wait_for_subscriptions();

    // Leader publishes → follower hears it
    let leader_event = make_event("from_leader", EventCategory::Session, b"hello from leader");
    leader.publish(&leader_event).unwrap();

    let follower_heard = follower_sub.recv_timeout(Duration::from_secs(2));
    assert!(follower_heard.is_some(), "follower should hear leader");
    let follower_heard = follower_heard.unwrap().unwrap();
    assert_eq!(follower_heard.header.event_type, "from_leader");
    assert_eq!(follower_heard.body, b"hello from leader");

    // Leader also hears its own message (subscribed to all)
    let leader_echo = leader_sub.recv_timeout(Duration::from_secs(2));
    assert!(leader_echo.is_some(), "leader should hear its own message");
    assert_eq!(
        leader_echo.unwrap().unwrap().header.event_type,
        "from_leader"
    );

    // Follower publishes → leader hears it
    let follower_event = make_event(
        "from_follower",
        EventCategory::Agent,
        b"hello from follower",
    );
    follower.publish(&follower_event).unwrap();

    let leader_heard = leader_sub.recv_timeout(Duration::from_secs(2));
    assert!(leader_heard.is_some(), "leader should hear follower");
    let leader_heard = leader_heard.unwrap().unwrap();
    assert_eq!(leader_heard.header.event_type, "from_follower");
    assert_eq!(leader_heard.body, b"hello from follower");

    // Follower also hears its own message
    let follower_echo = follower_sub.recv_timeout(Duration::from_secs(2));
    assert!(
        follower_echo.is_some(),
        "follower should hear its own message"
    );
    assert_eq!(
        follower_echo.unwrap().unwrap().header.event_type,
        "from_follower"
    );
}

#[test]
fn test_topic_filtering() {
    let env = TestEnv::new();
    let leader = env.expect_leader();

    // Subscribe only to "session" topic
    let sub: Subscriber<HebEvent> = leader.subscribe(&[b"session"]).unwrap();
    wait_for_subscriptions();

    // Publish a hook event (filtered out) then a session event (received)
    leader
        .publish(&make_event("pre_tool_use", EventCategory::Hook, b"hook"))
        .unwrap();
    leader
        .publish(&make_event("start", EventCategory::Session, b"session"))
        .unwrap();

    let received = sub.recv_timeout(Duration::from_secs(2));
    assert!(received.is_some());
    let received = received.unwrap().unwrap();
    assert_eq!(received.header.category, EventCategory::Session);

    // Hook event should not arrive
    let nothing = sub.recv_timeout(Duration::from_millis(500));
    assert!(nothing.is_none(), "hook event should be filtered out");
}

// ─── SQLite persistence tests (database created + torn down per test) ──

#[test]
fn test_sqlite_database_created_and_torn_down() {
    let env = TestEnv::new();
    env.init_db();

    let db_path = env.db_path();
    assert!(db_path.exists(), "database should be created");

    // Write an event
    let h = make_header("test", EventCategory::Hook);
    let id = env.publish_to_db(&h, b"body");
    assert_eq!(id, h.id);

    // Verify it's there
    let events = env.replay_all();
    assert_eq!(events.len(), 1);

    // TempDir drops here → database file deleted
    let db_path_copy = db_path.clone();
    drop(env);
    assert!(
        !db_path_copy.exists(),
        "database should be cleaned up by TempDir"
    );
}

#[test]
fn test_leader_and_follower_events_both_persist() {
    let env = TestEnv::new();
    env.init_db();
    let leader = env.expect_leader();
    let follower = env.expect_follower();

    // Both publish events, both write to the same SQLite
    let h1 = EventHeader::new(
        "leader-sess",
        "/ws",
        EventCategory::Hook,
        "leader_event",
        "leader",
    );
    let h2 = EventHeader::new(
        "follower-sess",
        "/ws",
        EventCategory::Agent,
        "follower_event",
        "follower",
    );

    let id1 = env.publish_to_db(&h1, b"leader body");
    let id2 = env.publish_to_db(&h2, b"follower body");
    assert_eq!(id1, h1.id);
    assert_eq!(id2, h2.id);
    assert!(id1 < id2, "second ULID should sort after first");

    // Also send via ZMQ bus
    let sub = leader.subscribe(&[]).unwrap();
    wait_for_subscriptions();

    let leader_event = HebEvent {
        header: h1.clone(),
        body: b"leader body".to_vec(),
    };
    let follower_event = HebEvent {
        header: h2.clone(),
        body: b"follower body".to_vec(),
    };
    leader.publish(&leader_event).unwrap();
    follower.publish(&follower_event).unwrap();

    // Verify both arrive via ZMQ
    let msg1 = sub.recv_timeout(Duration::from_secs(2));
    assert!(msg1.is_some(), "should receive leader's event via ZMQ");
    let msg2 = sub.recv_timeout(Duration::from_secs(2));
    assert!(msg2.is_some(), "should receive follower's event via ZMQ");

    // Verify both persisted in SQLite
    let all = env.replay_all();
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].0.source, "leader");
    assert_eq!(all[1].0.source, "follower");

    // Verify filtered replay
    let hooks = env.replay_category("hook");
    assert_eq!(hooks.len(), 1);
    assert_eq!(hooks[0].0.event_type, "leader_event");

    let agents = env.replay_category("agent");
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0].0.event_type, "follower_event");
}

#[test]
fn test_replay_since_id() {
    let env = TestEnv::new();
    env.init_db();

    let h1 = make_header("test", EventCategory::Hook);
    let h2 = make_header("test", EventCategory::Hook);
    let h3 = make_header("test", EventCategory::Hook);
    env.publish_to_db(&h1, b"event-1");
    let id2 = env.publish_to_db(&h2, b"event-2");
    env.publish_to_db(&h3, b"event-3");

    let since_2 = env.replay_since(&id2);
    assert_eq!(since_2.len(), 1);
    assert_eq!(since_2[0].1, b"event-3");
}

// ─── Leader promotion tests ────────────────────────────────────────────

#[test]
fn test_leader_promotion_resumes_bus() {
    let env = TestEnv::new();

    // Leader A wins
    let leader_a = env.expect_leader();

    // Follower B joins
    let follower_b = env.expect_follower();

    // Leader A dies
    drop(leader_a);
    thread::sleep(Duration::from_millis(200));

    // Follower B promotes to leader
    let leader_b = follower_b.try_promote().unwrap().expect("should promote");

    // New leader can subscribe and publish
    let sub = leader_b.subscribe(&[]).unwrap();
    wait_for_subscriptions();

    let event = make_event("resumed", EventCategory::System, b"promoted");
    leader_b.publish(&event).unwrap();

    let received = sub.recv_timeout(Duration::from_secs(2));
    assert!(received.is_some(), "should receive on promoted leader");
    let received = received.unwrap().unwrap();
    assert_eq!(received.header.event_type, "resumed");
}

#[test]
fn test_events_survive_leader_transition_in_sqlite() {
    let env = TestEnv::new();
    env.init_db();

    // Leader A posts events
    let leader_a = env.expect_leader();
    let h1 = make_header("before_transition", EventCategory::Hook);
    env.publish_to_db(&h1, b"from leader A");

    // Follower B
    let follower_b = env.expect_follower();

    // Leader A dies
    drop(leader_a);
    thread::sleep(Duration::from_millis(200));

    // Follower B promotes and posts events
    let _leader_b = follower_b.try_promote().unwrap().expect("should promote");
    let h2 = make_header("after_transition", EventCategory::Hook);
    env.publish_to_db(&h2, b"from leader B");

    // Both events survive in SQLite
    let all = env.replay_all();
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].0.event_type, "before_transition");
    assert_eq!(all[0].1, b"from leader A");
    assert_eq!(all[1].0.event_type, "after_transition");
    assert_eq!(all[1].1, b"from leader B");
}
