//! D-Bus's values ([`Value`]) and their wire format: marshalled little endian, each aligned from the message's start
//! as its type says (an `Encoder`), unmarshalled in either endianness and checked as they are read (a
//! `Decoder`): padding, booleans, UTF-8 and NULs in strings, object paths, signatures, array lengths, nesting.

use super::Error;
use super::signature::{MAX_SIGNATURE, Type};

/// The longest array, in bytes (the specification's 64 MiB).
const MAX_ARRAY: usize = 64 << 20;
/// How deep values may nest (variants in variants, arrays, structs) while they are read: the specification's 64.
const MAX_NESTING: usize = 64;

/// A D-Bus value.
#[derive(Clone, PartialEq, Debug)]
pub enum Value {
    Byte(u8),
    Boolean(bool),
    Int16(i16),
    Uint16(u16),
    Int32(i32),
    Uint32(u32),
    Int64(i64),
    Uint64(u64),
    Double(f64),
    String(String),
    ObjectPath(String),
    Signature(String),
    /// An index into the message's file descriptors (none are passed on KompozR's connections).
    UnixFd(u32),
    Variant(Box<Value>),
    /// An array: its element's type (an empty one has one too) and its items.
    Array(Type, Vec<Value>),
    Struct(Vec<Value>),
    DictEntry(Box<Value>, Box<Value>),
}

impl Value {
    pub fn str(s: impl Into<String>) -> Value {
        Value::String(s.into())
    }

    pub fn path(p: impl Into<String>) -> Value {
        Value::ObjectPath(p.into())
    }

    pub fn variant(v: Value) -> Value {
        Value::Variant(Box::new(v))
    }

    /// An array of `element`s.
    pub fn array(element: Type, items: Vec<Value>) -> Value {
        Value::Array(element, items)
    }

    /// A dictionary `a{key value}` of `entries`, in order.
    pub fn dict(key: Type, value: Type, entries: Vec<(Value, Value)>) -> Value {
        let items = entries.into_iter().map(|(k, v)| Value::DictEntry(Box::new(k), Box::new(v))).collect();
        Value::Array(Type::DictEntry(Box::new(key), Box::new(value)), items)
    }

    /// An empty `a{sv}` (what AT-SPI's events and the portals carry when they have nothing to add).
    pub fn empty_properties() -> Value {
        Value::dict(Type::String, Type::Variant, Vec::new())
    }

    /// Its type.
    pub fn type_of(&self) -> Type {
        match self {
            Value::Byte(_) => Type::Byte,
            Value::Boolean(_) => Type::Boolean,
            Value::Int16(_) => Type::Int16,
            Value::Uint16(_) => Type::Uint16,
            Value::Int32(_) => Type::Int32,
            Value::Uint32(_) => Type::Uint32,
            Value::Int64(_) => Type::Int64,
            Value::Uint64(_) => Type::Uint64,
            Value::Double(_) => Type::Double,
            Value::String(_) => Type::String,
            Value::ObjectPath(_) => Type::ObjectPath,
            Value::Signature(_) => Type::Signature,
            Value::UnixFd(_) => Type::UnixFd,
            Value::Variant(_) => Type::Variant,
            Value::Array(element, _) => Type::Array(Box::new(element.clone())),
            Value::Struct(fields) => Type::Struct(fields.iter().map(Value::type_of).collect()),
            Value::DictEntry(k, v) => Type::DictEntry(Box::new(k.type_of()), Box::new(v.type_of())),
        }
    }

    /// What is inside its variants (itself if it is none).
    pub fn unwrap_variants(&self) -> &Value {
        let mut v = self;
        while let Value::Variant(inner) = v {
            v = inner;
        }
        v
    }

    /// A string's, an object path's or a signature's text.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) | Value::ObjectPath(s) | Value::Signature(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        if let Value::Boolean(b) = self { Some(*b) } else { None }
    }

    pub fn as_u8(&self) -> Option<u8> {
        if let Value::Byte(b) = self { Some(*b) } else { None }
    }

    pub fn as_i16(&self) -> Option<i16> {
        if let Value::Int16(v) = self { Some(*v) } else { None }
    }

    pub fn as_u16(&self) -> Option<u16> {
        if let Value::Uint16(v) = self { Some(*v) } else { None }
    }

    pub fn as_i32(&self) -> Option<i32> {
        if let Value::Int32(v) = self { Some(*v) } else { None }
    }

    pub fn as_u32(&self) -> Option<u32> {
        if let Value::Uint32(v) = self { Some(*v) } else { None }
    }

    pub fn as_i64(&self) -> Option<i64> {
        if let Value::Int64(v) = self { Some(*v) } else { None }
    }

    pub fn as_u64(&self) -> Option<u64> {
        if let Value::Uint64(v) = self { Some(*v) } else { None }
    }

    pub fn as_f64(&self) -> Option<f64> {
        if let Value::Double(v) = self { Some(*v) } else { None }
    }

    /// A struct's fields.
    pub fn fields(&self) -> Option<&[Value]> {
        if let Value::Struct(f) = self { Some(f) } else { None }
    }

    /// An array's items.
    pub fn items(&self) -> Option<&[Value]> {
        if let Value::Array(_, items) = self { Some(items) } else { None }
    }

    /// A dictionary entry's key and value.
    pub fn entry(&self) -> Option<(&Value, &Value)> {
        if let Value::DictEntry(k, v) = self { Some((k, v)) } else { None }
    }

    /// The value a dictionary keyed by strings has for `key`.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.items()?.iter().filter_map(Value::entry).find(|(k, _)| k.as_str() == Some(key)).map(|(_, v)| v)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Value {
        Value::String(s.to_string())
    }
}

impl From<String> for Value {
    fn from(s: String) -> Value {
        Value::String(s)
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Value {
        Value::Boolean(b)
    }
}

impl From<i32> for Value {
    fn from(v: i32) -> Value {
        Value::Int32(v)
    }
}

impl From<u32> for Value {
    fn from(v: u32) -> Value {
        Value::Uint32(v)
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Value {
        Value::Double(v)
    }
}

/// Whether `p` is a valid object path: "/", or "/" then elements of `[A-Za-z0-9_]` separated by single slashes, no
/// slash at the end.
pub fn is_valid_object_path(p: &str) -> bool {
    if p == "/" {
        return true;
    }
    let Some(rest) = p.strip_prefix('/') else { return false };
    rest.split('/').all(|e| !e.is_empty() && e.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'))
}

// ---- Writing ----------------------------------------------------------------------------------------------------

/// Values marshalled little endian into a buffer whose start is at an 8-byte boundary of the message (its header's
/// start, or its body's).
pub(crate) struct Encoder {
    pub buf: Vec<u8>,
}

impl Encoder {
    pub fn new() -> Encoder {
        Encoder { buf: Vec::new() }
    }

    fn pad(&mut self, alignment: usize) {
        let n = self.buf.len().next_multiple_of(alignment);
        self.buf.resize(n, 0);
    }

    fn bytes(&mut self, alignment: usize, b: &[u8]) {
        self.pad(alignment);
        self.buf.extend_from_slice(b);
    }

    fn string(&mut self, s: &str) -> Result<(), Error> {
        if s.contains('\0') {
            return Err(Error::Protocol("a string with a NUL".into()));
        }
        self.bytes(4, &(s.len() as u32).to_le_bytes());
        self.buf.extend_from_slice(s.as_bytes());
        self.buf.push(0);
        Ok(())
    }

    fn signature(&mut self, s: &str) {
        self.buf.push(s.len() as u8);
        self.buf.extend_from_slice(s.as_bytes());
        self.buf.push(0);
    }

    /// `v` marshalled after what is there.
    pub fn value(&mut self, v: &Value) -> Result<(), Error> {
        match v {
            Value::Byte(b) => self.buf.push(*b),
            Value::Boolean(b) => self.bytes(4, &u32::from(*b).to_le_bytes()),
            Value::Int16(x) => self.bytes(2, &x.to_le_bytes()),
            Value::Uint16(x) => self.bytes(2, &x.to_le_bytes()),
            Value::Int32(x) => self.bytes(4, &x.to_le_bytes()),
            Value::Uint32(x) | Value::UnixFd(x) => self.bytes(4, &x.to_le_bytes()),
            Value::Int64(x) => self.bytes(8, &x.to_le_bytes()),
            Value::Uint64(x) => self.bytes(8, &x.to_le_bytes()),
            Value::Double(x) => self.bytes(8, &x.to_le_bytes()),
            Value::String(s) => self.string(s)?,
            Value::ObjectPath(p) => {
                if !is_valid_object_path(p) {
                    return Err(Error::Protocol(format!("not an object path: \"{p}\"")));
                }
                self.string(p)?;
            }
            Value::Signature(s) => {
                Type::parse(s)?;
                self.signature(s);
            }
            Value::Variant(inner) => {
                let signature = inner.type_of().signature();
                if signature.len() > MAX_SIGNATURE {
                    return Err(Error::Protocol("a variant's signature is too long".into()));
                }
                self.signature(&signature);
                self.value(inner)?;
            }
            Value::Array(element, items) => {
                self.pad(4);
                let length_at = self.buf.len();
                self.buf.extend_from_slice(&[0; 4]);
                self.pad(element.alignment());
                let start = self.buf.len();
                for item in items {
                    if item.type_of() != *element {
                        return Err(Error::Protocol(format!(
                            "an array of {} holding a {}",
                            element.signature(),
                            item.type_of().signature()
                        )));
                    }
                    self.value(item)?;
                }
                let length = self.buf.len() - start;
                if length > MAX_ARRAY {
                    return Err(Error::Protocol("an array over 64 MiB".into()));
                }
                self.buf[length_at..length_at + 4].copy_from_slice(&(length as u32).to_le_bytes());
            }
            Value::Struct(fields) => {
                if fields.is_empty() {
                    return Err(Error::Protocol("an empty struct".into()));
                }
                self.pad(8);
                for f in fields {
                    self.value(f)?;
                }
            }
            Value::DictEntry(k, v) => {
                if !k.type_of().is_basic() {
                    return Err(Error::Protocol("a dictionary keyed by a container".into()));
                }
                self.pad(8);
                self.value(k)?;
                self.value(v)?;
            }
        }
        Ok(())
    }
}

// ---- Reading ----------------------------------------------------------------------------------------------------

/// Values read from `data` (whose start is at an 8-byte boundary of the message) from `pos` on, big or little
/// endian.
pub(crate) struct Decoder<'a> {
    data: &'a [u8],
    pub pos: usize,
    big_endian: bool,
}

impl<'a> Decoder<'a> {
    pub fn new(data: &'a [u8], pos: usize, big_endian: bool) -> Decoder<'a> {
        Decoder { data, pos, big_endian }
    }

    fn short(&self) -> Error {
        Error::Protocol("a message ends inside a value".into())
    }

    /// Skips to `alignment`: the padding is NULs.
    pub fn align(&mut self, alignment: usize) -> Result<(), Error> {
        let to = self.pos.next_multiple_of(alignment);
        let padding = self.data.get(self.pos..to).ok_or_else(|| self.short())?;
        if padding.iter().any(|&b| b != 0) {
            return Err(Error::Protocol("padding that isn't NULs".into()));
        }
        self.pos = to;
        Ok(())
    }

    fn take<const N: usize>(&mut self) -> Result<[u8; N], Error> {
        self.align(N)?;
        let b = self.data.get(self.pos..self.pos + N).ok_or_else(|| self.short())?;
        self.pos += N;
        Ok(b.try_into().expect("N bytes"))
    }

    fn u16(&mut self) -> Result<u16, Error> {
        let b = self.take::<2>()?;
        Ok(if self.big_endian { u16::from_be_bytes(b) } else { u16::from_le_bytes(b) })
    }

    pub fn u32(&mut self) -> Result<u32, Error> {
        let b = self.take::<4>()?;
        Ok(if self.big_endian { u32::from_be_bytes(b) } else { u32::from_le_bytes(b) })
    }

    fn u64(&mut self) -> Result<u64, Error> {
        let b = self.take::<8>()?;
        Ok(if self.big_endian { u64::from_be_bytes(b) } else { u64::from_le_bytes(b) })
    }

    /// `len` bytes then a NUL, as UTF-8 without NULs.
    fn text(&mut self, len: usize) -> Result<String, Error> {
        let end = self.pos.checked_add(len).ok_or_else(|| self.short())?;
        let bytes = self.data.get(self.pos..end).ok_or_else(|| self.short())?;
        if self.data.get(end) != Some(&0) {
            return Err(Error::Protocol("a string not ended by a NUL".into()));
        }
        if bytes.contains(&0) {
            return Err(Error::Protocol("a string with a NUL".into()));
        }
        let s = std::str::from_utf8(bytes).map_err(|_| Error::Protocol("a string that isn't UTF-8".into()))?;
        self.pos = end + 1;
        Ok(s.to_string())
    }

    fn string(&mut self) -> Result<String, Error> {
        let len = self.u32()? as usize;
        self.text(len)
    }

    fn signature(&mut self) -> Result<String, Error> {
        let len = *self.data.get(self.pos).ok_or_else(|| self.short())? as usize;
        self.pos += 1;
        self.text(len)
    }

    /// A value of type `t`.
    pub fn value(&mut self, t: &Type) -> Result<Value, Error> {
        self.nested(t, 0)
    }

    fn nested(&mut self, t: &Type, depth: usize) -> Result<Value, Error> {
        if depth > MAX_NESTING {
            return Err(Error::Protocol("values nested too deep".into()));
        }
        Ok(match t {
            Type::Byte => {
                let b = *self.data.get(self.pos).ok_or_else(|| self.short())?;
                self.pos += 1;
                Value::Byte(b)
            }
            Type::Boolean => match self.u32()? {
                0 => Value::Boolean(false),
                1 => Value::Boolean(true),
                other => return Err(Error::Protocol(format!("a boolean of {other}"))),
            },
            Type::Int16 => Value::Int16(self.u16()? as i16),
            Type::Uint16 => Value::Uint16(self.u16()?),
            Type::Int32 => Value::Int32(self.u32()? as i32),
            Type::Uint32 => Value::Uint32(self.u32()?),
            Type::UnixFd => Value::UnixFd(self.u32()?),
            Type::Int64 => Value::Int64(self.u64()? as i64),
            Type::Uint64 => Value::Uint64(self.u64()?),
            Type::Double => Value::Double(f64::from_bits(self.u64()?)),
            Type::String => Value::String(self.string()?),
            Type::ObjectPath => {
                let p = self.string()?;
                if !is_valid_object_path(&p) {
                    return Err(Error::Protocol(format!("not an object path: \"{p}\"")));
                }
                Value::ObjectPath(p)
            }
            Type::Signature => {
                let s = self.signature()?;
                Type::parse(&s)?;
                Value::Signature(s)
            }
            Type::Variant => {
                let s = self.signature()?;
                let inner = Type::parse_single(&s)?;
                Value::Variant(Box::new(self.nested(&inner, depth + 1)?))
            }
            Type::Array(element) => {
                let len = self.u32()? as usize;
                if len > MAX_ARRAY {
                    return Err(Error::Protocol(format!("an array of {len} bytes")));
                }
                self.align(element.alignment())?;
                let end = self.pos + len;
                if end > self.data.len() {
                    return Err(self.short());
                }
                let mut items = Vec::new();
                while self.pos < end {
                    items.push(self.nested(element, depth + 1)?);
                }
                if self.pos != end {
                    return Err(Error::Protocol("an array's items overrun its length".into()));
                }
                Value::Array((**element).clone(), items)
            }
            Type::Struct(fields) => {
                self.align(8)?;
                let mut values = Vec::with_capacity(fields.len());
                for f in fields {
                    values.push(self.nested(f, depth + 1)?);
                }
                Value::Struct(values)
            }
            Type::DictEntry(k, v) => {
                self.align(8)?;
                let key = self.nested(k, depth + 1)?;
                let value = self.nested(v, depth + 1)?;
                Value::DictEntry(Box::new(key), Box::new(value))
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(values: &[Value]) -> Vec<u8> {
        let mut e = Encoder::new();
        values.iter().for_each(|v| e.value(v).unwrap());
        e.buf
    }

    fn decode(bytes: &[u8], signature: &str, big: bool) -> Result<Vec<Value>, Error> {
        let mut d = Decoder::new(bytes, 0, big);
        let values = Type::parse(signature)?.iter().map(|t| d.value(t)).collect::<Result<Vec<_>, _>>()?;
        if d.pos != bytes.len() {
            return Err(Error::Protocol("bytes left".into()));
        }
        Ok(values)
    }

    fn every_kind() -> Vec<Value> {
        vec![
            Value::Byte(7),
            Value::Boolean(true),
            Value::Int16(-2),
            Value::Uint16(65000),
            Value::Byte(1), // (so that what follows starts at an odd offset)
            Value::Int32(-70000),
            Value::Uint32(4_000_000_000),
            Value::Byte(2),
            Value::Int64(-1 << 40),
            Value::Uint64(u64::MAX - 1),
            Value::Double(-0.125),
            Value::str("héllo"),
            Value::path("/org/a11y/atspi/accessible/12"),
            Value::Signature("a{sv}".into()),
            Value::UnixFd(3),
            Value::variant(Value::Struct(vec![Value::Double(0.25), Value::Double(0.5), Value::Double(1.0)])),
            Value::variant(Value::variant(Value::Uint32(1))),
            Value::array(Type::Int64, Vec::new()),
            Value::Byte(3),
            Value::array(Type::Struct(vec![Type::String, Type::ObjectPath]), Vec::new()),
            Value::array(
                Type::Struct(vec![Type::String, Type::ObjectPath]),
                vec![
                    Value::Struct(vec![Value::str(":1.5"), Value::path("/")]),
                    Value::Struct(vec![Value::str(""), Value::path("/org/a11y/atspi/null")]),
                ],
            ),
            Value::dict(
                Type::String,
                Type::Variant,
                vec![
                    (Value::str("a"), Value::variant(Value::Boolean(false))),
                    (Value::str("b"), Value::variant(Value::array(Type::Byte, vec![Value::Byte(1), Value::Byte(2)]))),
                ],
            ),
            Value::array(Type::Array(Box::new(Type::Uint16)), vec![Value::array(Type::Uint16, vec![Value::Uint16(9)])]),
        ]
    }

    #[test]
    fn every_kind_round_trips_at_odd_offsets() {
        let values = every_kind();
        let signature: String = values.iter().map(|v| v.type_of().signature()).collect();
        let bytes = encode(&values);
        assert_eq!(decode(&bytes, &signature, false).unwrap(), values);
        // Each value after one byte, three bytes, five: aligned from the start all the same.
        for lead in 1..8 {
            let mut all = vec![Value::array(Type::Byte, vec![Value::Byte(0); lead])];
            all.extend(values.iter().cloned());
            let bytes = encode(&all);
            assert_eq!(decode(&bytes, &format!("ay{signature}"), false).unwrap(), all);
        }
    }

    #[test]
    fn alignment_is_from_the_start() {
        // y, then u at 4, then (t) at 8, then n at 16, then an empty a(y) whose items would start at 24.
        let bytes = encode(&[
            Value::Byte(1),
            Value::Uint32(2),
            Value::Struct(vec![Value::Uint64(3)]),
            Value::Int16(4),
            Value::array(Type::Struct(vec![Type::Byte]), Vec::new()),
        ]);
        assert_eq!(bytes.len(), 24);
        assert_eq!(&bytes[..8], &[1, 0, 0, 0, 2, 0, 0, 0]);
        assert_eq!(&bytes[16..24], &[4, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn big_endian_is_read() {
        // (u n s ad) as a big-endian peer writes them.
        let mut b = vec![0, 0, 0, 5, 0xFF, 0xFE, 0, 0, 0, 0, 0, 2, b'h', b'i', 0, 0];
        b.extend_from_slice(&8u32.to_be_bytes());
        b.extend_from_slice(&[0; 4]);
        b.extend_from_slice(&1.5f64.to_be_bytes());
        let values = decode(&b, "unsad", true).unwrap();
        assert_eq!(
            values,
            vec![
                Value::Uint32(5),
                Value::Int16(-2),
                Value::str("hi"),
                Value::array(Type::Double, vec![Value::Double(1.5)])
            ]
        );
    }

    #[test]
    fn bad_bytes_are_refused() {
        let ok = encode(&[Value::str("abc")]);
        assert!(decode(&ok[..ok.len() - 1], "s", false).is_err()); // cut short
        let mut no_nul = ok.clone();
        *no_nul.last_mut().unwrap() = b'x';
        assert!(decode(&no_nul, "s", false).is_err());
        let mut not_utf8 = ok.clone();
        not_utf8[4] = 0xFF;
        assert!(decode(&not_utf8, "s", false).is_err());
        assert!(decode(&2u32.to_le_bytes(), "b", false).is_err()); // a boolean of 2
        assert!(decode(&[1, 9, 0, 0, 0, 0, 0, 0], "yu", false).is_err()); // padding that isn't NULs
        assert!(decode(&encode(&[Value::str("/a//b")]), "o", false).is_err());
        assert!(decode(&[2, b'a', b'(', 0], "g", false).is_err());
        assert!(decode(&[1, b'z', 0], "v", false).is_err());
        let mut long = 1u32.to_le_bytes().to_vec(); // an array of u32 one byte long
        long.extend_from_slice(&[0; 4]);
        assert!(decode(&long, "au", false).is_err());
        assert!(decode(&(65u32 << 20).to_le_bytes(), "ay", false).is_err());
        // Variants in variants, too deep.
        let mut deep = Value::Byte(0);
        for _ in 0..70 {
            deep = Value::variant(deep);
        }
        assert!(decode(&encode(&[deep]), "v", false).is_err());
    }

    #[test]
    fn bad_values_are_not_written() {
        let mut e = Encoder::new();
        assert!(e.value(&Value::str("a\0b")).is_err());
        assert!(e.value(&Value::path("relative")).is_err());
        assert!(e.value(&Value::path("/trailing/")).is_err());
        assert!(e.value(&Value::Signature("a{".into())).is_err());
        assert!(e.value(&Value::array(Type::Int32, vec![Value::Uint32(1)])).is_err());
        assert!(e.value(&Value::Struct(Vec::new())).is_err());
        assert!(is_valid_object_path("/") && is_valid_object_path("/org/a11y/bus") && !is_valid_object_path(""));
    }

    #[test]
    fn values_are_looked_into() {
        let d = Value::dict(Type::String, Type::Variant, vec![(Value::str("x"), Value::variant(Value::Int32(4)))]);
        assert_eq!(d.get("x").map(Value::unwrap_variants), Some(&Value::Int32(4)));
        assert_eq!(d.get("y"), None);
        assert_eq!(d.type_of().signature(), "a{sv}");
        assert_eq!(Value::variant(Value::variant(Value::str("s"))).unwrap_variants().as_str(), Some("s"));
    }
}
