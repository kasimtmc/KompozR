//! Where a D-Bus bus is: its address ("unix:path=/run/user/1000/bus", "unix:abstract=…;unix:path=…"), the session
//! bus's (`DBUS_SESSION_BUS_ADDRESS`, else `$XDG_RUNTIME_DIR/bus`, else `/run/user/<uid>/bus`), and the user the
//! process runs as (what SASL's EXTERNAL tells the bus).

use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

/// A place to connect to that an address names.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Address {
    /// A Unix socket at a path.
    Path(PathBuf),
    /// A Unix socket in the abstract namespace (Linux's), by its name.
    Abstract(Vec<u8>),
}

/// The places `address` names, in order (its `;`-separated parts): the `unix:` ones with a `path` or an `abstract`
/// name; the others (`tcp:`, `unix:tmpdir=`, `launchd:`, which listen or need other authentication) left out.
/// Values are percent-decoded, as the specification escapes them.
pub fn parse_address(address: &str) -> Vec<Address> {
    let mut out = Vec::new();
    for part in address.split(';') {
        let Some((transport, rest)) = part.split_once(':') else { continue };
        if transport != "unix" {
            continue;
        }
        for pair in rest.split(',') {
            let Some((key, value)) = pair.split_once('=') else { continue };
            let Some(value) = unescape(value) else { break };
            match key {
                "path" => out.push(Address::Path(PathBuf::from(String::from_utf8_lossy(&value).into_owned()))),
                "abstract" => out.push(Address::Abstract(value)),
                _ => continue,
            }
            break;
        }
    }
    out
}

/// An address value's bytes, its `%xx` decoded; none if one is malformed.
fn unescape(value: &str) -> Option<Vec<u8>> {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    Some(out)
}

/// `value` escaped for an address: anything but `[-0-9A-Za-z_/.\*]` as `%xx`.
pub fn escape(value: &[u8]) -> String {
    let mut out = String::new();
    for &b in value {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'/' | b'.' | b'\\' | b'*') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02x}"));
        }
    }
    out
}

/// The user the process runs as (the owner of `/proc/self`: its effective uid); none without `/proc`.
pub fn uid() -> Option<u32> {
    std::fs::metadata("/proc/self").ok().map(|m| m.uid())
}

/// The session bus's address: `DBUS_SESSION_BUS_ADDRESS`, else the user's bus socket where systemd puts it
/// (`$XDG_RUNTIME_DIR/bus`, else `/run/user/<uid>/bus`) if it is there; none if neither.
pub fn session_bus_address() -> Option<String> {
    if let Ok(address) = std::env::var("DBUS_SESSION_BUS_ADDRESS")
        && !address.is_empty()
    {
        return Some(address);
    }
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .or_else(|| uid().map(|u| PathBuf::from(format!("/run/user/{u}"))))?;
    let socket = runtime.join("bus");
    socket_exists(&socket).then(|| format!("unix:path={}", escape(socket.as_os_str().as_encoded_bytes())))
}

fn socket_exists(path: &Path) -> bool {
    use std::os::unix::fs::FileTypeExt;
    std::fs::metadata(path).is_ok_and(|m| m.file_type().is_socket())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_are_parsed() {
        assert_eq!(
            parse_address("unix:path=/run/user/1000/bus,guid=0123"),
            [Address::Path("/run/user/1000/bus".into())]
        );
        assert_eq!(
            parse_address(
                "tcp:host=localhost,port=1;unix:abstract=kompoz%2dtest%00x;unix:tmpdir=/tmp;unix:path=/a%20b"
            ),
            [Address::Abstract(b"kompoz-test\0x".to_vec()), Address::Path("/a b".into())]
        );
        assert!(parse_address("unix:path=/bad%zz").is_empty());
        assert!(parse_address("").is_empty());
        assert_eq!(escape(b"/run/user/1000/a b"), "/run/user/1000/a%20b");
        assert!(uid().is_some());
    }
}
