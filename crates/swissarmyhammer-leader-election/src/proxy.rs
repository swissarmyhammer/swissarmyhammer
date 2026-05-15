//! XPUB/XSUB proxy thread for the bus.
//!
//! The leader spawns this thread to forward messages between publishers and subscribers.
//! Uses a manual poll loop (not `zmq::proxy()`) so we can cleanly shut down.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use crate::discovery::BusAddresses;
use crate::error::{ElectionError, Result};

/// Handle to a running proxy thread. Stops the proxy on drop.
pub struct ProxyHandle {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    zmq_ctx: zmq::Context,
}

impl ProxyHandle {
    /// Start the XPUB/XSUB proxy on the given addresses.
    ///
    /// Binds XSUB on `frontend` (publishers connect here) and XPUB on `backend`
    /// (subscribers connect here). Spawns a thread that forwards messages.
    pub fn start(addrs: &BusAddresses) -> Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let zmq_ctx = zmq::Context::new();

        let front_addr = addrs.frontend.clone();
        let back_addr = addrs.backend.clone();
        let ctx = zmq_ctx.clone();
        let stop_clone = stop.clone();

        let thread = thread::spawn(move || {
            if let Err(e) = run_proxy(&ctx, &front_addr, &back_addr, &stop_clone) {
                if !stop_clone.load(Ordering::Relaxed) {
                    tracing::error!("Bus proxy error: {}", e);
                }
            }
        });

        Ok(ProxyHandle {
            stop,
            thread: Some(thread),
            zmq_ctx,
        })
    }

    /// Get a reference to the ZMQ context (for creating publisher/subscriber sockets).
    pub fn zmq_context(&self) -> &zmq::Context {
        &self.zmq_ctx
    }
}

impl Drop for ProxyHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Run the XPUB/XSUB forwarding proxy using a poll loop.
///
/// Forwards messages in both directions:
/// - XSUB → XPUB: data messages from publishers to subscribers
/// - XPUB → XSUB: subscription messages from subscribers to publishers
fn run_proxy(
    ctx: &zmq::Context,
    front_addr: &str,
    back_addr: &str,
    stop: &Arc<AtomicBool>,
) -> Result<()> {
    let frontend = ctx.socket(zmq::XSUB).map_err(ElectionError::Bus)?;
    frontend.bind(front_addr).map_err(ElectionError::Bus)?;

    let backend = ctx.socket(zmq::XPUB).map_err(ElectionError::Bus)?;
    backend.bind(back_addr).map_err(ElectionError::Bus)?;

    // Poll loop with 50ms timeout so we can check the stop flag
    while !stop.load(Ordering::Acquire) {
        let mut items = [
            frontend.as_poll_item(zmq::POLLIN),
            backend.as_poll_item(zmq::POLLIN),
        ];

        match zmq::poll(&mut items, 50) {
            Ok(_) => {}
            Err(zmq::Error::ETERM) => break,
            Err(e) => return Err(ElectionError::Bus(e)),
        }

        // Forward XSUB → XPUB (publisher data → subscribers)
        if items[0].is_readable() {
            if let Ok(msg) = frontend.recv_multipart(zmq::DONTWAIT) {
                let _ = backend.send_multipart(&msg, 0);
            }
        }

        // Forward XPUB → XSUB (subscription management)
        if items[1].is_readable() {
            if let Ok(msg) = backend.recv_multipart(zmq::DONTWAIT) {
                let _ = frontend.send_multipart(&msg, 0);
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tempfile::TempDir;

    #[test]
    fn test_proxy_start_stop() {
        let dir = TempDir::new().unwrap();
        let addrs = crate::discovery::ipc_addresses(dir.path(), "test", "proxy");

        let handle = ProxyHandle::start(&addrs).unwrap();
        thread::sleep(Duration::from_millis(50));
        drop(handle);
    }

    #[test]
    fn test_proxy_send_receive() {
        let dir = TempDir::new().unwrap();
        let addrs = crate::discovery::ipc_addresses(dir.path(), "test", "pubsub");

        let handle = ProxyHandle::start(&addrs).unwrap();
        thread::sleep(Duration::from_millis(100));

        let ctx = handle.zmq_context();

        // Publisher connects to frontend (XSUB)
        let pub_sock = ctx.socket(zmq::PUB).unwrap();
        pub_sock.connect(&addrs.frontend).unwrap();

        // Subscriber connects to backend (XPUB)
        let sub_sock = ctx.socket(zmq::SUB).unwrap();
        sub_sock.connect(&addrs.backend).unwrap();
        sub_sock.set_subscribe(b"").unwrap();
        sub_sock.set_rcvtimeo(2000).unwrap();

        // Let subscriptions propagate
        thread::sleep(Duration::from_millis(300));

        // Send a message
        pub_sock.send("hello", 0).unwrap();

        // Receive it
        let msg = sub_sock.recv_string(0).unwrap().unwrap();
        assert_eq!(msg, "hello");

        drop(handle);
    }
}
