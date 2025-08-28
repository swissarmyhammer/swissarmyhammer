use tokio::signal;
use tracing::info;

#[allow(dead_code)]
pub async fn setup_signal_handlers() -> anyhow::Result<()> {
    tokio::spawn(async {
        let ctrl_c = async {
            signal::ctrl_c()
                .await
                .expect("failed to install Ctrl+C handler");
        };

        #[cfg(unix)]
        let terminate = async {
            signal::unix::signal(signal::unix::SignalKind::terminate())
                .expect("failed to install signal handler")
                .recv()
                .await;
        };

        #[cfg(not(unix))]
        let terminate = std::future::pending::<()>();

        tokio::select! {
            _ = ctrl_c => {
                info!("Received Ctrl+C signal, shutting down gracefully...");
            },
            _ = terminate => {
                info!("Received terminate signal, shutting down gracefully...");
            },
        }
    });

    Ok(())
}

/// Wait for shutdown signals (Ctrl+C or SIGTERM)
pub async fn wait_for_shutdown() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    async fn test_signal_handler_setup() {
        // Simply test that the function can be called without panicking
        let result = setup_signal_handlers().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_signal_handler_behavior() {
        // Create a flag to track if handler was called
        let handler_called = Arc::new(AtomicBool::new(false));
        let handler_called_clone = handler_called.clone();

        // Spawn a task that sets up a custom signal handler
        tokio::spawn(async move {
            let ctrl_c = async {
                signal::ctrl_c()
                    .await
                    .expect("failed to install Ctrl+C handler");
            };

            #[cfg(unix)]
            let terminate = async {
                signal::unix::signal(signal::unix::SignalKind::terminate())
                    .expect("failed to install signal handler")
                    .recv()
                    .await;
            };

            #[cfg(not(unix))]
            let terminate = std::future::pending::<()>();

            tokio::select! {
                _ = ctrl_c => {
                    handler_called_clone.store(true, Ordering::SeqCst);
                    info!("Test: Received Ctrl+C signal");
                },
                _ = terminate => {
                    handler_called_clone.store(true, Ordering::SeqCst);
                    info!("Test: Received terminate signal");
                },
            }
        });

        // Give the handler time to install
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Test that handler is not called initially
        assert!(!handler_called.load(Ordering::SeqCst));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_unix_terminate_signal_setup() {
        // Test that we can set up a terminate signal handler on Unix
        let result = signal::unix::signal(signal::unix::SignalKind::terminate());
        assert!(
            result.is_ok(),
            "Should be able to create terminate signal handler on Unix"
        );

        let mut signal = result.unwrap();

        // Test that we can poll the signal without blocking
        let result = timeout(Duration::from_millis(10), signal.recv()).await;
        assert!(result.is_err(), "Should timeout when no signal is sent");
    }

    #[tokio::test]
    async fn test_ctrl_c_signal_setup() {
        // Test that we can set up Ctrl+C handler
        let ctrl_c_future = signal::ctrl_c();

        // This should not block or panic
        let result = timeout(Duration::from_millis(10), ctrl_c_future).await;

        // We expect a timeout since no signal was sent
        assert!(result.is_err(), "Should timeout when no Ctrl+C is sent");
    }

    #[tokio::test]
    async fn test_signal_handler_does_not_block() {
        // Test that setup_signal_handlers returns immediately without blocking
        let start = std::time::Instant::now();

        let result = setup_signal_handlers().await;
        assert!(result.is_ok());

        let elapsed = start.elapsed();

        // The function should return almost immediately (within 100ms)
        assert!(
            elapsed.as_millis() < 100,
            "setup_signal_handlers should not block, but took {elapsed:?}"
        );
    }

    #[tokio::test]
    async fn test_multiple_signal_handler_setup() {
        // Test that we can set up signal handlers multiple times without errors
        for _ in 0..3 {
            let result = setup_signal_handlers().await;
            assert!(
                result.is_ok(),
                "Should be able to set up signal handlers multiple times"
            );

            // Small delay between setups
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
}
