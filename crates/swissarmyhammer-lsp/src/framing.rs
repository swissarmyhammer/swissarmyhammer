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

/// The largest body length, in bytes, that a reader accepts.
///
/// A larger Content-Length is not trusted. No reader makes a buffer of that
/// size or waits for that many bytes.
pub(crate) const MAX_CONTENT_LENGTH: usize = 64 * 1024 * 1024;

/// The largest header block, in bytes, that a reader accepts.
///
/// A header block that has no blank line within this many bytes is not
/// trusted, so a reader does not keep or read bytes for it with no limit.
pub(crate) const MAX_HEADER_BLOCK_BYTES: usize = 64 * 1024;

/// What one line of a header block is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HeaderLine<'a> {
    /// The blank line that ends the header block.
    End,
    /// A Content-Length field, with the text of its value.
    ContentLength(&'a str),
    /// A header field other than Content-Length. It is ignored.
    Field,
}

/// Find what one header line is.
fn parse_header_line(line: &str) -> HeaderLine<'_> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return HeaderLine::End;
    }
    if let Some(value) = trimmed.strip_prefix(CONTENT_LENGTH_HEADER) {
        return HeaderLine::ContentLength(value);
    }
    HeaderLine::Field
}

/// Parse the value of a Content-Length field.
///
/// The error is the reason that the value is not trusted: it is not a number
/// of bytes, or it is larger than [`MAX_CONTENT_LENGTH`].
fn parse_content_length(value: &str) -> Result<usize, String> {
    let length = value
        .trim()
        .parse::<usize>()
        .map_err(|e| format!("bad Content-Length: {e}"))?;
    if length > MAX_CONTENT_LENGTH {
        return Err(format!(
            "bad Content-Length: {length} is larger than the maximum of {MAX_CONTENT_LENGTH} bytes"
        ));
    }
    Ok(length)
}

/// The body length that a complete header block with no fault gave.
///
/// A block with no Content-Length gives an error that a reader can recover
/// from: the block is removed, and the next read starts after it.
fn body_length(content_length: Option<usize>) -> Result<usize, LspError> {
    content_length.ok_or_else(|| LspError::JsonRpc("missing Content-Length header".into()))
}

/// The error for a header block that is larger than [`MAX_HEADER_BLOCK_BYTES`].
fn header_block_too_large() -> LspError {
    LspError::OutOfStep(format!(
        "header block is larger than the maximum of {MAX_HEADER_BLOCK_BYTES} bytes"
    ))
}

/// A header block that a reader takes in one line at a time.
///
/// This is the one parse rule for a header block, and the sync frame buffer
/// and the async daemon reader both use it. Each line is parsed, and the block
/// ends at the blank line. The block has one of three Content-Length states:
///
/// - missing: no Content-Length field came. The result is an error that the
///   reader can recover from, and the next read starts after the block.
/// - trusted: the field gave a body length. The result is that length.
/// - not trusted: a line of the block is not UTF-8, or a Content-Length value
///   is bad, negative, or larger than [`MAX_CONTENT_LENGTH`]. The result is an
///   [`LspError::OutOfStep`] error, because the length of the body is not
///   known. The reader must not read a body or a next frame after it.
///
/// A block that is larger than [`MAX_HEADER_BLOCK_BYTES`] also gives an
/// [`LspError::OutOfStep`] error, at the line that makes it too large.
#[derive(Debug, Default)]
pub(crate) struct HeaderBlock {
    /// The body length that a Content-Length field gave.
    content_length: Option<usize>,
    /// The reason that the block is not trusted, from its first bad line.
    fault: Option<String>,
    /// The number of bytes of the block that the reader took in.
    bytes_seen: usize,
}

impl HeaderBlock {
    /// Take in one header line, with or without its line ending.
    ///
    /// The result is `None` while the block continues. At the blank line that
    /// ends the block, the result is the body length or an error (see
    /// [`HeaderBlock`]). A line that makes the block too large gives its
    /// error at once.
    pub(crate) fn feed(&mut self, line: &[u8]) -> Option<Result<usize, LspError>> {
        if let Err(e) = self.count_bytes(line.len()) {
            return Some(Err(e));
        }
        let Ok(line) = std::str::from_utf8(line) else {
            self.note_fault("a header line is not UTF-8".into());
            return None;
        };
        match parse_header_line(line) {
            HeaderLine::End => Some(self.finish()),
            HeaderLine::ContentLength(value) => {
                match parse_content_length(value) {
                    Ok(length) => self.content_length = Some(length),
                    Err(reason) => self.note_fault(reason),
                }
                None
            }
            HeaderLine::Field => None,
        }
    }

    /// The most bytes that the next line can have, plus one.
    ///
    /// A reader that reads a line through this limit reads no more bytes than
    /// the block can take. A line that fills the limit makes the block too
    /// large, so [`feed`](Self::feed) gives the error for it.
    pub(crate) fn line_limit(&self) -> usize {
        MAX_HEADER_BLOCK_BYTES.saturating_sub(self.bytes_seen) + 1
    }

    /// Count `len` more bytes of the block. The error is the fatal error for a
    /// block that is now larger than [`MAX_HEADER_BLOCK_BYTES`].
    fn count_bytes(&mut self, len: usize) -> Result<(), LspError> {
        self.bytes_seen = self.bytes_seen.saturating_add(len);
        if self.bytes_seen > MAX_HEADER_BLOCK_BYTES {
            return Err(header_block_too_large());
        }
        Ok(())
    }

    /// Keep `reason` as the fault of the block, if the block has no fault
    /// yet. The first bad line gives the reason.
    fn note_fault(&mut self, reason: String) {
        self.fault.get_or_insert(reason);
    }

    /// The result of the block at its blank line.
    fn finish(&mut self) -> Result<usize, LspError> {
        match self.fault.take() {
            Some(reason) => Err(LspError::OutOfStep(reason)),
            None => body_length(self.content_length),
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
///
/// After an [`LspError::OutOfStep`] error the buffer is out of step for all
/// time: it drops its bytes, keeps no bytes that come later, and gives the
/// same error on each later call. The owner of the connection must then stop
/// it.
#[derive(Debug, Default)]
pub(crate) struct FrameBuffer {
    /// The unread bytes, in the order the server sent them.
    bytes: Vec<u8>,
    /// The reason of the fatal error, after the buffer went out of step.
    out_of_step: Option<String>,
}

impl FrameBuffer {
    /// Add bytes that the server sent. An out-of-step buffer drops them.
    pub(crate) fn push(&mut self, data: &[u8]) {
        if self.out_of_step.is_none() {
            self.bytes.extend_from_slice(data);
        }
    }

    /// Tell if a fatal error put the buffer out of step.
    pub(crate) fn is_out_of_step(&self) -> bool {
        self.out_of_step.is_some()
    }

    /// Take the first complete message out of the buffer.
    ///
    /// The result is `Ok(None)` while the buffer holds no complete frame. A
    /// frame with a bad body, or a header block with no Content-Length, is
    /// removed from the buffer, and its error is the result, so the next call
    /// reads the next frame. A header block that is not trusted (see
    /// [`HeaderBlock`]) puts the buffer out of step.
    pub(crate) fn take_message(&mut self) -> Result<Option<Value>, LspError> {
        if let Some(reason) = &self.out_of_step {
            return Err(LspError::OutOfStep(reason.clone()));
        }
        let Some((header_len, length)) = self.scan_headers() else {
            return Ok(None);
        };
        let length = match length {
            Ok(length) => length,
            Err(e) => return Err(self.drop_header_block(header_len, e)),
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
        if let Some(reason) = &self.out_of_step {
            return LspError::OutOfStep(reason.clone());
        }
        match self.scan_headers() {
            Some(_) => LspError::JsonRpc("read body: unexpected EOF".into()),
            None => eof_in_headers(),
        }
    }

    /// Remove a header block of `header_len` bytes whose result is `error`,
    /// and give the error back.
    ///
    /// A fatal error puts the buffer out of step, and the buffer drops all of
    /// its bytes, because the position of the next frame is not known.
    fn drop_header_block(&mut self, header_len: usize, error: LspError) -> LspError {
        match &error {
            LspError::OutOfStep(reason) => {
                self.out_of_step = Some(reason.clone());
                self.bytes = Vec::new();
            }
            _ => {
                self.bytes.drain(..header_len);
            }
        }
        error
    }

    /// Find the end of the first header block.
    ///
    /// Give the byte length of the block and its body length, or `None` while
    /// the block is not complete. A bad header line makes the body length an
    /// error, and the block length still covers the whole block (see
    /// [`HeaderBlock`]). When the buffer holds more bytes than a header block
    /// can have and no end of the block, the result is the fatal error for a
    /// block that is too large.
    fn scan_headers(&self) -> Option<(usize, Result<usize, LspError>)> {
        let mut block = HeaderBlock::default();
        let mut start = 0;
        while let Some(offset) = self.bytes[start..].iter().position(|&b| b == b'\n') {
            let end = start + offset + 1;
            if let Some(length) = block.feed(&self.bytes[start..end]) {
                return Some((end, length));
            }
            start = end;
        }
        match block.count_bytes(self.bytes.len() - start) {
            Ok(()) => None,
            Err(e) => Some((self.bytes.len(), Err(e))),
        }
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

        let first = block.feed(b"Content-Length: abc\r\n");
        let second = block.feed(b"Content-Length:\r\n");
        let end = block.feed(b"\r\n").expect("the blank line ends the block");

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

    /// Feed each line of `lines` to a new header block, and give the result
    /// of the line that ends the block.
    fn block_result(lines: &[&[u8]]) -> Result<usize, LspError> {
        let mut block = HeaderBlock::default();
        lines
            .iter()
            .find_map(|line| block.feed(line))
            .expect("a line ends the block")
    }

    /// Assert that `result` is a fatal error whose text contains `expected`.
    #[track_caller]
    fn assert_out_of_step<T: std::fmt::Debug>(result: Result<T, LspError>, expected: &str) {
        match result {
            Err(LspError::OutOfStep(reason)) => assert!(
                reason.contains(expected),
                "expected {expected:?} in the reason, got: {reason}"
            ),
            other => panic!("expected an out-of-step error, got: {other:?}"),
        }
    }

    #[test]
    fn a_content_length_larger_than_the_maximum_is_out_of_step() {
        let result = block_result(&[b"Content-Length: 18446744073709551615\r\n", b"\r\n"]);

        assert_out_of_step(result, "larger than the maximum");
    }

    #[test]
    fn a_content_length_one_byte_larger_than_the_maximum_is_out_of_step() {
        let line = format!("Content-Length: {}\r\n", MAX_CONTENT_LENGTH + 1);

        let result = block_result(&[line.as_bytes(), b"\r\n"]);

        assert_out_of_step(result, "larger than the maximum");
    }

    #[test]
    fn a_content_length_at_the_maximum_is_the_body_length() {
        let line = format!("Content-Length: {MAX_CONTENT_LENGTH}\r\n");

        let result = block_result(&[line.as_bytes(), b"\r\n"]);

        assert_eq!(result.expect("the maximum is allowed"), MAX_CONTENT_LENGTH);
    }

    #[test]
    fn a_negative_content_length_is_out_of_step() {
        let result = block_result(&[b"Content-Length: -5\r\n", b"\r\n"]);

        assert_out_of_step(result, "bad Content-Length");
    }

    #[test]
    fn a_header_line_that_is_not_utf8_is_out_of_step() {
        let result = block_result(&[b"Content-Length: 2\r\n", b"X-Name: \xff\xfe\r\n", b"\r\n"]);

        assert_out_of_step(result, "not UTF-8");
    }

    #[test]
    fn a_header_block_larger_than_the_maximum_is_out_of_step_before_its_end() {
        let field = b"X-Padding: 0123456789\r\n";
        let mut block = HeaderBlock::default();

        let result = std::iter::repeat_n(&field[..], MAX_HEADER_BLOCK_BYTES / field.len() + 1)
            .find_map(|line| block.feed(line))
            .expect("the block stops at the maximum");

        assert_out_of_step(result, "header block");
    }

    #[test]
    fn an_out_of_step_frame_buffer_drops_its_bytes_and_keeps_the_error() {
        let mut frames = FrameBuffer::default();
        frames.push(b"Content-Length: 18446744073709551615\r\n\r\n{}");

        let first = frames.take_message();
        frames.push(b"Content-Length: 2\r\n\r\n{}");
        let second = frames.take_message();

        assert_out_of_step(first, "larger than the maximum");
        assert_out_of_step(second, "larger than the maximum");
        assert!(frames.is_out_of_step());
        assert!(frames.bytes.is_empty());
    }

    #[test]
    fn a_frame_buffer_drops_the_bytes_after_a_header_line_that_is_not_utf8() {
        let mut frames = FrameBuffer::default();
        frames.push(b"Content-Length: 2\r\nX-Name: \xff\r\n\r\n{}");

        let first = frames.take_message();
        frames.push(b"more bytes from the server");

        assert_out_of_step(first, "not UTF-8");
        assert!(frames.bytes.is_empty());
    }

    #[test]
    fn a_partial_header_line_larger_than_the_maximum_is_out_of_step() {
        let mut frames = FrameBuffer::default();
        frames.push(&vec![b'a'; MAX_HEADER_BLOCK_BYTES + 1]);

        let result = frames.take_message();

        assert_out_of_step(result, "header block");
        assert!(frames.bytes.is_empty());
    }

    #[test]
    fn a_partial_header_line_at_the_maximum_waits_for_more_bytes() {
        let mut frames = FrameBuffer::default();
        frames.push(&vec![b'a'; MAX_HEADER_BLOCK_BYTES]);

        let result = frames.take_message();

        assert_eq!(result.expect("no error yet"), None);
        assert!(!frames.is_out_of_step());
    }

    #[test]
    fn eof_after_an_out_of_step_error_gives_the_out_of_step_error() {
        let mut frames = FrameBuffer::default();
        frames.push(b"Content-Length: -5\r\n\r\n");
        let _ = frames.take_message();

        assert_out_of_step(Err::<(), _>(frames.eof_error()), "bad Content-Length");
    }
}
