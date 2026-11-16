//! A connection to a D-Bus bus (C++ used libdbus's): a Unix socket, SASL's EXTERNAL authentication (the uid the
//! kernel vouches for), `Hello` (the connection's unique name), messages sent with serials of their own, calls
//! waited for with a timeout (what comes meanwhile kept, in order), signals subscribed to with match rules, calls
//! to objects served with replies and errors.
//!
//! One thread uses it (it is `Send`): the D-Bus threads of the portal and the AT-SPI bridge each have their own, and
//! wait on it and on their wake eventfd at once ([`Connection::read`]).

use super::address::{Address, parse_address, uid};
use super::message::{Message, MessageType};
use super::value::Value;
use super::{Error, errors};
use crate::linux::sys::{self, PollFd};
use std::collections::VecDeque;
use std::fmt;
use std::io::{self, Read, Write};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, RawFd};
use std::os::linux::net::SocketAddrExt;
use std::os::unix::net::{SocketAddr, UnixStream};
use std::time::{Duration, Instant};

/// How long the bus itself is waited for (`Hello`, `AddMatch`, `RequestName`), and the authentication.
pub const BUS_TIMEOUT: Duration = Duration::from_secs(5);

const BUS_NAME: &str = "org.freedesktop.DBus";
const BUS_PATH: &str = "/org/freedesktop/DBus";

/// What a wait for bytes ended with.
enum Filled {
    Read,
    TimedOut,
    Woken,
}

/// A connection to a bus.
pub struct Connection {
    stream: UnixStream,
    /// Bytes read that make no whole message yet.
    input: Vec<u8>,
    /// Messages read and not yet taken (those that came while a call waited for its reply).
    queue: VecDeque<Message>,
    serial: u32,
    unique_name: String,
}

impl Connection {
    /// A connection to the bus at `address` (the first of its places that takes it), authenticated and registered
    /// (`Hello`).
    pub fn open(address: &str) -> Result<Connection, Error> {
        let mut c = Connection::connect(address)?;
        let reply = c.call(Message::method_call(Some(BUS_NAME), BUS_PATH, Some(BUS_NAME), "Hello"), BUS_TIMEOUT)?;
        c.unique_name = reply.arg(0).and_then(Value::as_str).unwrap_or_default().to_string();
        Ok(c)
    }

    /// A connection to the session bus ([`session_bus_address`](super::session_bus_address)).
    pub fn session() -> Result<Connection, Error> {
        let address = super::session_bus_address().ok_or_else(|| Error::NoAddress("no session bus".into()))?;
        Connection::open(&address)
    }

    /// A connection to the peer at `address`, authenticated but not registered (a peer that is no bus).
    pub fn connect(address: &str) -> Result<Connection, Error> {
        let mut why = Vec::new();
        for place in parse_address(address) {
            let stream = match &place {
                Address::Path(path) => UnixStream::connect(path),
                Address::Abstract(name) => {
                    SocketAddr::from_abstract_name(name).and_then(|a| UnixStream::connect_addr(&a))
                }
            };
            match stream {
                Ok(stream) => {
                    let mut c = Connection {
                        stream,
                        input: Vec::new(),
                        queue: VecDeque::new(),
                        serial: 0,
                        unique_name: String::new(),
                    };
                    c.authenticate()?;
                    return Ok(c);
                }
                Err(e) => why.push(format!("{place:?}: {e}")),
            }
        }
        Err(Error::NoAddress(if why.is_empty() {
            format!("no place to connect to in \"{address}\"")
        } else {
            why.join("; ")
        }))
    }

    /// SASL's EXTERNAL: the uid (its digits in hex) the bus checks against the socket's credentials, then BEGIN.
    fn authenticate(&mut self) -> Result<(), Error> {
        let uid = uid().ok_or_else(|| Error::Auth("the process's uid can't be told (no /proc)".into()))?;
        let hex: String = uid.to_string().bytes().map(|b| format!("{b:02x}")).collect();
        self.stream.write_all(b"\0")?;
        self.stream.write_all(format!("AUTH EXTERNAL {hex}\r\n").as_bytes())?;
        let deadline = Instant::now() + BUS_TIMEOUT;
        loop {
            let line = self.auth_line(deadline)?;
            if line == "OK" || line.starts_with("OK ") {
                self.stream.write_all(b"BEGIN\r\n")?;
                return Ok(());
            }
            if line == "DATA" || line.starts_with("DATA ") {
                // (The server asks for the response it already has: an empty one confirms it.)
                self.stream.write_all(b"DATA\r\n")?;
                continue;
            }
            self.stream.write_all(b"CANCEL\r\n").ok();
            return Err(Error::Auth(format!("the bus said \"{line}\"")));
        }
    }

    /// The server's next line of the authentication, without its CRLF.
    fn auth_line(&mut self, deadline: Instant) -> Result<String, Error> {
        loop {
            if let Some(end) = self.input.windows(2).position(|w| w == b"\r\n") {
                let line = String::from_utf8_lossy(&self.input[..end]).into_owned();
                self.input.drain(..end + 2);
                return Ok(line);
            }
            if self.input.len() > 16 * 1024 {
                return Err(Error::Auth("a line too long".into()));
            }
            if let Filled::TimedOut = self.fill(Some(deadline), None)? {
                return Err(Error::Timeout);
            }
        }
    }

    /// Its unique name on the bus (":1.42"); empty on a connection to a peer.
    pub fn unique_name(&self) -> &str {
        &self.unique_name
    }

    /// Waits until bytes come (then reads what is there), `deadline` passes, or `wake` (an eventfd) is written
    /// (its counter taken).
    fn fill(&mut self, deadline: Option<Instant>, wake: Option<BorrowedFd<'_>>) -> Result<Filled, Error> {
        loop {
            let timeout = deadline.map(|d| d.saturating_duration_since(Instant::now()));
            let mut fds = vec![PollFd::readable(self.stream.as_raw_fd())];
            if let Some(w) = wake {
                fds.push(PollFd::readable(w.as_raw_fd()));
            }
            sys::poll(&mut fds, timeout)?;
            if fds.get(1).is_some_and(|f| f.ready) {
                sys::eventfd_read(wake.expect("polled"))?;
                return Ok(Filled::Woken);
            }
            if fds[0].ready {
                let mut chunk = [0u8; 16 * 1024];
                match self.stream.read(&mut chunk) {
                    Ok(0) => return Err(Error::Closed),
                    Ok(n) => {
                        self.input.extend_from_slice(&chunk[..n]);
                        return Ok(Filled::Read);
                    }
                    Err(e) if e.kind() == io::ErrorKind::Interrupted || e.kind() == io::ErrorKind::WouldBlock => {}
                    Err(e) => return Err(e.into()),
                }
            } else if deadline.is_some_and(|d| Instant::now() >= d) {
                return Ok(Filled::TimedOut);
            }
        }
    }

    /// The whole messages among the bytes read, into the queue.
    fn take_messages(&mut self) -> Result<(), Error> {
        while let Some(length) = Message::length(&self.input) {
            let length = length?;
            if self.input.len() < length {
                break;
            }
            let message = Message::decode(&self.input[..length])?;
            self.input.drain(..length);
            self.queue.push_back(message);
        }
        Ok(())
    }

    /// `message` sent (its serial set: what it returns); a call waits for nothing.
    pub fn send(&mut self, message: &Message) -> Result<u32, Error> {
        self.serial = self.serial.wrapping_add(1).max(1);
        let bytes = message.encode(self.serial)?;
        self.stream.write_all(&bytes)?;
        Ok(self.serial)
    }

    /// Calls `call` and waits for its reply at most `timeout`: its return, or its error as
    /// [`Error::Remote`]. What else comes meanwhile is kept for [`read`](Connection::read).
    pub fn call(&mut self, call: Message, timeout: Duration) -> Result<Message, Error> {
        let serial = self.send(&call)?;
        let deadline = Instant::now() + timeout;
        loop {
            self.take_messages()?;
            let reply = self.queue.iter().position(|m| {
                matches!(m.kind, MessageType::MethodReturn | MessageType::Error) && m.reply_serial == Some(serial)
            });
            if let Some(i) = reply {
                let reply = self.queue.remove(i).expect("found");
                if reply.kind == MessageType::Error {
                    return Err(Error::Remote {
                        name: reply.error_name.clone().unwrap_or_default(),
                        message: reply.error_text(),
                    });
                }
                return Ok(reply);
            }
            if let Filled::TimedOut = self.fill(Some(deadline), None)? {
                return Err(Error::Timeout);
            }
        }
    }

    /// Calls `interface`'s `member` on `destination`'s object at `path` with `args`; the reply's values.
    pub fn call_method(
        &mut self,
        destination: &str,
        path: &str,
        interface: &str,
        member: &str,
        args: Vec<Value>,
        timeout: Duration,
    ) -> Result<Vec<Value>, Error> {
        let call = Message::method_call(Some(destination), path, Some(interface), member).with_body(args);
        Ok(self.call(call, timeout)?.body)
    }

    /// A property's value (`org.freedesktop.DBus.Properties.Get`), out of its variant.
    pub fn get_property(
        &mut self,
        destination: &str,
        path: &str,
        interface: &str,
        name: &str,
        timeout: Duration,
    ) -> Result<Value, Error> {
        let body = self.call_method(
            destination,
            path,
            "org.freedesktop.DBus.Properties",
            "Get",
            vec![Value::str(interface), Value::str(name)],
            timeout,
        )?;
        match body.into_iter().next() {
            Some(Value::Variant(v)) => Ok(*v),
            _ => Err(Error::Protocol("a property that isn't a variant".into())),
        }
    }

    /// The next message that came (a call to serve, a signal), waiting at most `timeout` (none: as long as it
    /// takes) and only until `wake` (an eventfd: its counter is taken) is written; none if nothing came by then.
    pub fn read(&mut self, timeout: Option<Duration>, wake: Option<BorrowedFd<'_>>) -> Result<Option<Message>, Error> {
        let deadline = timeout.map(|t| Instant::now() + t);
        loop {
            self.take_messages()?;
            if let Some(m) = self.queue.pop_front() {
                return Ok(Some(m));
            }
            match self.fill(deadline, wake)? {
                Filled::Read => {}
                Filled::TimedOut | Filled::Woken => return Ok(None),
            }
        }
    }

    /// The next message that came, if one is there now.
    pub fn try_read(&mut self) -> Result<Option<Message>, Error> {
        self.read(Some(Duration::ZERO), None)
    }

    /// Signals matching `rule` sent to this connection from now on (`AddMatch`).
    pub fn add_match(&mut self, rule: &MatchRule) -> Result<(), Error> {
        self.bus_call("AddMatch", vec![Value::str(rule.to_string())])?;
        Ok(())
    }

    /// No more of them (`RemoveMatch`).
    pub fn remove_match(&mut self, rule: &MatchRule) -> Result<(), Error> {
        self.bus_call("RemoveMatch", vec![Value::str(rule.to_string())])?;
        Ok(())
    }

    /// Owns the well-known `name` (`RequestName` with `flags`: 1 allow replacement, 2 replace existing, 4 do not
    /// queue); the bus's answer (1: now its primary owner, 2: queued, 3: exists, 4: already its owner).
    pub fn request_name(&mut self, name: &str, flags: u32) -> Result<u32, Error> {
        let body = self.bus_call("RequestName", vec![Value::str(name), Value::Uint32(flags)])?;
        body.first().and_then(Value::as_u32).ok_or_else(|| Error::Protocol("RequestName's reply".into()))
    }

    fn bus_call(&mut self, member: &str, args: Vec<Value>) -> Result<Vec<Value>, Error> {
        self.call_method(BUS_NAME, BUS_PATH, BUS_NAME, member, args, BUS_TIMEOUT)
    }

    /// The return of `call` with `body`, unless it wants none.
    pub fn reply(&mut self, call: &Message, body: Vec<Value>) -> Result<(), Error> {
        if !call.no_reply() {
            self.send(&Message::method_return(call).with_body(body))?;
        }
        Ok(())
    }

    /// The error `name` for `call`, unless it wants no reply.
    pub fn reply_error(&mut self, call: &Message, name: &str, text: &str) -> Result<(), Error> {
        if !call.no_reply() {
            self.send(&Message::error(call, name, text))?;
        }
        Ok(())
    }

    /// Answers `call` if it is `org.freedesktop.DBus.Peer`'s (Ping, GetMachineId), as every connection does
    /// (libdbus answers them by itself); whether it was.
    pub fn answer_peer(&mut self, call: &Message) -> Result<bool, Error> {
        if call.kind != MessageType::MethodCall || call.interface.as_deref() != Some("org.freedesktop.DBus.Peer") {
            return Ok(false);
        }
        match call.member.as_deref() {
            Some("Ping") => self.reply(call, Vec::new())?,
            Some("GetMachineId") => match machine_id() {
                Some(id) => self.reply(call, vec![Value::str(id)])?,
                None => self.reply_error(call, errors::FAILED, "no machine id")?,
            },
            _ => self.reply_error(call, errors::UNKNOWN_METHOD, "no such method")?,
        }
        Ok(true)
    }
}

impl AsFd for Connection {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.stream.as_fd()
    }
}

impl AsRawFd for Connection {
    fn as_raw_fd(&self) -> RawFd {
        self.stream.as_raw_fd()
    }
}

/// The machine's id (`/etc/machine-id`, else D-Bus's own).
fn machine_id() -> Option<String> {
    ["/etc/machine-id", "/var/lib/dbus/machine-id"]
        .iter()
        .find_map(|p| std::fs::read_to_string(p).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Which messages a connection is sent (`AddMatch`'s rule), and whether a message is one of them.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct MatchRule {
    kind: Option<&'static str>,
    sender: Option<String>,
    interface: Option<String>,
    member: Option<String>,
    path: Option<String>,
    args: Vec<(u8, String)>,
}

impl MatchRule {
    /// Signals (of any interface, member, path until narrowed).
    pub fn signal() -> MatchRule {
        MatchRule { kind: Some("signal"), ..MatchRule::default() }
    }

    pub fn sender(mut self, sender: &str) -> MatchRule {
        self.sender = Some(sender.to_string());
        self
    }

    pub fn interface(mut self, interface: &str) -> MatchRule {
        self.interface = Some(interface.to_string());
        self
    }

    pub fn member(mut self, member: &str) -> MatchRule {
        self.member = Some(member.to_string());
        self
    }

    pub fn path(mut self, path: &str) -> MatchRule {
        self.path = Some(path.to_string());
        self
    }

    /// Its `n`th argument (0 to 63) a string equal to `value`.
    pub fn arg(mut self, n: u8, value: &str) -> MatchRule {
        self.args.push((n.min(63), value.to_string()));
        self
    }

    /// Whether `m` is one it names (the sender as the bus wrote it: a unique name matches only itself).
    pub fn matches(&self, m: &Message) -> bool {
        let kind = match m.kind {
            MessageType::Signal => "signal",
            MessageType::MethodCall => "method_call",
            MessageType::MethodReturn => "method_return",
            MessageType::Error => "error",
        };
        let same = |want: &Option<String>, have: &Option<String>| want.is_none() || want == have;
        self.kind.is_none_or(|k| k == kind)
            && same(&self.sender, &m.sender)
            && same(&self.interface, &m.interface)
            && same(&self.member, &m.member)
            && same(&self.path, &m.path)
            && self.args.iter().all(|(n, v)| m.arg(*n as usize).and_then(Value::as_str) == Some(v.as_str()))
    }
}

impl fmt::Display for MatchRule {
    /// The rule as `AddMatch` takes it: `type='signal',interface='…'`, a quote in a value as `'\''`.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut parts = Vec::new();
        let quoted = |v: &str| format!("'{}'", v.replace('\'', "'\\''"));
        if let Some(k) = self.kind {
            parts.push(format!("type={}", quoted(k)));
        }
        for (key, value) in
            [("sender", &self.sender), ("interface", &self.interface), ("member", &self.member), ("path", &self.path)]
        {
            if let Some(v) = value {
                parts.push(format!("{key}={}", quoted(v)));
            }
        }
        for (n, v) in &self.args {
            parts.push(format!("arg{n}={}", quoted(v)));
        }
        f.write_str(&parts.join(","))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_rules_are_written_and_match() {
        let rule = MatchRule::signal()
            .interface("org.freedesktop.portal.Settings")
            .member("SettingChanged")
            .arg(0, "org.freedesktop.appearance");
        assert_eq!(
            rule.to_string(),
            "type='signal',interface='org.freedesktop.portal.Settings',member='SettingChanged',\
             arg0='org.freedesktop.appearance'"
        );
        assert_eq!(MatchRule::signal().path("/it's").to_string(), "type='signal',path='/it'\\''s'");
        let signal =
            Message::signal("/org/freedesktop/portal/desktop", "org.freedesktop.portal.Settings", "SettingChanged")
                .with_body(vec![Value::str("org.freedesktop.appearance"), Value::str("color-scheme")]);
        assert!(rule.matches(&signal));
        let other = Message { body: vec![Value::str("org.gnome.desktop.interface")], ..signal.clone() };
        assert!(!rule.matches(&other));
        assert!(!rule.clone().member("Other").matches(&signal));
    }
}
