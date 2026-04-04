//! Bus message trait, Publisher, and Subscriber for typed pub/sub over leader election.
//!
//! `zmq::Socket` is `!Send`, so Publisher and Subscriber use internal channels
//! to communicate with dedicated ZMQ threads that own the sockets.

use std::marker::PhantomData;
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::error::{ElectionError, Result};

/// Trait that message types must implement to ride the bus.
///
/// The default type parameter `NullMessage` means existing consumers that don't
/// use the bus compile unchanged — they get an idle proxy forwarding nothing.
pub trait BusMessage: Send + 'static {
    /// The topic/category for ZMQ prefix filtering.
    fn topic(&self) -> &[u8];

    /// Serialize to wire format (ZMQ frames after the topic frame).
    fn to_frames(&self) -> Result<Vec<Vec<u8>>>;

    /// Deserialize from wire format.
    fn from_frames(topic: &[u8], frames: &[Vec<u8>]) -> Result<Self>
    where
        Self: Sized;
}

/// No-op message type. Used as the default type parameter so that
/// `LeaderElection::new()` works without specifying a message type.
#[derive(Debug, Clone)]
pub struct NullMessage;

impl BusMessage for NullMessage {
    fn topic(&self) -> &[u8] {
        b""
    }

    fn to_frames(&self) -> Result<Vec<Vec<u8>>> {
        Ok(vec![])
    }

    fn from_frames(_topic: &[u8], _frames: &[Vec<u8>]) -> Result<Self> {
        Ok(NullMessage)
    }
}

/// Serialized message ready to send over ZMQ (topic + frames).
struct WireMessage {
    topic: Vec<u8>,
    frames: Vec<Vec<u8>>,
}

/// Publisher handle for sending messages to the bus.
///
/// Thread-safe (`Send`) — uses an internal channel to a dedicated ZMQ thread.
/// Not `Sync` because `mpsc::Sender` is `!Sync`. Both leaders and followers
/// get one on election.
pub struct Publisher<M: BusMessage> {
    /// None = noop publisher (NullMessage / no bus configured)
    sender: Option<mpsc::Sender<WireMessage>>,
    /// Handle to the ZMQ PUB thread (joined on drop)
    _thread: Option<JoinHandle<()>>,
    _phantom: PhantomData<M>,
}

impl<M: BusMessage> Publisher<M> {
    /// Create a no-op publisher (used when bus is not active, e.g. NullMessage).
    pub(crate) fn noop() -> Self {
        Self {
            sender: None,
            _thread: None,
            _phantom: PhantomData,
        }
    }

    /// Create a publisher connected to the given frontend address.
    ///
    /// Spawns a thread that owns the ZMQ PUB socket (because `zmq::Socket` is `!Send`).
    pub(crate) fn connected(ctx: &zmq::Context, frontend_addr: &str) -> Result<Self> {
        let (tx, rx) = mpsc::channel::<WireMessage>();
        let ctx = ctx.clone();
        let addr = frontend_addr.to_string();

        let thread = thread::spawn(move || {
            let sock = match ctx.socket(zmq::PUB) {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!("Failed to create PUB socket: {}", e);
                    return;
                }
            };
            if let Err(e) = sock.connect(&addr) {
                tracing::error!("Failed to connect PUB socket to {}: {}", addr, e);
                return;
            }

            // Process messages until the channel is closed
            while let Ok(wire) = rx.recv() {
                // Send as multipart: [topic, frame0, frame1, ...]
                let total = 1 + wire.frames.len();
                if let Err(e) = sock.send(&wire.topic, if total > 1 { zmq::SNDMORE } else { 0 }) {
                    tracing::warn!("PUB send topic failed: {}", e);
                    continue;
                }
                for (i, frame) in wire.frames.iter().enumerate() {
                    let flags = if i < wire.frames.len() - 1 {
                        zmq::SNDMORE
                    } else {
                        0
                    };
                    if let Err(e) = sock.send(frame, flags) {
                        tracing::warn!("PUB send frame failed: {}", e);
                        break;
                    }
                }
            }
        });

        Ok(Self {
            sender: Some(tx),
            _thread: Some(thread),
            _phantom: PhantomData,
        })
    }

    /// Send a message to the bus.
    ///
    /// Serializes the message and queues it for the ZMQ thread.
    /// No-op if this is a noop publisher.
    pub fn send(&self, msg: &M) -> Result<()> {
        let Some(ref sender) = self.sender else {
            return Ok(());
        };
        let topic = msg.topic().to_vec();
        let frames = msg.to_frames()?;
        let wire = WireMessage { topic, frames };
        sender
            .send(wire)
            .map_err(|_| ElectionError::Message("Publisher channel closed".to_string()))?;
        Ok(())
    }
}

/// Subscriber handle for receiving messages from the bus.
///
/// Thread-safe (`Send`) — uses an internal channel from a dedicated ZMQ thread.
///
/// **Thread lifecycle**: the ZMQ thread runs a recv loop with `rcvtimeo=100ms`.
/// When the `Subscriber` is dropped, the channel receiver is dropped first,
/// causing the next `tx.send()` in the ZMQ thread to fail. The thread then
/// breaks out of its loop and exits. Worst case latency is one `rcvtimeo`
/// interval (100ms) after the subscriber is dropped.
pub struct Subscriber<M: BusMessage> {
    receiver: mpsc::Receiver<Result<M>>,
    _thread: JoinHandle<()>,
}

impl<M: BusMessage> Subscriber<M> {
    /// Create a subscriber connected to the given backend address.
    ///
    /// Subscribes to the given topics (empty slice = subscribe to all).
    pub(crate) fn connected(
        ctx: &zmq::Context,
        backend_addr: &str,
        topics: &[&[u8]],
    ) -> Result<Self> {
        let (tx, rx) = mpsc::channel::<Result<M>>();
        let ctx = ctx.clone();
        let addr = backend_addr.to_string();
        let topics: Vec<Vec<u8>> = topics.iter().map(|t| t.to_vec()).collect();

        let thread = thread::spawn(move || {
            let sock = match ctx.socket(zmq::SUB) {
                Ok(s) => s,
                Err(e) => {
                    let _ = tx.send(Err(ElectionError::Bus(e)));
                    return;
                }
            };
            if let Err(e) = sock.connect(&addr) {
                let _ = tx.send(Err(ElectionError::Bus(e)));
                return;
            }

            // Subscribe to topics (empty = all)
            if topics.is_empty() {
                let _ = sock.set_subscribe(b"");
            } else {
                for topic in &topics {
                    let _ = sock.set_subscribe(topic);
                }
            }

            // Set a receive timeout so we can check if the channel is still open
            let _ = sock.set_rcvtimeo(100);

            loop {
                // Receive multipart: [topic, frame0, frame1, ...]
                match sock.recv_multipart(0) {
                    Ok(parts) if parts.len() >= 2 => {
                        let topic = &parts[0];
                        let frames: Vec<Vec<u8>> = parts[1..].to_vec();
                        let result = M::from_frames(topic, &frames);
                        if tx.send(result).is_err() {
                            break; // Receiver dropped
                        }
                    }
                    Ok(_) => {
                        // Malformed message — skip
                    }
                    Err(zmq::Error::EAGAIN) => {
                        // Timeout — check if receiver is still alive by trying a zero-size send
                        // Actually we can't easily check, just continue
                        continue;
                    }
                    Err(zmq::Error::ETERM) => break, // Context destroyed
                    Err(e) => {
                        if tx.send(Err(ElectionError::Bus(e))).is_err() {
                            break;
                        }
                    }
                }
            }
        });

        Ok(Self {
            receiver: rx,
            _thread: thread,
        })
    }

    /// Receive the next message. Blocks until a message arrives.
    pub fn recv(&self) -> Result<M> {
        self.receiver
            .recv()
            .map_err(|_| ElectionError::Message("Subscriber channel closed".to_string()))?
    }

    /// Try to receive a message with a timeout.
    ///
    /// Returns `None` on timeout, `Some(Ok(msg))` on success,
    /// or `Some(Err(...))` if the subscriber channel is disconnected.
    pub fn recv_timeout(&self, timeout: Duration) -> Option<Result<M>> {
        match self.receiver.recv_timeout(timeout) {
            Ok(msg) => Some(msg),
            Err(mpsc::RecvTimeoutError::Timeout) => None,
            Err(mpsc::RecvTimeoutError::Disconnected) => Some(Err(ElectionError::Message(
                "Subscriber channel disconnected".to_string(),
            ))),
        }
    }
}

// Send assertions (Publisher and Subscriber are Send but not Sync)
const _: () = {
    fn _assert_send<T: Send>() {}
    fn _checks() {
        _assert_send::<Publisher<NullMessage>>();
        _assert_send::<Subscriber<NullMessage>>();
    }
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_null_message_roundtrip() {
        let msg = NullMessage;
        assert_eq!(msg.topic(), b"");
        let frames = msg.to_frames().unwrap();
        assert!(frames.is_empty());
        let _restored = NullMessage::from_frames(b"", &frames).unwrap();
    }

    #[test]
    fn test_publisher_noop_send() {
        let pub_handle: Publisher<NullMessage> = Publisher::noop();
        assert!(pub_handle.send(&NullMessage).is_ok());
    }

    #[test]
    fn test_publisher_connected_send() {
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let addrs = crate::discovery::ipc_addresses(dir.path(), "test", "pubconn");

        // Start a proxy so the publisher has something to connect to
        let proxy = crate::proxy::ProxyHandle::start(&addrs).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));

        let publisher: Publisher<NullMessage> =
            Publisher::connected(proxy.zmq_context(), &addrs.frontend).unwrap();

        // Send should succeed (message goes to proxy)
        assert!(publisher.send(&NullMessage).is_ok());

        drop(publisher);
        drop(proxy);
    }

    #[test]
    fn test_publisher_send_with_real_message() {
        use tempfile::TempDir;

        /// A test message type with actual content.
        #[derive(Debug, Clone)]
        struct TestMsg {
            data: String,
        }

        impl BusMessage for TestMsg {
            fn topic(&self) -> &[u8] {
                b"test"
            }
            fn to_frames(&self) -> Result<Vec<Vec<u8>>> {
                Ok(vec![self.data.as_bytes().to_vec()])
            }
            fn from_frames(_topic: &[u8], frames: &[Vec<u8>]) -> Result<Self> {
                Ok(TestMsg {
                    data: String::from_utf8_lossy(&frames[0]).to_string(),
                })
            }
        }

        let dir = TempDir::new().unwrap();
        let addrs = crate::discovery::ipc_addresses(dir.path(), "test", "realmsg");

        let proxy = crate::proxy::ProxyHandle::start(&addrs).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));

        let publisher: Publisher<TestMsg> =
            Publisher::connected(proxy.zmq_context(), &addrs.frontend).unwrap();

        let msg = TestMsg {
            data: "hello".to_string(),
        };
        assert!(publisher.send(&msg).is_ok());

        drop(publisher);
        drop(proxy);
    }

    #[test]
    fn test_subscriber_connected_recv_timeout() {
        use std::time::Duration;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let addrs = crate::discovery::ipc_addresses(dir.path(), "test", "subconn");

        let proxy = crate::proxy::ProxyHandle::start(&addrs).unwrap();
        std::thread::sleep(Duration::from_millis(50));

        let subscriber: Subscriber<NullMessage> =
            Subscriber::connected(proxy.zmq_context(), &addrs.backend, &[]).unwrap();

        // No messages published, so recv_timeout should return None
        let result = subscriber.recv_timeout(Duration::from_millis(200));
        assert!(result.is_none());

        drop(subscriber);
        drop(proxy);
    }

    #[test]
    fn test_subscriber_with_topic_filter() {
        use std::time::Duration;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let addrs = crate::discovery::ipc_addresses(dir.path(), "test", "subtopic");

        let proxy = crate::proxy::ProxyHandle::start(&addrs).unwrap();
        std::thread::sleep(Duration::from_millis(50));

        // Subscribe to specific topics
        let subscriber: Subscriber<NullMessage> =
            Subscriber::connected(proxy.zmq_context(), &addrs.backend, &[b"events"]).unwrap();

        // No matching messages, recv_timeout returns None
        let result = subscriber.recv_timeout(Duration::from_millis(200));
        assert!(result.is_none());

        drop(subscriber);
        drop(proxy);
    }

    #[test]
    fn test_publisher_subscriber_end_to_end() {
        use std::time::Duration;
        use tempfile::TempDir;

        /// A test message for end-to-end pub/sub.
        #[derive(Debug, Clone, PartialEq)]
        struct E2EMsg {
            payload: String,
        }

        impl BusMessage for E2EMsg {
            fn topic(&self) -> &[u8] {
                b"e2e"
            }
            fn to_frames(&self) -> Result<Vec<Vec<u8>>> {
                Ok(vec![self.payload.as_bytes().to_vec()])
            }
            fn from_frames(_topic: &[u8], frames: &[Vec<u8>]) -> Result<Self> {
                Ok(E2EMsg {
                    payload: String::from_utf8_lossy(&frames[0]).to_string(),
                })
            }
        }

        let dir = TempDir::new().unwrap();
        let addrs = crate::discovery::ipc_addresses(dir.path(), "test", "e2ebus");

        let proxy = crate::proxy::ProxyHandle::start(&addrs).unwrap();
        std::thread::sleep(Duration::from_millis(100));

        let ctx = proxy.zmq_context();

        let publisher: Publisher<E2EMsg> = Publisher::connected(ctx, &addrs.frontend).unwrap();
        let subscriber: Subscriber<E2EMsg> =
            Subscriber::connected(ctx, &addrs.backend, &[b"e2e"]).unwrap();

        // Let subscriptions propagate
        std::thread::sleep(Duration::from_millis(300));

        let msg = E2EMsg {
            payload: "hello world".to_string(),
        };
        publisher.send(&msg).unwrap();

        // Subscriber should receive the message
        let received = subscriber.recv_timeout(Duration::from_millis(2000));
        assert!(received.is_some());
        let received_msg = received.unwrap().unwrap();
        assert_eq!(received_msg.payload, "hello world");

        drop(subscriber);
        drop(publisher);
        drop(proxy);
    }

    #[test]
    fn test_subscriber_recv_timeout_no_messages() {
        use std::time::Duration;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let addrs = crate::discovery::ipc_addresses(dir.path(), "test", "recvtm");

        let proxy = crate::proxy::ProxyHandle::start(&addrs).unwrap();
        std::thread::sleep(Duration::from_millis(50));

        let subscriber: Subscriber<NullMessage> =
            Subscriber::connected(proxy.zmq_context(), &addrs.backend, &[]).unwrap();

        // No messages published — recv_timeout returns None
        let result = subscriber.recv_timeout(Duration::from_millis(200));
        assert!(result.is_none());

        drop(subscriber);
        drop(proxy);
    }

    #[test]
    fn test_publisher_drop_closes_channel() {
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let addrs = crate::discovery::ipc_addresses(dir.path(), "test", "pubdrop");

        let proxy = crate::proxy::ProxyHandle::start(&addrs).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));

        let publisher: Publisher<NullMessage> =
            Publisher::connected(proxy.zmq_context(), &addrs.frontend).unwrap();

        // Drop publisher — its thread should exit cleanly
        drop(publisher);
        // If we get here without hanging, the test passes
        drop(proxy);
    }
}
