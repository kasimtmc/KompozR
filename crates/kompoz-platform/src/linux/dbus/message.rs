//! D-Bus messages: method calls, their returns and errors, signals; their header (the fixed part, then its fields:
//! path, interface, member, error name, reply serial, destination, sender, signature) and their body of values, as
//! they go over the wire.

use super::Error;
use super::signature::{Type, signature_of};
use super::value::{Decoder, Encoder, Value, is_valid_object_path};

/// The longest message (the specification's 128 MiB).
const MAX_MESSAGE: usize = 128 << 20;

/// The flag of a call no reply is wanted for.
pub const NO_REPLY_EXPECTED: u8 = 0x1;
/// The flag of a call that mustn't start the service it is for.
pub const NO_AUTO_START: u8 = 0x2;
/// The flag of a call that may ask the user to authorize it.
pub const ALLOW_INTERACTIVE_AUTHORIZATION: u8 = 0x4;

/// What a message is.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum MessageType {
    MethodCall = 1,
    MethodReturn = 2,
    Error = 3,
    Signal = 4,
}

/// A message: its header's fields and its body.
#[derive(Clone, PartialEq, Debug)]
pub struct Message {
    pub kind: MessageType,
    pub flags: u8,
    /// Its serial, set when it is sent (or as it came).
    pub serial: u32,
    pub path: Option<String>,
    pub interface: Option<String>,
    pub member: Option<String>,
    pub error_name: Option<String>,
    /// The serial of the call it answers.
    pub reply_serial: Option<u32>,
    pub destination: Option<String>,
    /// Who sent it (the bus sets it).
    pub sender: Option<String>,
    pub body: Vec<Value>,
}

impl Message {
    fn new(kind: MessageType) -> Message {
        Message {
            kind,
            flags: 0,
            serial: 0,
            path: None,
            interface: None,
            member: None,
            error_name: None,
            reply_serial: None,
            destination: None,
            sender: None,
            body: Vec::new(),
        }
    }

    /// A call of `interface`'s `member` on the object at `path` of `destination` (none: a peer's own).
    pub fn method_call(destination: Option<&str>, path: &str, interface: Option<&str>, member: &str) -> Message {
        Message {
            destination: destination.map(str::to_string),
            path: Some(path.to_string()),
            interface: interface.map(str::to_string),
            member: Some(member.to_string()),
            ..Message::new(MessageType::MethodCall)
        }
    }

    /// A signal: `interface`'s `member` from the object at `path`.
    pub fn signal(path: &str, interface: &str, member: &str) -> Message {
        Message {
            path: Some(path.to_string()),
            interface: Some(interface.to_string()),
            member: Some(member.to_string()),
            ..Message::new(MessageType::Signal)
        }
    }

    /// The return of `call`, to its sender.
    pub fn method_return(call: &Message) -> Message {
        Message {
            reply_serial: Some(call.serial),
            destination: call.sender.clone(),
            ..Message::new(MessageType::MethodReturn)
        }
    }

    /// The error `name` (and its text) for `call`, to its sender.
    pub fn error(call: &Message, name: &str, text: &str) -> Message {
        Message {
            error_name: Some(name.to_string()),
            reply_serial: Some(call.serial),
            destination: call.sender.clone(),
            body: vec![Value::str(text)],
            ..Message::new(MessageType::Error)
        }
    }

    /// With these arguments.
    pub fn with_body(mut self, body: Vec<Value>) -> Message {
        self.body = body;
        self
    }

    /// With `flags` too.
    pub fn with_flags(mut self, flags: u8) -> Message {
        self.flags |= flags;
        self
    }

    /// Its body's signature.
    pub fn signature(&self) -> String {
        signature_of(&self.body.iter().map(Value::type_of).collect::<Vec<_>>())
    }

    /// Its `i`th argument.
    pub fn arg(&self, i: usize) -> Option<&Value> {
        self.body.get(i)
    }

    /// Whether it is the signal `interface`'s `member`.
    pub fn is_signal(&self, interface: &str, member: &str) -> bool {
        self.kind == MessageType::Signal
            && self.interface.as_deref() == Some(interface)
            && self.member.as_deref() == Some(member)
    }

    /// Whether it is a call that wants no reply.
    pub fn no_reply(&self) -> bool {
        self.flags & NO_REPLY_EXPECTED != 0
    }

    /// An error's text (its first argument, if a string).
    pub fn error_text(&self) -> String {
        self.arg(0).and_then(Value::as_str).unwrap_or_default().to_string()
    }

    /// Its bytes, sent as `serial` (little endian).
    pub fn encode(&self, serial: u32) -> Result<Vec<u8>, Error> {
        let missing = |what: &str| Error::Protocol(format!("a {:?} without {what}", self.kind));
        match self.kind {
            MessageType::MethodCall if self.path.is_none() || self.member.is_none() => {
                return Err(missing("a path and a member"));
            }
            MessageType::Signal if self.path.is_none() || self.interface.is_none() || self.member.is_none() => {
                return Err(missing("a path, an interface and a member"));
            }
            MessageType::Error if self.error_name.is_none() || self.reply_serial.is_none() => {
                return Err(missing("an error name and a reply serial"));
            }
            MessageType::MethodReturn if self.reply_serial.is_none() => return Err(missing("a reply serial")),
            _ => {}
        }
        if let Some(p) = &self.path
            && !is_valid_object_path(p)
        {
            return Err(Error::Protocol(format!("not an object path: \"{p}\"")));
        }
        if serial == 0 {
            return Err(Error::Protocol("a serial of 0".into()));
        }
        let mut body = Encoder::new();
        for v in &self.body {
            body.value(v)?;
        }
        let signature = self.signature();
        let mut fields = Vec::new();
        let mut field = |code: u8, v: Value| fields.push(Value::Struct(vec![Value::Byte(code), Value::variant(v)]));
        if let Some(p) = &self.path {
            field(1, Value::path(p.as_str()));
        }
        for (code, s) in [(2, &self.interface), (3, &self.member), (4, &self.error_name)] {
            if let Some(s) = s {
                field(code, Value::str(s.as_str()));
            }
        }
        if let Some(r) = self.reply_serial {
            field(5, Value::Uint32(r));
        }
        for (code, s) in [(6, &self.destination), (7, &self.sender)] {
            if let Some(s) = s {
                field(code, Value::str(s.as_str()));
            }
        }
        if !signature.is_empty() {
            field(8, Value::Signature(signature));
        }
        let mut header = Encoder::new();
        header.buf.extend_from_slice(&[b'l', self.kind as u8, self.flags, 1]);
        header.buf.extend_from_slice(&(body.buf.len() as u32).to_le_bytes());
        header.buf.extend_from_slice(&serial.to_le_bytes());
        header.value(&Value::array(Type::Struct(vec![Type::Byte, Type::Variant]), fields))?;
        let mut bytes = header.buf;
        bytes.resize(bytes.len().next_multiple_of(8), 0);
        bytes.extend_from_slice(&body.buf);
        if bytes.len() > MAX_MESSAGE {
            return Err(Error::Protocol("a message over 128 MiB".into()));
        }
        Ok(bytes)
    }

    /// How long the message `bytes` begin with is, once its fixed header (16 bytes) is there; an error if it can't
    /// be one.
    pub fn length(bytes: &[u8]) -> Option<Result<usize, Error>> {
        if bytes.len() < 16 {
            return None;
        }
        let big = match bytes[0] {
            b'l' => false,
            b'B' => true,
            _ => return Some(Err(Error::Protocol("a message of neither endianness".into()))),
        };
        let u32_at = |at: usize| {
            let b: [u8; 4] = bytes[at..at + 4].try_into().expect("4 bytes");
            (if big { u32::from_be_bytes(b) } else { u32::from_le_bytes(b) }) as usize
        };
        let (body, fields) = (u32_at(4), u32_at(12));
        let total = (16 + fields).next_multiple_of(8).saturating_add(body);
        if total > MAX_MESSAGE {
            return Some(Err(Error::Protocol(format!("a message of {total} bytes"))));
        }
        Some(Ok(total))
    }

    /// The message `bytes` are (all of them, as [`length`](Message::length) measured).
    pub fn decode(bytes: &[u8]) -> Result<Message, Error> {
        let bad = |why: &str| Error::Protocol(why.to_string());
        let total = Message::length(bytes).ok_or_else(|| bad("a message cut short"))??;
        if bytes.len() != total {
            return Err(bad("a message of another length than its header says"));
        }
        let big = bytes[0] == b'B';
        let kind = match bytes[1] {
            1 => MessageType::MethodCall,
            2 => MessageType::MethodReturn,
            3 => MessageType::Error,
            4 => MessageType::Signal,
            other => return Err(Error::Protocol(format!("a message of type {other}"))),
        };
        if bytes[3] != 1 {
            return Err(Error::Protocol(format!("a message of protocol version {}", bytes[3])));
        }
        let mut header = Decoder::new(bytes, 8, big);
        let serial = header.u32()?;
        if serial == 0 {
            return Err(bad("a serial of 0"));
        }
        let fields = header.value(&Type::Array(Box::new(Type::Struct(vec![Type::Byte, Type::Variant]))))?;
        header.align(8)?;
        let body_start = header.pos;
        let mut m = Message { serial, flags: bytes[2], ..Message::new(kind) };
        let mut signature = String::new();
        let mut unix_fds = 0;
        for field in fields.items().unwrap_or_default() {
            let (Some(Value::Byte(code)), Some(Value::Variant(v))) =
                (field.fields().and_then(|f| f.first()), field.fields().and_then(|f| f.get(1)))
            else {
                continue;
            };
            let wrong = || Error::Protocol(format!("header field {code} of the wrong type"));
            let text = || v.as_str().map(str::to_string).ok_or_else(wrong);
            match (code, &**v) {
                (1, Value::ObjectPath(p)) => m.path = Some(p.clone()),
                (2, _) => m.interface = Some(text()?),
                (3, _) => m.member = Some(text()?),
                (4, _) => m.error_name = Some(text()?),
                (5, Value::Uint32(r)) => m.reply_serial = Some(*r),
                (6, _) => m.destination = Some(text()?),
                (7, _) => m.sender = Some(text()?),
                (8, Value::Signature(s)) => signature = s.clone(),
                (9, Value::Uint32(n)) => unix_fds = *n,
                (1 | 5 | 8 | 9, _) => return Err(wrong()),
                _ => {} // (unknown fields are ignored, as the specification asks)
            }
        }
        if unix_fds > 0 {
            return Err(bad("a message carrying file descriptors (never negotiated)"));
        }
        let ok = match kind {
            MessageType::MethodCall => m.path.is_some() && m.member.is_some(),
            MessageType::Signal => m.path.is_some() && m.interface.is_some() && m.member.is_some(),
            MessageType::Error => m.error_name.is_some() && m.reply_serial.is_some(),
            MessageType::MethodReturn => m.reply_serial.is_some(),
        };
        if !ok {
            return Err(Error::Protocol(format!("a {kind:?} without its required header fields")));
        }
        let mut body = Decoder::new(&bytes[body_start..], 0, big);
        for t in Type::parse(&signature)? {
            m.body.push(body.value(&t)?);
        }
        if body_start + body.pos != total {
            return Err(bad("a body longer than its signature says"));
        }
        Ok(m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_round_trip() {
        let call = Message::method_call(
            Some("org.freedesktop.portal.Desktop"),
            "/org/freedesktop/portal/desktop",
            Some("org.freedesktop.portal.Settings"),
            "ReadOne",
        )
        .with_body(vec![Value::str("org.freedesktop.appearance"), Value::str("accent-color")])
        .with_flags(NO_AUTO_START);
        let bytes = call.encode(7).unwrap();
        assert_eq!(Message::length(&bytes).unwrap().unwrap(), bytes.len());
        let back = Message::decode(&bytes).unwrap();
        assert_eq!(back, Message { serial: 7, ..call.clone() });
        assert_eq!(back.signature(), "ss");

        let mut asked = back.clone();
        asked.sender = Some(":1.4".into());
        let reply = Message::method_return(&asked).with_body(vec![Value::variant(Value::Uint32(1))]);
        let r = Message::decode(&reply.encode(9).unwrap()).unwrap();
        assert_eq!(
            (r.kind, r.reply_serial, r.destination.as_deref()),
            (MessageType::MethodReturn, Some(7), Some(":1.4"))
        );
        let e = Message::decode(&Message::error(&asked, "org.x.Error.No", "no").encode(10).unwrap()).unwrap();
        assert_eq!((e.error_name.as_deref(), e.error_text().as_str()), (Some("org.x.Error.No"), "no"));
        let s = Message::signal("/org/a11y/bus", "org.freedesktop.DBus.Properties", "PropertiesChanged")
            .with_body(vec![Value::str("org.a11y.Status"), Value::empty_properties()]);
        let s = Message::decode(&s.encode(11).unwrap()).unwrap();
        assert!(s.is_signal("org.freedesktop.DBus.Properties", "PropertiesChanged"));
        assert_eq!(s.body.len(), 2);
    }

    #[test]
    fn big_endian_messages_are_read() {
        // A signal as a big-endian peer would send it: path, interface, member, signature "s", body "hi".
        let mut fields = Vec::new();
        let mut field = |code: u8, sig: u8, text: &str| {
            while fields.len() % 8 != 0 {
                fields.push(0);
            }
            fields.extend_from_slice(&[code, 1, sig, 0]);
            if sig == b'g' {
                fields.push(text.len() as u8);
            } else {
                fields.extend_from_slice(&(text.len() as u32).to_be_bytes());
            }
            fields.extend_from_slice(text.as_bytes());
            fields.push(0);
        };
        field(1, b'o', "/a");
        field(2, b's', "org.x");
        field(3, b's', "Y");
        field(8, b'g', "s");
        let mut bytes = vec![b'B', 4, 0, 1];
        bytes.extend_from_slice(&7u32.to_be_bytes()); // body: u32 + "hi" + NUL
        bytes.extend_from_slice(&3u32.to_be_bytes());
        bytes.extend_from_slice(&(fields.len() as u32).to_be_bytes());
        bytes.extend_from_slice(&fields);
        while bytes.len() % 8 != 0 {
            bytes.push(0);
        }
        bytes.extend_from_slice(&2u32.to_be_bytes());
        bytes.extend_from_slice(b"hi\0");
        let m = Message::decode(&bytes).unwrap();
        assert!(m.is_signal("org.x", "Y"));
        assert_eq!((m.serial, m.path.as_deref(), m.body.as_slice()), (3, Some("/a"), &[Value::str("hi")][..]));
    }

    #[test]
    fn bad_messages_are_refused() {
        let call = Message::method_call(None, "/", None, "Ping");
        assert!(call.encode(0).is_err());
        assert!(Message::method_call(None, "no-slash", None, "Ping").encode(1).is_err());
        assert!(Message { member: None, ..call.clone() }.encode(1).is_err());
        let bytes = call.encode(1).unwrap();
        assert!(Message::length(&bytes[..15]).is_none());
        let mut endian = bytes.clone();
        endian[0] = b'x';
        assert!(Message::decode(&endian).is_err());
        let mut version = bytes.clone();
        version[3] = 2;
        assert!(Message::decode(&version).is_err());
        let mut kind = bytes.clone();
        kind[1] = 9;
        assert!(Message::decode(&kind).is_err());
        assert!(Message::decode(&bytes[..bytes.len() - 1]).is_err());
        // A return without its reply serial.
        let mut no_reply = bytes.clone();
        no_reply[1] = 2;
        assert!(Message::decode(&no_reply).is_err());
    }
}
