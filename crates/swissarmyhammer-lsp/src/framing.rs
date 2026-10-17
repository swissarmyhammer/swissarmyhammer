//! Content-Length framing of JSON-RPC messages.
//!
//! The LSP wire carries each JSON-RPC message as a header block and a body:
//! `Content-Length: <bytes>\r\n\r\n<json>`. This module is the one place that
//! knows that format. The sync client ([`crate::client`]) and the async daemon
//! handshake ([`crate::daemon`]) both encode, parse and decode through it, so
//! the two cannot drift apart. Each side keeps only its own I/O calls, because
//! one writes to a blocking pipe and the other to a tokio stream.

use std::fmt::Display;
use std::io::Write;

use serde_json::Value;

use crate::error::LspError;

/// The header that gives the byte length of the body.
const CONTENT_LENGTH_HEADER: &str = "Content-Length:";

/// One JSON-RPC message, encoded for the wire.
pub(crate) struct Frame {
    /// The header block, with the blank line that ends it.
    pub(crate) header: String,
    /// The JSON body.
    pub(crate) body: String,
}

/// Encode `message` as a frame.
pub(crate) fn encode(message: &Value) -> Result<Frame, LspError> {
    let body = serde_json::to_string(message)
        .map_err(|e| LspError::JsonRpc(format!("json encode: {e}")))?;
    let header = format!("{CONTENT_LENGTH_HEADER} {}\r\n\r\n", body.len());
    Ok(Frame { header, body })
}

/// The step of a frame write. Each step names its own failure, so the sync and
/// the async writer report a failure in the same words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WriteStep {
    /// The write of the header block.
    Header,
    /// The write of the body.
    Body,
    /// The flush after the body.
    Flush,
}

impl WriteStep {
    /// The error for a failure of this step.
    pub(crate) fn error(self, cause: impl Display) -> LspError {
        let step = match self {
            WriteStep::Header => "write header",
            WriteStep::Body => "write body",
            WriteStep::Flush => "flush",
        };
        LspError::JsonRpc(format!("{step}: {cause}"))
    }
}

/// Write `message` as one frame to a blocking writer, and flush it.
pub(crate) fn write_frame<W: Write>(writer: &mut W, message: &Value) -> Result<(), LspError> {
    let frame = encode(message)?;
    writer
        .write_all(frame.header.as_bytes())
        .map_err(|e| WriteStep::Header.error(e))?;
    writer
        .write_all(frame.body.as_bytes())
        .map_err(|e| WriteStep::Body.error(e))?;
    writer.flush().map_err(|e| WriteStep::Flush.error(e))
}

/// What one line of a header block is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HeaderLine {
    /// The blank line that ends the header block.
    End,
    /// A header field. A field other than Content-Length is ignored.
    Field,
}

/// Parse one header line. A Content-Length field sets `content_length`.
fn parse_header_line(
    line: &str,
    content_length: &mut Option<usize>,
) -> Result<HeaderLine, LspError> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Ok(HeaderLine::End);
    }
    if let Some(value) = trimmed.strip_prefix(CONTENT_LENGTH_HEADER) {
        let length = value
            .trim()
            .parse::<usize>()
            .map_err(|e| LspError::JsonRpc(format!("bad Content-Length: {e}")))?;
        *content_length = Some(length);
    }
    Ok(HeaderLine::Field)
}

/// The body length that a complete header block gave.
fn body_length(content_length: Option<usize>) -> Result<usize, LspError> {
    content_length.ok_or_else(|| LspError::JsonRpc("missing Content-Length header".into()))
}

/// A header block that a reader takes in one line at a time.
///
/// This is the one parse rule for a header block, and the sync frame buffer
/// and the async daemon reader both use it. Each line is parsed. The first bad
/// line is kept as the error, and the lines after it are still read. The block
/// ends only at the blank line, so a bad block is removed from the stream as
/// one unit and the next read starts at the next frame.
#[derive(Debug, Default)]
pub(crate) struct HeaderBlock {
    /// The body length that a Content-Length field gave.
    content_length: Option<usize>,
    /// The error of the first bad line in the block.
    first_error: Option<LspError>,
}

impl HeaderBlock {
    /// Take in one header line, with or without its line ending.
    ///
    /// The result is `None` while the block continues. At the blank line that
    /// ends the block, the result is the body length, or the error of the
    /// first bad line.
    pub(crate) fn feed(&mut self, line: &str) -> Option<Result<usize, LspError>> {
        match parse_header_line(line, &mut self.content_length) {
            Ok(HeaderLine::End) => Some(match self.first_error.take() {
                Some(e) => Err(e),
                None => body_length(self.content_length),
            }),
            Ok(HeaderLine::Field) => None,
            Err(e) => {
                self.first_error.get_or_insert(e);
                None
            }
        }
    }
}

/// Decode the JSON body of a frame.
pub(crate) fn decode_body(body: &[u8]) -> Result<Value, LspError> {
    serde_json::from_slice(body).map_err(|e| LspError::JsonRpc(format!("json decode: {e}")))
}

/// The error for an end of stream before the header block is complete.
pub(crate) fn eof_in_headers() -> LspError {
    LspError::JsonRpc("unexpected EOF reading headers".into())
}

/// The bytes that the server sent and the client did not read as a message
/// yet.
///
/// The sync client reads raw chunks from the pipe into this buffer, and takes
/// each complete frame out of it. A partial frame stays in the buffer until
/// the rest of it comes, so a read that stops at a deadline in the middle of
/// a frame does not lose bytes or put the stream out of step.
#[derive(Debug, Default)]
pub(crate) struct FrameBuffer {
    /// The unread bytes, in the order the server sent them.
    bytes: Vec<u8>,
}

impl FrameBuffer {
    /// Add bytes that the server sent.
    pub(crate) fn push(&mut self, data: &[u8]) {
        self.bytes.extend_from_slice(data);
    }

    /// Take the first complete message out of the buffer.
    ///
    /// The result is `Ok(None)` while the buffer holds no complete frame. A
    /// frame with a bad header block or a bad body is removed from the
    /// buffer, and its error is the result, so the next call reads the next
    /// frame.
    pub(crate) fn take_message(&mut self) -> Result<Option<Value>, LspError> {
        let Some((header_len, length)) = self.scan_headers() else {
            return Ok(None);
        };
        let length = match length {
            Ok(length) => length,
            Err(e) => {
                self.bytes.drain(..header_len);
                return Err(e);
            }
        };
        let frame_len = header_len + length;
        if self.bytes.len() < frame_len {
            return Ok(None);
        }
        let message = decode_body(&self.bytes[header_len..frame_len]);
        self.bytes.drain(..frame_len);
        message.map(Some)
    }

    /// The error for an end of stream with the bytes that the buffer holds.
    pub(crate) fn eof_error(&self) -> LspError {
        match self.scan_headers() {
            Some(_) => LspError::JsonRpc("read body: unexpected EOF".into()),
            None => eof_in_headers(),
        }
    }

    /// Find the end of the first header block.
    ///
    /// Give the byte length of the block and its body length, or `None` while
    /// the block is not complete. A bad header line makes the body length an
    /// error, and the block length still covers the whole block (see
    /// [`HeaderBlock`]).
    fn scan_headers(&self) -> Option<(usize, Result<usize, LspError>)> {
        let mut block = HeaderBlock::default();
        let mut start = 0;
        while let Some(offset) = self.bytes[start..].iter().position(|&b| b == b'\n') {
            let end = start + offset + 1;
            let line = String::from_utf8_lossy(&self.bytes[start..end]);
            if let Some(length) = block.feed(&line) {
                return Some((end, length));
            }
            start = end;
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn encode_frames_the_body_with_its_byte_length() {
        let message = json!({"jsonrpc": "2.0", "method": "é"});

        let frame = encode(&message).expect("encode");

        let body = serde_json::to_string(&message).unwrap();
        assert_eq!(
            frame.header,
            format!("Content-Length: {}\r\n\r\n", body.len())
        );
        assert_eq!(frame.body, body);
    }

    #[test]
    fn write_frame_writes_the_header_then_the_body() {
        let message = json!({"jsonrpc": "2.0", "id": 1, "result": null});
        let mut bytes = Vec::new();

        write_frame(&mut bytes, &message).expect("write");

        let frame = encode(&message).unwrap();
        assert_eq!(
            bytes,
            format!("{}{}", frame.header, frame.body).into_bytes()
        );
    }

    #[test]
    fn a_partial_frame_gives_no_message_until_it_is_complete() {
        const CUT: usize = 12;
        let message = json!({"jsonrpc": "2.0", "id": 1, "result": "ok"});
        let mut bytes = Vec::new();
        write_frame(&mut bytes, &message).unwrap();
        let mut frames = FrameBuffer::default();

        frames.push(&bytes[..CUT]);
        let before = frames.take_message().expect("no error");
        frames.push(&bytes[CUT..]);
        let after = frames.take_message().expect("no error");

        assert_eq!(before, None);
        assert_eq!(after, Some(message));
        assert!(frames.bytes.is_empty());
    }

    #[test]
    fn two_frames_in_one_push_give_two_messages_in_order() {
        let first = json!({"jsonrpc": "2.0", "id": 1, "result": 1});
        let second = json!({"jsonrpc": "2.0", "id": 2, "result": 2});
        let mut bytes = Vec::new();
        write_frame(&mut bytes, &first).unwrap();
        write_frame(&mut bytes, &second).unwrap();
        let mut frames = FrameBuffer::default();

        frames.push(&bytes);

        assert_eq!(frames.take_message().unwrap(), Some(first));
        assert_eq!(frames.take_message().unwrap(), Some(second));
        assert_eq!(frames.take_message().unwrap(), None);
    }

    #[test]
    fn a_header_block_with_no_content_length_is_dropped_with_an_error() {
        let next = json!({"jsonrpc": "2.0", "id": 3, "result": 3});
        let mut frames = FrameBuffer::default();
        frames.push(b"Content-Type: text\r\n\r\n");
        let mut bytes = Vec::new();
        write_frame(&mut bytes, &next).unwrap();
        frames.push(&bytes);

        let error = frames.take_message().expect_err("missing Content-Length");

        assert!(error.to_string().contains("missing Content-Length header"));
        assert_eq!(frames.take_message().unwrap(), Some(next));
    }

    #[test]
    fn a_header_block_gives_the_first_error_only_at_the_blank_line() {
        let mut block = HeaderBlock::default();

        let first = block.feed("Content-Length: abc\r\n");
        let second = block.feed("Content-Length:\r\n");
        let end = block.feed("\r\n").expect("the blank line ends the block");

        assert!(first.is_none());
        assert!(second.is_none());
        let error = end.expect_err("bad Content-Length");
        assert!(error.to_string().contains("invalid digit"));
        assert!(!error.to_string().contains("empty string"));
    }

    #[test]
    fn a_body_that_is_not_json_is_dropped_with_an_error() {
        let mut frames = FrameBuffer::default();
        frames.push(b"Content-Length: 3\r\n\r\nabc");

        let error = frames.take_message().expect_err("bad JSON");

        assert!(error.to_string().contains("json decode"));
        assert!(frames.bytes.is_empty());
    }

    #[test]
    fn eof_in_the_headers_names_the_headers() {
        let mut frames = FrameBuffer::default();
        frames.push(b"Content-Len");

        assert!(frames
            .eof_error()
            .to_string()
            .contains("unexpected EOF reading headers"));
    }

    #[test]
    fn eof_in_the_body_names_the_body() {
        let mut frames = FrameBuffer::default();
        frames.push(b"Content-Length: 10\r\n\r\nabc");

        assert!(frames.eof_error().to_string().contains("read body"));
    }
}
