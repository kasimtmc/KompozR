//! LSP's base protocol: messages framed by a `Content-Length` header, read from a stream and written by a thread of
//! their own — whoever sends never waits on a pipe the other side isn't reading yet (the editor and rust-analyzer
//! each write while they read).

use crate::json::Json;
use std::io::{BufRead, Write};
use std::sync::mpsc;

/// The next message of `input`: `Ok(None)` at its end; a body that isn't JSON is skipped.
pub fn read(input: &mut impl BufRead) -> std::io::Result<Option<Json>> {
    loop {
        let mut length: Option<usize> = None;
        let mut line = String::new();
        loop {
            line.clear();
            if input.read_line(&mut line)? == 0 {
                return Ok(None);
            }
            let header = line.trim_end_matches(['\r', '\n']);
            if header.is_empty() {
                if length.is_some() {
                    break;
                }
                continue;
            }
            if let Some((name, value)) = header.split_once(':')
                && name.trim().eq_ignore_ascii_case("content-length")
            {
                length = value.trim().parse().ok();
            }
        }
        let mut body = vec![0; length.unwrap_or(0)];
        input.read_exact(&mut body)?;
        if let Ok(text) = std::str::from_utf8(&body)
            && let Ok(message) = Json::parse(text)
        {
            return Ok(Some(message));
        }
    }
}

/// `message` framed.
pub fn frame(message: &Json) -> Vec<u8> {
    let body = message.dump();
    let mut out = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
    out.extend_from_slice(body.as_bytes());
    out
}

/// A sender of messages to `output`, written in order by a thread of its own (until the sender and its clones are
/// gone, or the stream fails).
pub fn writer(mut output: impl Write + Send + 'static) -> mpsc::Sender<Json> {
    let (tx, rx) = mpsc::channel::<Json>();
    std::thread::spawn(move || {
        for message in rx {
            if output.write_all(&frame(&message)).and_then(|_| output.flush()).is_err() {
                return;
            }
        }
    });
    tx
}

/// A request.
pub fn request(id: impl Into<Json>, method: &str, params: Json) -> Json {
    Json::object([("jsonrpc", "2.0".into()), ("id", id.into()), ("method", method.into()), ("params", params)])
}

/// A notification.
pub fn notification(method: &str, params: Json) -> Json {
    Json::object([("jsonrpc", "2.0".into()), ("method", method.into()), ("params", params)])
}

/// A response with a result.
pub fn response(id: Json, result: Json) -> Json {
    Json::object([("jsonrpc", "2.0".into()), ("id", id), ("result", result)])
}

/// A response with an error.
pub fn error(id: Json, code: i64, message: &str) -> Json {
    Json::object([
        ("jsonrpc", "2.0".into()),
        ("id", id),
        ("error", Json::object([("code", code.into()), ("message", message.into())])),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framed_and_read_back() {
        let a = notification("x", Json::object([("ü", 1.into())]));
        let b = request(7, "y", Json::Null);
        let mut bytes = frame(&a);
        bytes.extend(b"Content-Type: whatever\r\n");
        bytes.extend(frame(&b));
        let mut input = std::io::BufReader::new(&bytes[..]);
        assert_eq!(read(&mut input).unwrap(), Some(a));
        assert_eq!(read(&mut input).unwrap(), Some(b));
        assert_eq!(read(&mut input).unwrap(), None);
    }
}
