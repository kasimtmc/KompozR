//! D-Bus's types as signatures write them ("a{sv}", "(so)"): parsed into [`Type`]s, checked as the specification
//! limits them (255 bytes, 32 arrays and 32 structs deep, dictionary entries only in arrays and keyed by a basic
//! type), and written back.

use super::Error;

/// The longest signature.
pub const MAX_SIGNATURE: usize = 255;
/// How deep arrays may nest, and structs (dictionary entries count as structs).
const MAX_DEPTH: usize = 32;

/// A complete D-Bus type.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Type {
    Byte,
    Boolean,
    Int16,
    Uint16,
    Int32,
    Uint32,
    Int64,
    Uint64,
    Double,
    String,
    ObjectPath,
    Signature,
    /// An index into the message's file descriptors (`h`).
    UnixFd,
    Variant,
    Array(Box<Type>),
    Struct(Vec<Type>),
    /// A dictionary's entry: only an array's element (`a{kv}`).
    DictEntry(Box<Type>, Box<Type>),
}

impl Type {
    /// The types `signature` lists in order (a message's body: any number of complete types).
    pub fn parse(signature: &str) -> Result<Vec<Type>, Error> {
        if signature.len() > MAX_SIGNATURE {
            return Err(Error::Protocol(format!("signature too long ({} bytes)", signature.len())));
        }
        let bytes = signature.as_bytes();
        let mut at = 0;
        let mut types = Vec::new();
        while at < bytes.len() {
            types.push(parse_one(bytes, &mut at, 0, 0, false)?);
        }
        Ok(types)
    }

    /// The single complete type `signature` is (a variant's).
    pub fn parse_single(signature: &str) -> Result<Type, Error> {
        let mut types = Type::parse(signature)?;
        if types.len() != 1 {
            return Err(Error::Protocol(format!("\"{signature}\" is not a single complete type")));
        }
        Ok(types.remove(0))
    }

    /// The type of a dictionary `a{key value}`.
    pub fn dict(key: Type, value: Type) -> Type {
        Type::Array(Box::new(Type::DictEntry(Box::new(key), Box::new(value))))
    }

    /// Whether it is a basic type (one that may key a dictionary).
    pub fn is_basic(&self) -> bool {
        !matches!(self, Type::Variant | Type::Array(_) | Type::Struct(_) | Type::DictEntry(..))
    }

    /// The boundary its values start on, in bytes from the message's start.
    pub fn alignment(&self) -> usize {
        match self {
            Type::Byte | Type::Signature | Type::Variant => 1,
            Type::Int16 | Type::Uint16 => 2,
            Type::Boolean | Type::Int32 | Type::Uint32 | Type::UnixFd | Type::String | Type::ObjectPath => 4,
            Type::Array(_) => 4,
            Type::Int64 | Type::Uint64 | Type::Double | Type::Struct(_) | Type::DictEntry(..) => 8,
        }
    }

    /// Its signature, appended to `out`.
    pub fn write_signature(&self, out: &mut String) {
        match self {
            Type::Byte => out.push('y'),
            Type::Boolean => out.push('b'),
            Type::Int16 => out.push('n'),
            Type::Uint16 => out.push('q'),
            Type::Int32 => out.push('i'),
            Type::Uint32 => out.push('u'),
            Type::Int64 => out.push('x'),
            Type::Uint64 => out.push('t'),
            Type::Double => out.push('d'),
            Type::String => out.push('s'),
            Type::ObjectPath => out.push('o'),
            Type::Signature => out.push('g'),
            Type::UnixFd => out.push('h'),
            Type::Variant => out.push('v'),
            Type::Array(element) => {
                out.push('a');
                element.write_signature(out);
            }
            Type::Struct(fields) => {
                out.push('(');
                fields.iter().for_each(|f| f.write_signature(out));
                out.push(')');
            }
            Type::DictEntry(key, value) => {
                out.push('{');
                key.write_signature(out);
                value.write_signature(out);
                out.push('}');
            }
        }
    }

    /// Its signature.
    pub fn signature(&self) -> String {
        let mut out = String::new();
        self.write_signature(&mut out);
        out
    }
}

/// The signature of `types` in order.
pub fn signature_of(types: &[Type]) -> String {
    let mut out = String::new();
    types.iter().for_each(|t| t.write_signature(&mut out));
    out
}

/// Whether `s` is a valid signature (a variant's, a `g` value's).
pub fn is_valid_signature(s: &str) -> bool {
    Type::parse(s).is_ok()
}

/// One complete type from `bytes[*at..]`; `arrays`, `structs`: how deep it already is; `in_array`: it is an
/// array's element (where a dictionary entry may be).
fn parse_one(bytes: &[u8], at: &mut usize, arrays: usize, structs: usize, in_array: bool) -> Result<Type, Error> {
    let bad = |why: &str| Error::Protocol(format!("bad signature \"{}\": {why}", String::from_utf8_lossy(bytes)));
    let Some(&c) = bytes.get(*at) else { return Err(bad("a type is missing")) };
    *at += 1;
    Ok(match c {
        b'y' => Type::Byte,
        b'b' => Type::Boolean,
        b'n' => Type::Int16,
        b'q' => Type::Uint16,
        b'i' => Type::Int32,
        b'u' => Type::Uint32,
        b'x' => Type::Int64,
        b't' => Type::Uint64,
        b'd' => Type::Double,
        b's' => Type::String,
        b'o' => Type::ObjectPath,
        b'g' => Type::Signature,
        b'h' => Type::UnixFd,
        b'v' => Type::Variant,
        b'a' => {
            if arrays + 1 > MAX_DEPTH {
                return Err(bad("arrays nested too deep"));
            }
            Type::Array(Box::new(parse_one(bytes, at, arrays + 1, structs, true)?))
        }
        b'(' => {
            if structs + 1 > MAX_DEPTH {
                return Err(bad("structs nested too deep"));
            }
            let mut fields = Vec::new();
            loop {
                match bytes.get(*at) {
                    Some(b')') => {
                        *at += 1;
                        break;
                    }
                    Some(_) => fields.push(parse_one(bytes, at, arrays, structs + 1, false)?),
                    None => return Err(bad("a struct isn't closed")),
                }
            }
            if fields.is_empty() {
                return Err(bad("an empty struct"));
            }
            Type::Struct(fields)
        }
        b'{' => {
            if !in_array {
                return Err(bad("a dictionary entry outside an array"));
            }
            if structs + 1 > MAX_DEPTH {
                return Err(bad("structs nested too deep"));
            }
            let key = parse_one(bytes, at, arrays, structs + 1, false)?;
            if !key.is_basic() {
                return Err(bad("a dictionary keyed by a container"));
            }
            let value = parse_one(bytes, at, arrays, structs + 1, false)?;
            if bytes.get(*at) != Some(&b'}') {
                return Err(bad("a dictionary entry of other than two types"));
            }
            *at += 1;
            Type::DictEntry(Box::new(key), Box::new(value))
        }
        _ => return Err(bad("an unknown type code")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signatures_parse_and_write_back() {
        for s in
            ["", "s", "a{sv}", "(so)", "a(so)", "((so)(so)(so)iiassusau)", "aai", "a{oa{sa{sv}}}", "ybnqiuxtdsoghv"]
        {
            assert_eq!(signature_of(&Type::parse(s).unwrap()), s);
        }
        assert_eq!(Type::parse("siiva{sv}").unwrap().len(), 5);
        assert_eq!(Type::parse_single("a{sv}").unwrap(), Type::dict(Type::String, Type::Variant));
        assert_eq!(Type::Struct(vec![Type::String, Type::ObjectPath]).alignment(), 8);
    }

    #[test]
    fn bad_signatures_are_refused() {
        for s in ["a", "(", "()", "(s", "{sv}", "a{vs}", "a{s}", "a{sss}", "z", "a{(s)v}"] {
            assert!(Type::parse(s).is_err(), "{s}");
        }
        assert!(Type::parse(&format!("{}i", "a".repeat(32))).is_ok());
        assert!(Type::parse(&format!("{}i", "a".repeat(33))).is_err());
        let deep = format!("{}i{}", "(".repeat(33), ")".repeat(33));
        assert!(Type::parse(&deep).is_err());
        assert!(Type::parse(&"i".repeat(256)).is_err());
        assert!(Type::parse_single("ii").is_err());
    }
}
