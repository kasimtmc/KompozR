//! What `remember_saveable` writes out so that state outlives the process (Android ends apps in the background
//! and brings them back with their saved instance state): a small tree of values, encoded as bytes.

use std::collections::BTreeMap;
use std::sync::Arc;

/// A saved value: nothing, a bool, a number, a text, or a list or map of saved values.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum SavedValue {
    #[default]
    Null,
    Bool(bool),
    Int(i64),
    Double(f64),
    String(String),
    List(Arc<Vec<SavedValue>>),
    Map(Arc<BTreeMap<String, SavedValue>>),
}

impl SavedValue {
    pub fn list(items: impl IntoIterator<Item = SavedValue>) -> SavedValue {
        SavedValue::List(Arc::new(items.into_iter().collect()))
    }
    pub fn map<K: Into<String>>(entries: impl IntoIterator<Item = (K, SavedValue)>) -> SavedValue {
        SavedValue::Map(Arc::new(entries.into_iter().map(|(k, v)| (k.into(), v)).collect()))
    }

    pub fn is_null(&self) -> bool {
        matches!(self, SavedValue::Null)
    }
    pub fn as_bool(&self) -> Option<bool> {
        if let SavedValue::Bool(b) = self { Some(*b) } else { None }
    }
    pub fn as_int(&self) -> Option<i64> {
        if let SavedValue::Int(n) = self { Some(*n) } else { None }
    }
    /// A number (an integer too).
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            SavedValue::Double(d) => Some(*d),
            SavedValue::Int(n) => Some(*n as f64),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        if let SavedValue::String(s) = self { Some(s) } else { None }
    }
    pub fn as_list(&self) -> Option<&[SavedValue]> {
        if let SavedValue::List(l) = self { Some(l) } else { None }
    }
    pub fn as_map(&self) -> Option<&BTreeMap<String, SavedValue>> {
        if let SavedValue::Map(m) = self { Some(m) } else { None }
    }
    /// The value of `key` in a map (nothing if it isn't a map with it).
    pub fn get(&self, key: &str) -> &SavedValue {
        static NONE: SavedValue = SavedValue::Null;
        self.as_map().and_then(|m| m.get(key)).unwrap_or(&NONE)
    }

    /// A mark and a version, then a tag byte per value: integers zigzag varints, doubles 8 bytes, strings and
    /// lists and maps a varint length, then their contents.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = vec![b'K', b'Z', 1];
        encode_to(self, &mut out);
        out
    }

    pub fn decode(bytes: &[u8]) -> Option<SavedValue> {
        if bytes.len() < 4 || bytes[..3] != [b'K', b'Z', 1] {
            return None;
        }
        let mut r = Reader { bytes: &bytes[3..], depth: 0 };
        let v = r.value()?;
        r.bytes.is_empty().then_some(v)
    }
}

const NULL: u8 = 0;
const FALSE: u8 = 1;
const TRUE: u8 = 2;
const INTEGER: u8 = 3;
const DOUBLE: u8 = 4;
const STRING: u8 = 5;
const LIST: u8 = 6;
const MAP: u8 = 7;

fn put_varint(out: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        out.push(v as u8 | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

fn put_string(out: &mut Vec<u8>, s: &str) {
    put_varint(out, s.len() as u64);
    out.extend_from_slice(s.as_bytes());
}

fn encode_to(v: &SavedValue, out: &mut Vec<u8>) {
    match v {
        SavedValue::Null => out.push(NULL),
        SavedValue::Bool(b) => out.push(if *b { TRUE } else { FALSE }),
        SavedValue::Int(n) => {
            out.push(INTEGER);
            put_varint(out, ((*n as u64) << 1) ^ ((*n >> 63) as u64));
        }
        SavedValue::Double(d) => {
            out.push(DOUBLE);
            out.extend_from_slice(&d.to_bits().to_le_bytes());
        }
        SavedValue::String(s) => {
            out.push(STRING);
            put_string(out, s);
        }
        SavedValue::List(l) => {
            out.push(LIST);
            put_varint(out, l.len() as u64);
            for item in l.iter() {
                encode_to(item, out);
            }
        }
        SavedValue::Map(m) => {
            out.push(MAP);
            put_varint(out, m.len() as u64);
            for (k, item) in m.iter() {
                put_string(out, k);
                encode_to(item, out);
            }
        }
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    depth: u32,
}

impl Reader<'_> {
    fn byte(&mut self) -> Option<u8> {
        let (b, rest) = self.bytes.split_first()?;
        self.bytes = rest;
        Some(*b)
    }
    fn varint(&mut self) -> Option<u64> {
        let mut v = 0u64;
        for shift in (0..64).step_by(7) {
            let b = self.byte()?;
            v |= ((b & 0x7f) as u64) << shift;
            if b & 0x80 == 0 {
                return Some(v);
            }
        }
        None
    }
    fn string(&mut self) -> Option<String> {
        let n = self.varint()? as usize;
        if n > self.bytes.len() {
            return None;
        }
        let (s, rest) = self.bytes.split_at(n);
        self.bytes = rest;
        String::from_utf8(s.to_vec()).ok()
    }
    fn value(&mut self) -> Option<SavedValue> {
        self.depth += 1;
        if self.depth > 64 {
            return None;
        }
        let v = match self.byte()? {
            NULL => SavedValue::Null,
            FALSE => SavedValue::Bool(false),
            TRUE => SavedValue::Bool(true),
            INTEGER => {
                let z = self.varint()?;
                SavedValue::Int((z >> 1) as i64 ^ -((z & 1) as i64))
            }
            DOUBLE => {
                if self.bytes.len() < 8 {
                    return None;
                }
                let (b, rest) = self.bytes.split_at(8);
                self.bytes = rest;
                SavedValue::Double(f64::from_bits(u64::from_le_bytes(b.try_into().unwrap())))
            }
            STRING => SavedValue::String(self.string()?),
            LIST => {
                let n = self.varint()?;
                if n > self.bytes.len() as u64 {
                    return None;
                }
                let mut list = Vec::with_capacity(n as usize);
                for _ in 0..n {
                    list.push(self.value()?);
                }
                SavedValue::List(Arc::new(list))
            }
            MAP => {
                let n = self.varint()?;
                if n > self.bytes.len() as u64 {
                    return None;
                }
                let mut map = BTreeMap::new();
                for _ in 0..n {
                    let k = self.string()?;
                    map.insert(k, self.value()?);
                }
                SavedValue::Map(Arc::new(map))
            }
            _ => return None,
        };
        self.depth -= 1;
        Some(v)
    }
}

impl From<bool> for SavedValue {
    fn from(b: bool) -> Self {
        SavedValue::Bool(b)
    }
}
impl From<String> for SavedValue {
    fn from(s: String) -> Self {
        SavedValue::String(s)
    }
}
impl From<&str> for SavedValue {
    fn from(s: &str) -> Self {
        SavedValue::String(s.to_string())
    }
}
impl From<f64> for SavedValue {
    fn from(d: f64) -> Self {
        SavedValue::Double(d)
    }
}
impl From<f32> for SavedValue {
    fn from(d: f32) -> Self {
        SavedValue::Double(d as f64)
    }
}
macro_rules! ints {
    ($($t:ty)*) => {$(
        impl From<$t> for SavedValue {
            fn from(n: $t) -> Self {
                SavedValue::Int(n as i64)
            }
        }
    )*};
}
ints!(i8 i16 i32 i64 isize u8 u16 u32 u64 usize);

/// A type written out and read back by `remember_saveable` (Compose's autoSaver types, or a `Saver`): numbers,
/// bools, strings, and vectors and options of saveable types are; so are the library's states.
pub trait Saveable: Sized {
    fn save(&self) -> SavedValue;
    /// `None` if it can't: then the value is made anew.
    fn restore(saved: &SavedValue) -> Option<Self>;
}

impl Saveable for bool {
    fn save(&self) -> SavedValue {
        SavedValue::Bool(*self)
    }
    fn restore(saved: &SavedValue) -> Option<Self> {
        saved.as_bool()
    }
}

macro_rules! saveable_ints {
    ($($t:ty)*) => {$(
        impl Saveable for $t {
            fn save(&self) -> SavedValue {
                SavedValue::Int(*self as i64)
            }
            fn restore(saved: &SavedValue) -> Option<Self> {
                saved.as_int().and_then(|n| <$t>::try_from(n).ok())
            }
        }
    )*};
}
saveable_ints!(i8 i16 i32 i64 isize u8 u16 u32 u64 usize);

impl Saveable for f32 {
    fn save(&self) -> SavedValue {
        SavedValue::Double(*self as f64)
    }
    fn restore(saved: &SavedValue) -> Option<Self> {
        saved.as_f64().map(|d| d as f32)
    }
}
impl Saveable for f64 {
    fn save(&self) -> SavedValue {
        SavedValue::Double(*self)
    }
    fn restore(saved: &SavedValue) -> Option<Self> {
        saved.as_f64()
    }
}
impl Saveable for String {
    fn save(&self) -> SavedValue {
        SavedValue::String(self.clone())
    }
    fn restore(saved: &SavedValue) -> Option<Self> {
        saved.as_str().map(str::to_string)
    }
}
impl<T: Saveable> Saveable for Vec<T> {
    fn save(&self) -> SavedValue {
        SavedValue::list(self.iter().map(T::save))
    }
    fn restore(saved: &SavedValue) -> Option<Self> {
        saved.as_list()?.iter().map(T::restore).collect()
    }
}
impl<T: Saveable> Saveable for Option<T> {
    fn save(&self) -> SavedValue {
        SavedValue::list(self.iter().map(T::save))
    }
    fn restore(saved: &SavedValue) -> Option<Self> {
        match saved.as_list()? {
            [] => Some(None),
            [v] => T::restore(v).map(Some),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_values_encode_and_decode() {
        let v = SavedValue::map([
            ("n", SavedValue::from(-5)),
            ("d", SavedValue::from(2.5)),
            ("s", SavedValue::from("çay")),
            ("l", SavedValue::list([true.into(), SavedValue::Null, i64::MIN.into()])),
        ]);
        let bytes = v.encode();
        assert_eq!(SavedValue::decode(&bytes), Some(v.clone()));
        assert_eq!(v.get("s").as_str(), Some("çay"));
        assert!(v.get("none").is_null());
        assert_eq!(SavedValue::decode(&bytes[..bytes.len() - 1]), None);
        assert_eq!(SavedValue::decode(b"KZ\x02\x00"), None);
        assert_eq!(Vec::<Option<i32>>::restore(&vec![Some(1), None].save()), Some(vec![Some(1), None]));
        assert_eq!(u8::restore(&SavedValue::Int(300)), None);
    }
}
