//! KompozR's own D-Bus client (C++ loaded libdbus at run time): what the desktop's services are reached with — the
//! settings portal ([`portal`](super::portal)), the accessibility bus ([`atspi`](super::atspi)), the screen saver
//! ([`desktop`](super::desktop)). Safe Rust over a Unix socket: addresses ([`address`]), types and signatures
//! ([`signature`]), values and their wire format ([`value`]), messages ([`message`]), connections ([`connection`]:
//! authentication, calls, signals, serving objects).
//!
//! ```text
//! let mut bus = Connection::session()?;
//! bus.add_match(&MatchRule::signal().interface(SETTINGS).member("SettingChanged"))?;
//! let reply = bus.call(Message::method_call(Some(PORTAL), PATH, Some(SETTINGS), "ReadOne")
//!     .with_body(vec![Value::str("org.freedesktop.appearance"), Value::str("color-scheme")]), timeout)?;
//! while let Some(m) = bus.read(None, Some(wake.as_fd()))? { … }   // signals, calls to serve
//! ```
//!
//! Not done: file descriptors passed with messages (never negotiated: nothing KompozR asks for carries any), and
//! transports other than Unix sockets (a desktop's buses are on those).

pub mod address;
pub mod connection;
pub mod message;
pub mod signature;
pub mod value;

pub use address::{Address, parse_address, session_bus_address};
pub use connection::{BUS_TIMEOUT, Connection, MatchRule};
pub use message::{Message, MessageType};
pub use signature::Type;
pub use value::Value;

use std::fmt;
use std::io;

/// The standard errors a served object answers with.
pub mod errors {
    pub const FAILED: &str = "org.freedesktop.DBus.Error.Failed";
    pub const UNKNOWN_METHOD: &str = "org.freedesktop.DBus.Error.UnknownMethod";
    pub const UNKNOWN_OBJECT: &str = "org.freedesktop.DBus.Error.UnknownObject";
    pub const UNKNOWN_INTERFACE: &str = "org.freedesktop.DBus.Error.UnknownInterface";
    pub const UNKNOWN_PROPERTY: &str = "org.freedesktop.DBus.Error.UnknownProperty";
    pub const INVALID_ARGS: &str = "org.freedesktop.DBus.Error.InvalidArgs";
    pub const PROPERTY_READ_ONLY: &str = "org.freedesktop.DBus.Error.PropertyReadOnly";
}

/// What went wrong on a connection.
#[derive(Debug)]
pub enum Error {
    /// The socket's.
    Io(io::Error),
    /// No bus where the address says (or no address at all).
    NoAddress(String),
    /// The bus didn't take the connection's credentials.
    Auth(String),
    /// Bytes or values that aren't D-Bus's, or a message that can't be sent as it is.
    Protocol(String),
    /// No reply in time.
    Timeout,
    /// The bus closed the connection.
    Closed,
    /// The error a call was answered with.
    Remote { name: String, message: String },
}

impl Error {
    /// The error's name, if it is one a call was answered with.
    pub fn remote_name(&self) -> Option<&str> {
        if let Error::Remote { name, .. } = self { Some(name) } else { None }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "D-Bus: {e}"),
            Error::NoAddress(why) => write!(f, "D-Bus: no bus ({why})"),
            Error::Auth(why) => write!(f, "D-Bus: authentication failed ({why})"),
            Error::Protocol(why) => write!(f, "D-Bus: {why}"),
            Error::Timeout => f.write_str("D-Bus: no reply in time"),
            Error::Closed => f.write_str("D-Bus: the connection was closed"),
            Error::Remote { name, message } => write!(f, "{name}: {message}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Error {
        Error::Io(e)
    }
}
