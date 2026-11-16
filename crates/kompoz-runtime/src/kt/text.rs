//! Values as Kotlin writes them (`toString()`, a string template's `"$x"`): [`KtToString`], the probe
//! [`template!`](crate::kt::template) goes through, and Java's way of writing a `Float` or a `Double`.

use super::collections::{KtMap, KtSet};
use super::range::{IndexedValue, KtProgression};
use crate::{DerivedState, MutableState, SnapshotStateList, SnapshotStateMap, State, StateSource};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

/// A value's text as Kotlin's `toString()` writes it: numbers as Kotlin does (`3.0`, `1.0E10`, `-0.0`), `null`
/// for an empty `Option`, lists and sets `[a, b]`, maps `{k=v}`, pairs `(a, b)`, strings and chars as they are.
///
/// The extension point: a type of one's own implements it (a Kotlin `data class` gets one written for it); a type
/// of another crate that is `Display` goes through [`KtDisplay`], or reaches a template through the probe's
/// `Display` fallback on its own.
#[diagnostic::on_unimplemented(
    message = "`{Self}` has no Kotlin text form (`toString()`)",
    label = "written as Kotlin writes values here",
    note = "implement `kt::KtToString` for it, or give a `Display` value as `kt::KtDisplay(value)`"
)]
pub trait KtToString {
    /// Appends the text to `out`.
    fn kt_write(&self, out: &mut String);

    /// The text (Kotlin's `toString()`).
    fn kt_to_string(&self) -> String {
        let mut out = String::new();
        self.kt_write(&mut out);
        out
    }

    /// What `String.format` sees of it: a number, a char, a boolean or null by its kind; anything else is only
    /// text (`%s`).
    #[doc(hidden)]
    fn kt_format_arg(&self) -> FormatArg {
        FormatArg::Other
    }
}

/// A value as `String.format` takes it.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FormatArg {
    /// An integer and its width in bits (`%x` of a negative one is its two's complement in that many).
    Int(i128, u32),
    Float(f64),
    Char(char),
    Bool(bool),
    Null,
    Other,
}

/// A `Display` value written by its `Display` (a type of another crate, inside a list or a map).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct KtDisplay<T>(pub T);

impl<T: fmt::Display> KtToString for KtDisplay<T> {
    fn kt_write(&self, out: &mut String) {
        use fmt::Write;
        let _ = write!(out, "{}", self.0);
    }
}

/// The text of `value` (Kotlin's `value.toString()`).
pub fn to_string<T: KtToString + ?Sized>(value: &T) -> String {
    value.kt_to_string()
}

/// What [`template!`](crate::kt::template) asks of each piece: `(&KtProbe(&x)).kt_write_probe(&mut out)` writes
/// it by its [`KtToString`] where it has one, else by its `Display`.
#[doc(hidden)]
pub struct KtProbe<'a, T: ?Sized>(pub &'a T);

#[doc(hidden)]
pub trait KtViaToString {
    fn kt_write_probe(&self, out: &mut String);
}
impl<T: KtToString + ?Sized> KtViaToString for KtProbe<'_, T> {
    fn kt_write_probe(&self, out: &mut String) {
        self.0.kt_write(out);
    }
}

#[doc(hidden)]
pub trait KtViaDisplay {
    fn kt_write_probe(&self, out: &mut String);
}
impl<T: fmt::Display + ?Sized> KtViaDisplay for &KtProbe<'_, T> {
    fn kt_write_probe(&self, out: &mut String) {
        use fmt::Write;
        let _ = write!(out, "{}", self.0);
    }
}

// ---- Numbers ----

/// Writes a `Double` (or, `single`, a `Float`: its shortest digits are a float's) as Java's
/// `Double.toString`/`Float.toString` (JDK 19 and later) and so Kotlin: the shortest digits that read back, plain
/// between 10⁻³ and 10⁷ with at least one digit after the point (`3.0`, `0.001`), else `d.dddE±n` (`1.0E7`,
/// `1.5E-4`); `NaN`, `Infinity`, `-Infinity`, `-0.0`.
pub(crate) fn write_java_float(out: &mut String, value: f64, single: bool) {
    if value.is_nan() {
        out.push_str("NaN");
        return;
    }
    if value.is_infinite() {
        out.push_str(if value < 0.0 { "-Infinity" } else { "Infinity" });
        return;
    }
    if value.is_sign_negative() {
        out.push('-');
    }
    if value == 0.0 {
        out.push_str("0.0");
        return;
    }
    let (digits, exp) = shortest_digits(value.abs(), single);
    let magnitude = value.abs();
    if (1e-3..1e7).contains(&magnitude) {
        if exp >= 0 {
            let whole = exp as usize + 1;
            if digits.len() <= whole {
                out.push_str(&digits);
                out.extend(std::iter::repeat_n('0', whole - digits.len()));
                out.push_str(".0");
            } else {
                out.push_str(&digits[..whole]);
                out.push('.');
                out.push_str(&digits[whole..]);
            }
        } else {
            out.push_str("0.");
            out.extend(std::iter::repeat_n('0', (-exp - 1) as usize));
            out.push_str(&digits);
        }
    } else {
        out.push_str(&digits[..1]);
        out.push('.');
        out.push_str(if digits.len() > 1 { &digits[1..] } else { "0" });
        out.push('E');
        out.push_str(&exp.to_string());
    }
}

/// The shortest decimal digits of a positive finite value that read back as it (as an `f32` when `single`), and
/// the exponent of the first: `(digits, e)` is `0.d₁d₂… × 10^(e+1)`.
pub(crate) fn shortest_digits(value: f64, single: bool) -> (String, i32) {
    let text = if single { format!("{:e}", value as f32) } else { format!("{value:e}") };
    let (mantissa, exp) = text.split_once('e').expect("`{:e}` writes an exponent");
    let digits: String = mantissa.chars().filter(|c| c.is_ascii_digit()).collect();
    let digits = digits.trim_end_matches('0');
    let digits = if digits.is_empty() { "0" } else { digits };
    (digits.to_string(), exp.parse().expect("`{:e}`'s exponent is a number"))
}

macro_rules! ints {
    ($($t:ty: $bits:expr),*) => {$(
        impl KtToString for $t {
            fn kt_write(&self, out: &mut String) {
                use fmt::Write;
                let _ = write!(out, "{self}");
            }
            fn kt_format_arg(&self) -> FormatArg {
                FormatArg::Int(*self as i128, $bits)
            }
        }
    )*};
}
ints!(i8: 8, i16: 16, i32: 32, i64: 64, i128: 128, isize: 64, u8: 8, u16: 16, u32: 32, u64: 64, u128: 128, usize: 64);

impl KtToString for f64 {
    fn kt_write(&self, out: &mut String) {
        write_java_float(out, *self, false);
    }
    fn kt_format_arg(&self) -> FormatArg {
        FormatArg::Float(*self)
    }
}

impl KtToString for f32 {
    fn kt_write(&self, out: &mut String) {
        write_java_float(out, *self as f64, true);
    }
    fn kt_format_arg(&self) -> FormatArg {
        FormatArg::Float(*self as f64)
    }
}

impl KtToString for bool {
    fn kt_write(&self, out: &mut String) {
        out.push_str(if *self { "true" } else { "false" });
    }
    fn kt_format_arg(&self) -> FormatArg {
        FormatArg::Bool(*self)
    }
}

impl KtToString for char {
    fn kt_write(&self, out: &mut String) {
        out.push(*self);
    }
    fn kt_format_arg(&self) -> FormatArg {
        FormatArg::Char(*self)
    }
}

impl KtToString for str {
    fn kt_write(&self, out: &mut String) {
        out.push_str(self);
    }
}

impl KtToString for String {
    fn kt_write(&self, out: &mut String) {
        out.push_str(self);
    }
}

impl KtToString for std::borrow::Cow<'_, str> {
    fn kt_write(&self, out: &mut String) {
        out.push_str(self);
    }
}

/// Kotlin's `Unit`.
impl KtToString for () {
    fn kt_write(&self, out: &mut String) {
        out.push_str("kotlin.Unit");
    }
}

// ---- References and pointers: what they point at ----

macro_rules! through {
    ($($t:ty),*) => {$(
        impl<T: KtToString + ?Sized> KtToString for $t {
            fn kt_write(&self, out: &mut String) {
                (**self).kt_write(out);
            }
            fn kt_format_arg(&self) -> FormatArg {
                (**self).kt_format_arg()
            }
        }
    )*};
}
through!(&T, &mut T, Box<T>, Rc<T>, Arc<T>);

// ---- null ----

/// `null` for none (nested ones too: `Some(None)` is null), else the value.
impl<T: KtToString> KtToString for Option<T> {
    fn kt_write(&self, out: &mut String) {
        match self {
            Some(v) => v.kt_write(out),
            None => out.push_str("null"),
        }
    }
    fn kt_format_arg(&self) -> FormatArg {
        match self {
            Some(v) => v.kt_format_arg(),
            None => FormatArg::Null,
        }
    }
}

// ---- Collections ----

fn write_list<'a, T: KtToString + 'a>(out: &mut String, items: impl IntoIterator<Item = &'a T>) {
    out.push('[');
    for (i, v) in items.into_iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        v.kt_write(out);
    }
    out.push(']');
}

fn write_map<'a, K: KtToString + 'a, V: KtToString + 'a>(
    out: &mut String,
    entries: impl IntoIterator<Item = (&'a K, &'a V)>,
) {
    out.push('{');
    for (i, (k, v)) in entries.into_iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        k.kt_write(out);
        out.push('=');
        v.kt_write(out);
    }
    out.push('}');
}

impl<T: KtToString> KtToString for [T] {
    fn kt_write(&self, out: &mut String) {
        write_list(out, self);
    }
}
impl<T: KtToString, const N: usize> KtToString for [T; N] {
    fn kt_write(&self, out: &mut String) {
        write_list(out, self);
    }
}
impl<T: KtToString> KtToString for Vec<T> {
    fn kt_write(&self, out: &mut String) {
        write_list(out, self);
    }
}
impl<T: KtToString> KtToString for VecDeque<T> {
    fn kt_write(&self, out: &mut String) {
        write_list(out, self);
    }
}
impl<T: KtToString> KtToString for BTreeSet<T> {
    fn kt_write(&self, out: &mut String) {
        write_list(out, self);
    }
}
impl<T: KtToString, S> KtToString for HashSet<T, S> {
    fn kt_write(&self, out: &mut String) {
        write_list(out, self);
    }
}
impl<T: KtToString> KtToString for KtSet<T> {
    fn kt_write(&self, out: &mut String) {
        write_list(out, self.as_slice());
    }
}
impl<K: KtToString, V: KtToString> KtToString for BTreeMap<K, V> {
    fn kt_write(&self, out: &mut String) {
        write_map(out, self);
    }
}
impl<K: KtToString, V: KtToString, S> KtToString for HashMap<K, V, S> {
    fn kt_write(&self, out: &mut String) {
        write_map(out, self);
    }
}
impl<K: KtToString, V: KtToString> KtToString for KtMap<K, V> {
    fn kt_write(&self, out: &mut String) {
        write_map(out, self.as_slice().iter().map(|(k, v)| (k, v)));
    }
}

// ---- Pairs, triples, indexed values ----

/// Kotlin's `Pair`: `(a, b)`.
impl<A: KtToString, B: KtToString> KtToString for (A, B) {
    fn kt_write(&self, out: &mut String) {
        out.push('(');
        self.0.kt_write(out);
        out.push_str(", ");
        self.1.kt_write(out);
        out.push(')');
    }
}

/// Kotlin's `Triple`: `(a, b, c)`.
impl<A: KtToString, B: KtToString, C: KtToString> KtToString for (A, B, C) {
    fn kt_write(&self, out: &mut String) {
        out.push('(');
        self.0.kt_write(out);
        out.push_str(", ");
        self.1.kt_write(out);
        out.push_str(", ");
        self.2.kt_write(out);
        out.push(')');
    }
}

impl<T: KtToString> KtToString for IndexedValue<T> {
    fn kt_write(&self, out: &mut String) {
        out.push_str("IndexedValue(index=");
        self.index.kt_write(out);
        out.push_str(", value=");
        self.value.kt_write(out);
        out.push(')');
    }
}

// ---- Ranges ----

/// `0..9` for a range (`0.0..<1.0` of an open-ended one), `0..9 step 2` and `9 downTo 0 step 1` for a progression
/// (Kotlin's `IntRange`, `OpenEndRange` and `IntProgression`).
impl<T: KtToString> KtToString for KtProgression<T> {
    fn kt_write(&self, out: &mut String) {
        let step = self.step();
        self.start_ref().kt_write(out);
        if self.is_range() {
            out.push_str(if self.is_open_end() { "..<" } else { ".." });
            self.last_ref().kt_write(out);
        } else if step > 0 {
            out.push_str("..");
            self.last_ref().kt_write(out);
            out.push_str(" step ");
            out.push_str(&step.to_string());
        } else {
            out.push_str(" downTo ");
            self.last_ref().kt_write(out);
            out.push_str(" step ");
            out.push_str(&(-(step as i128)).to_string());
        }
    }
}

// ---- The runtime's states, as Compose writes them ----

/// Its identity hash, as Kotlin's `hashCode()` of an object without one of its own.
fn identity_hash(p: *const ()) -> i32 {
    ((p as usize as u64 ^ (p as usize as u64 >> 32)) as u32 & 0x7fff_ffff) as i32
}

/// `MutableState(value=…)@hash` (Compose's SnapshotMutableStateImpl): its value read without subscribing.
impl<T: KtToString + Clone + Send + Sync + 'static> KtToString for MutableState<T> {
    fn kt_write(&self, out: &mut String) {
        out.push_str("MutableState(value=");
        self.peek_with(|v| v.kt_write(out));
        out.push_str(")@");
        out.push_str(&identity_hash(StateSource::identity(self)).to_string());
    }
}

/// `DerivedState(value=…)@hash` (Compose's DerivedSnapshotState).
impl<T: KtToString + Clone + 'static> KtToString for DerivedState<T> {
    fn kt_write(&self, out: &mut String) {
        out.push_str("DerivedState(value=");
        self.peek().kt_write(out);
        out.push_str(")@");
        out.push_str(&identity_hash(StateSource::identity(self)).to_string());
    }
}

/// `State(value=…)@hash`.
impl<T: KtToString + Clone + 'static> KtToString for State<T> {
    fn kt_write(&self, out: &mut String) {
        out.push_str("State(value=");
        self.peek().kt_write(out);
        out.push_str(")@");
        out.push_str(&identity_hash(self.identity()).to_string());
    }
}

/// `SnapshotStateList(value=[…])@hash`.
impl<T: KtToString + Clone + Send + Sync + 'static> KtToString for SnapshotStateList<T> {
    fn kt_write(&self, out: &mut String) {
        out.push_str("SnapshotStateList(value=");
        self.peek_with(|items| write_list(out, items));
        out.push_str(")@");
        out.push_str(&identity_hash(self.identity()).to_string());
    }
}

/// `SnapshotStateMap(value={…})@hash`.
impl<K: KtToString + Ord + Clone + Send + Sync + 'static, V: KtToString + Clone + Send + Sync + 'static> KtToString
    for SnapshotStateMap<K, V>
{
    fn kt_write(&self, out: &mut String) {
        out.push_str("SnapshotStateMap(value=");
        self.peek_with(|map| write_map(out, map));
        out.push_str(")@");
        out.push_str(&identity_hash(self.identity()).to_string());
    }
}
