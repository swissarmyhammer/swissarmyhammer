//! Tests for the JSON-RPC framing helpers, [`send_jsonrpc_message`] and
//! [`read_jsonrpc_message`].

use super::*;

#[tokio::test]
async fn test_jsonrpc_roundtrip() {
    let msg = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": { "processId": 123 }
    });

    // Write to a buffer
    let mut buf: Vec<u8> = Vec::new();
    send_jsonrpc_message(&mut buf, &msg).await.unwrap();

    // Read back
    let mut cursor = &buf[..];
    let mut reader = BufReader::new(&mut cursor);
    let decoded = read_jsonrpc_message(&mut reader).await.unwrap();

    assert_eq!(decoded["jsonrpc"], "2.0");
    assert_eq!(decoded["id"], 1);
    assert_eq!(decoded["method"], "initialize");
    assert_eq!(decoded["params"]["processId"], 123);
}

#[tokio::test]
async fn test_jsonrpc_multiple_messages() {
    let msg1 = json!({"jsonrpc": "2.0", "id": 1, "method": "foo"});
    let msg2 = json!({"jsonrpc": "2.0", "id": 2, "method": "bar"});

    let mut buf: Vec<u8> = Vec::new();
    send_jsonrpc_message(&mut buf, &msg1).await.unwrap();
    send_jsonrpc_message(&mut buf, &msg2).await.unwrap();

    let mut cursor = &buf[..];
    let mut reader = BufReader::new(&mut cursor);

    let decoded1 = read_jsonrpc_message(&mut reader).await.unwrap();
    assert_eq!(decoded1["id"], 1);

    let decoded2 = read_jsonrpc_message(&mut reader).await.unwrap();
    assert_eq!(decoded2["id"], 2);
}

#[tokio::test]
async fn test_jsonrpc_missing_content_length() {
    let bad_input = b"SomeHeader: value\r\n\r\n{}";
    let mut cursor = &bad_input[..];
    let mut reader = BufReader::new(&mut cursor);
    let result = read_jsonrpc_message(&mut reader).await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("missing Content-Length"));
}

#[tokio::test]
async fn test_jsonrpc_eof() {
    let empty: &[u8] = b"";
    let mut cursor = empty;
    let mut reader = BufReader::new(&mut cursor);
    let result = read_jsonrpc_message(&mut reader).await;
    assert!(result.is_err());
}

// -- JSON-RPC edge case tests -------------------------------------------

#[tokio::test]
async fn test_jsonrpc_bad_content_length_value() {
    let bad_input = b"Content-Length: abc\r\n\r\n";
    let mut cursor = &bad_input[..];
    let mut reader = BufReader::new(&mut cursor);
    let result = read_jsonrpc_message(&mut reader).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("bad Content-Length"),
        "expected bad Content-Length error, got: {err}"
    );
}

#[tokio::test]
async fn test_jsonrpc_ignores_non_content_length_headers() {
    // Build a valid message with extra headers that should be ignored
    let body = r#"{"jsonrpc":"2.0","id":1}"#;
    let msg = format!(
        "Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
    );
    let bytes = msg.as_bytes();
    let mut cursor = bytes;
    let mut reader = BufReader::new(&mut cursor);
    let decoded = read_jsonrpc_message(&mut reader).await.unwrap();
    assert_eq!(decoded["id"], 1);
}

#[tokio::test]
async fn test_jsonrpc_invalid_json_body() {
    let body = "not valid json!!!";
    let msg = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);
    let bytes = msg.as_bytes();
    let mut cursor = bytes;
    let mut reader = BufReader::new(&mut cursor);
    let result = read_jsonrpc_message(&mut reader).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("json decode"),
        "expected json decode error, got: {err}"
    );
}

#[tokio::test]
async fn test_jsonrpc_truncated_body() {
    // Content-Length says 100 but only 5 bytes available
    let msg = "Content-Length: 100\r\n\r\nhello";
    let bytes = msg.as_bytes();
    let mut cursor = bytes;
    let mut reader = BufReader::new(&mut cursor);
    let result = read_jsonrpc_message(&mut reader).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("read body"),
        "expected read body error, got: {err}"
    );
}

#[tokio::test]
async fn test_send_jsonrpc_formats_content_length_correctly() {
    let msg = json!({"jsonrpc": "2.0", "method": "test"});
    let mut buf: Vec<u8> = Vec::new();
    send_jsonrpc_message(&mut buf, &msg).await.unwrap();

    let output = String::from_utf8(buf).unwrap();
    assert!(output.starts_with("Content-Length: "));
    assert!(output.contains("\r\n\r\n"));
    // Verify the content-length value matches the actual body
    let parts: Vec<&str> = output.splitn(2, "\r\n\r\n").collect();
    let header = parts[0];
    let body = parts[1];
    let claimed_len: usize = header
        .strip_prefix("Content-Length: ")
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(claimed_len, body.len());
}

// -- send_jsonrpc_message write failure test ------------------------------

/// An async writer that always returns an I/O error on write.
struct FailingWriter;

impl tokio::io::AsyncWrite for FailingWriter {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        _buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        std::task::Poll::Ready(Err(std::io::Error::new(
            std::io::ErrorKind::BrokenPipe,
            "simulated write failure",
        )))
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::task::Poll::Ready(Err(std::io::Error::new(
            std::io::ErrorKind::BrokenPipe,
            "simulated flush failure",
        )))
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::task::Poll::Ready(Ok(()))
    }
}

#[tokio::test]
async fn test_send_jsonrpc_write_failure() {
    let msg = json!({"jsonrpc": "2.0", "method": "test"});
    let mut writer = FailingWriter;
    let result = send_jsonrpc_message(&mut writer, &msg).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("write header"),
        "expected write header error, got: {err}"
    );
}

// -- Card 5: async send/read_jsonrpc_message error path coverage --------

/// A writer that accepts the first N bytes, then returns BrokenPipe on
/// any further writes. Used to simulate a header-OK / body-fail split.
struct PartialWriter {
    remaining: usize,
}

impl tokio::io::AsyncWrite for PartialWriter {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        if self.remaining == 0 {
            return std::task::Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "partial writer exhausted",
            )));
        }
        let n = std::cmp::min(self.remaining, buf.len());
        self.remaining -= n;
        std::task::Poll::Ready(Ok(n))
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::task::Poll::Ready(Ok(()))
    }
}

/// A writer that accepts all writes but fails on flush. Used to
/// isolate the flush error path in `send_jsonrpc_message`.
struct FlushFailingWriter {
    buf: Vec<u8>,
}

impl tokio::io::AsyncWrite for FlushFailingWriter {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        self.buf.extend_from_slice(buf);
        std::task::Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::task::Poll::Ready(Err(std::io::Error::other("simulated flush failure")))
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::task::Poll::Ready(Ok(()))
    }
}

/// A reader that returns an I/O error (not EOF) on every read. Used to
/// exercise the read_line error-return in `read_jsonrpc_message`.
struct FailingReader;

impl tokio::io::AsyncRead for FailingReader {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        _buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::task::Poll::Ready(Err(std::io::Error::new(
            std::io::ErrorKind::ConnectionReset,
            "simulated read failure",
        )))
    }
}

/// `send_jsonrpc_message` must return an error when the writer fails
/// *after* the header has been written but during the body write.
/// `PartialWriter` lets us pick the exact number of bytes that will
/// succeed — just enough for the Content-Length header + CRLF.
#[tokio::test]
async fn test_send_jsonrpc_body_write_failure() {
    let msg = json!({"jsonrpc": "2.0", "method": "test", "id": 1});
    // First compute the serialized body length so we can build the
    // header exactly and size the writer to succeed on the header
    // bytes only.
    let body = serde_json::to_string(&msg).unwrap();
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    let mut writer = PartialWriter {
        remaining: header.len(),
    };
    let result = send_jsonrpc_message(&mut writer, &msg).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("write body"),
        "expected write body error, got: {err}"
    );
}

/// `send_jsonrpc_message` must return an error when the writer's
/// flush fails (header + body succeed, flush fails).
#[tokio::test]
async fn test_send_jsonrpc_flush_failure() {
    let msg = json!({"jsonrpc": "2.0", "method": "test", "id": 1});
    let mut writer = FlushFailingWriter { buf: Vec::new() };
    let result = send_jsonrpc_message(&mut writer, &msg).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("flush"),
        "expected flush error, got: {err}"
    );
}

/// `read_jsonrpc_message` must return an error when the underlying
/// reader I/O-errors during `read_line` (not EOF, a real error).
/// This exercises the `read header line: {e}` error arm.
#[tokio::test]
async fn test_read_jsonrpc_read_line_io_error() {
    let reader = FailingReader;
    let mut buf_reader = BufReader::new(reader);
    let result = read_jsonrpc_message(&mut buf_reader).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("read header line"),
        "expected read header line error, got: {err}"
    );
}

/// A reader that emits exactly one valid header line with a correct
/// Content-Length, sends the blank-line terminator, then I/O-errors
/// when the body `read_exact` is attempted. Covers the `read body: {e}`
/// error arm.
struct HeaderOkBodyFailReader {
    stage: u8,
    header_bytes: &'static [u8],
    pos: usize,
}

impl tokio::io::AsyncRead for HeaderOkBodyFailReader {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.stage {
            0 => {
                // Feed header bytes byte-by-byte (simple and robust)
                if self.pos < self.header_bytes.len() {
                    let b = self.header_bytes[self.pos];
                    buf.put_slice(&[b]);
                    self.pos += 1;
                    if self.pos == self.header_bytes.len() {
                        self.stage = 1;
                    }
                    std::task::Poll::Ready(Ok(()))
                } else {
                    self.stage = 1;
                    std::task::Poll::Ready(Ok(()))
                }
            }
            _ => std::task::Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "simulated body read failure",
            ))),
        }
    }
}

/// Read-body I/O error: the underlying reader errors during the
/// `read_exact(&mut body)` call after headers were parsed. This hits
/// the `read body: {e}` error arm.
#[tokio::test]
async fn test_read_jsonrpc_read_exact_io_error() {
    // Content-Length: 10, blank line, then body read will fail.
    let header = b"Content-Length: 10\r\n\r\n";
    let reader = HeaderOkBodyFailReader {
        stage: 0,
        header_bytes: header,
        pos: 0,
    };
    let mut buf_reader = BufReader::new(reader);
    let result = read_jsonrpc_message(&mut buf_reader).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("read body"),
        "expected read body error, got: {err}"
    );
}

/// `send_jsonrpc_message` writes header + body + flushes. Verify
/// that a happy-path call against a `Vec<u8>` produces the exact
/// expected bytes (header format + body JSON) — this locks the
/// wire format.
#[tokio::test]
async fn test_send_jsonrpc_produces_exact_wire_format() {
    let msg = json!({"jsonrpc": "2.0", "id": 42, "method": "ping"});
    let mut buf: Vec<u8> = Vec::new();
    send_jsonrpc_message(&mut buf, &msg).await.unwrap();

    let expected_body = serde_json::to_string(&msg).unwrap();
    let expected_header = format!("Content-Length: {}\r\n\r\n", expected_body.len());
    let mut expected = Vec::new();
    expected.extend_from_slice(expected_header.as_bytes());
    expected.extend_from_slice(expected_body.as_bytes());
    assert_eq!(buf, expected);
}

/// The async daemon writer and the sync client writer use one framing, so a
/// message goes on the wire as the same bytes from both.
#[tokio::test]
async fn test_daemon_and_client_write_the_same_frame() {
    let msg = json!({"jsonrpc": "2.0", "id": 7, "method": "initialize", "params": {"é": 1}});
    let mut daemon_bytes: Vec<u8> = Vec::new();
    let mut client_bytes: Vec<u8> = Vec::new();

    send_jsonrpc_message(&mut daemon_bytes, &msg).await.unwrap();
    crate::framing::write_frame(&mut client_bytes, &msg).unwrap();

    assert_eq!(daemon_bytes, client_bytes);
}

/// The async daemon reader and the sync client frame buffer read one header
/// format: a frame that the daemon reads, the client buffer reads the same.
#[tokio::test]
async fn test_daemon_and_client_read_the_same_frame() {
    let body = r#"{"jsonrpc":"2.0","id":9,"result":true}"#;
    let raw = format!(
        "Content-Type: application/json\r\nContent-Length:{}\r\n\r\n{}",
        body.len(),
        body
    );
    let mut cursor = raw.as_bytes();
    let mut reader = BufReader::new(&mut cursor);
    let mut frames = crate::framing::FrameBuffer::default();
    frames.push(raw.as_bytes());

    let from_daemon = read_jsonrpc_message(&mut reader).await.unwrap();
    let from_client = frames.take_message().unwrap();

    assert_eq!(Some(from_daemon), from_client);
}

/// `read_jsonrpc_message` accepts any case and spacing variant of the
/// `Content-Length:` header prefix, as long as the literal prefix
/// `Content-Length:` is present. Verifies the `strip_prefix` path.
#[tokio::test]
async fn test_read_jsonrpc_content_length_with_no_space_after_colon() {
    let body = r#"{"jsonrpc":"2.0"}"#;
    let msg = format!("Content-Length:{}\r\n\r\n{}", body.len(), body);
    let bytes = msg.as_bytes();
    let mut cursor = bytes;
    let mut reader = BufReader::new(&mut cursor);
    let decoded = read_jsonrpc_message(&mut reader).await.unwrap();
    assert_eq!(decoded["jsonrpc"], "2.0");
}

/// `read_jsonrpc_message` must surface an error when EOF arrives
/// *between* a valid header line and the blank-line terminator
/// (i.e. we parsed headers but the input was cut short). This
/// specifically hits the `if n == 0 { return Err(...) }` path for
/// the second iteration of the header loop.
#[tokio::test]
async fn test_read_jsonrpc_eof_after_first_header() {
    // Provide exactly one header line with \r\n, then EOF — no blank line
    let partial = b"Content-Length: 5\r\n";
    let mut cursor = &partial[..];
    let mut reader = BufReader::new(&mut cursor);
    let result = read_jsonrpc_message(&mut reader).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("unexpected EOF"),
        "expected 'unexpected EOF', got: {err}"
    );
}

/// `send_jsonrpc_message` on a serde-Value that cannot be serialized
/// would error at the `serde_json::to_string` step. Since any
/// `serde_json::Value` is always serializable, we document this by
/// exercising a complex nested Value successfully — locking the
/// happy path for `serde_json::to_string`.
#[tokio::test]
async fn test_send_jsonrpc_serializes_nested_values() {
    let msg = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "complex",
        "params": {
            "nested": {
                "arr": [1, 2, 3, {"deep": true}],
                "null_value": null,
                "unicode": "αβγ"
            }
        }
    });
    let mut buf: Vec<u8> = Vec::new();
    send_jsonrpc_message(&mut buf, &msg).await.unwrap();

    // Read it back to verify round-trip
    let mut cursor = &buf[..];
    let mut reader = BufReader::new(&mut cursor);
    let decoded = read_jsonrpc_message(&mut reader).await.unwrap();
    assert_eq!(decoded["params"]["nested"]["unicode"], "αβγ");
    assert_eq!(decoded["params"]["nested"]["arr"][3]["deep"], true);
}
