//! libwayland-client (and libwayland-cursor), loaded at run time as C++ Kompoz loads them (`dlopen`: a binary that
//! runs where they aren't installed, on X11 or headless), and the runtime the generated protocols
//! ([`generated`](super::generated)) are written over. libwayland stays (the Vulkan driver's WSI speaks through our
//! display and surface: `VK_KHR_wayland_surface` takes libwayland's own pointers), but nothing of its C API reaches
//! the platform:
//!
//! - the protocols' tables are ours ([`Interface`], [`Message`]: `wl_interface` and `wl_message` as libwayland reads
//!   them, made by the generator);
//! - requests are marshalled by [`Proxy`] through `wl_proxy_marshal_array_flags` (libwayland 1.20+), each argument
//!   checked against the message's signature first (a mismatch, a null where none may be, a request newer than the
//!   object's version: not sent), so that the generated functions are safe;
//! - every object's events go through one dispatcher (`wl_proxy_add_dispatcher`), which only decodes their
//!   arguments into owned values (strings copied, fds owned, new ids wrapped as objects with the dispatcher on
//!   them) and queues them on the [`Connection`]: no platform code runs inside libwayland's dispatch, nothing
//!   unwinds through it. The platform reads the queue afterwards ([`Connection::take_event`],
//!   [`Event::decode`](super::generated::Event::decode)).
//! - objects are owned: dropping one sends its destructor request (if its version has one) and destroys the proxy.
//!   Each holds its connection weakly: once the display is disconnected, an object left over is inert.
//!
//! The only `unsafe` code of the Wayland platform besides its Vulkan surface (`vulkan`), each block saying why it
//! holds.

#![allow(unsafe_code)]

use super::generated::{WL_DISPLAY_INTERFACE, WlBuffer, WlDisplay, WlShm};
use crate::linux::sys::Library;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::ffi::{CStr, CString, c_char, c_int, c_uint, c_void};
use std::fmt;
use std::io;
use std::marker::PhantomData;
use std::os::fd::{AsRawFd, BorrowedFd, FromRawFd, OwnedFd, RawFd};
use std::ptr::{self, NonNull};
use std::rc::{Rc, Weak};
use std::sync::OnceLock;

type Ptr = *mut c_void;

// ---- The protocols' tables ----------------------------------------------------------------------------------------

/// An interface's table: libwayland's `wl_interface` (its fields first, as libwayland reads them through a pointer),
/// then KompozR's own (the request destroying an object of it), which libwayland never sees. Made by the generator,
/// from `'static` data only.
#[repr(C)]
pub struct Interface {
    name: *const c_char,
    version: c_int,
    method_count: c_int,
    methods: *const Message,
    event_count: c_int,
    events: *const Message,
    /// The opcode of the argumentless destructor request; -1: none.
    destructor: i32,
}

// SAFETY: its pointers are to immutable `'static` data (names, signatures, messages), readable from any thread.
unsafe impl Sync for Interface {}

impl Interface {
    pub const fn new(
        name: &'static CStr,
        version: i32,
        requests: &'static [Message],
        events: &'static [Message],
        destructor: Option<u16>,
    ) -> Interface {
        Interface {
            name: name.as_ptr(),
            version,
            method_count: requests.len() as c_int,
            methods: requests.as_ptr(),
            event_count: events.len() as c_int,
            events: events.as_ptr(),
            destructor: match destructor {
                Some(opcode) => opcode as i32,
                None => -1,
            },
        }
    }

    pub fn name(&self) -> &'static CStr {
        // SAFETY: `new` took it from a `&'static CStr`.
        unsafe { CStr::from_ptr(self.name) }
    }

    /// The highest version the protocol's XML has.
    pub fn version(&self) -> u32 {
        self.version as u32
    }

    pub fn requests(&self) -> &'static [Message] {
        // SAFETY: `new` took pointer and length from a `&'static [Message]`.
        unsafe { std::slice::from_raw_parts(self.methods, self.method_count as usize) }
    }

    pub fn events(&self) -> &'static [Message] {
        // SAFETY: as `requests`.
        unsafe { std::slice::from_raw_parts(self.events, self.event_count as usize) }
    }

    /// The request destroying an object of it (its opcode), if it has an argumentless one.
    pub fn destructor(&self) -> Option<u16> {
        u16::try_from(self.destructor).ok()
    }
}

impl fmt::Debug for Interface {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Interface({})", self.name().to_string_lossy())
    }
}

/// A request's or an event's table: libwayland's `wl_message` (exactly: libwayland indexes arrays of them). Its
/// signature is libwayland's (since-version digits, then a character for each argument, `?` before a nullable one);
/// its types the interfaces of its object and new-id arguments, one for each signature argument.
#[repr(C)]
pub struct Message {
    name: *const c_char,
    signature: *const c_char,
    types: *const Option<&'static Interface>,
}

// SAFETY: as Interface's: pointers to immutable `'static` data.
unsafe impl Sync for Message {}

/// The arguments a signature has (a nullable mark and the since-version aren't).
const fn argument_count(signature: &[u8]) -> usize {
    let mut n = 0;
    let mut i = 0;
    while i < signature.len() {
        if matches!(signature[i], b'i' | b'u' | b'f' | b's' | b'o' | b'n' | b'a' | b'h') {
            n += 1;
        }
        i += 1;
    }
    n
}

impl Message {
    /// A message's table; `types` must have an entry for each argument of `signature` (checked when it is made: a
    /// generated table that hasn't fails to compile).
    pub const fn new(
        name: &'static CStr,
        signature: &'static CStr,
        types: &'static [Option<&'static Interface>],
    ) -> Message {
        let n = argument_count(signature.to_bytes());
        assert!(n == 0 || types.len() == n, "a message's types are its arguments'");
        Message { name: name.as_ptr(), signature: signature.as_ptr(), types: types.as_ptr() }
    }

    pub fn name(&self) -> &'static CStr {
        // SAFETY: `new` took it from a `&'static CStr`.
        unsafe { CStr::from_ptr(self.name) }
    }

    pub fn signature(&self) -> &'static CStr {
        // SAFETY: as `name`.
        unsafe { CStr::from_ptr(self.signature) }
    }

    /// The version of the interface it came in (1 unless the signature says).
    pub fn since(&self) -> u32 {
        let digits: String =
            self.signature().to_bytes().iter().take_while(|b| b.is_ascii_digit()).map(|&b| b as char).collect();
        digits.parse().unwrap_or(1).max(1)
    }

    /// Its arguments: each one's type character and whether it may be null.
    pub fn arguments(&self) -> Vec<(u8, bool)> {
        let mut out = Vec::new();
        let mut nullable = false;
        for &b in self.signature().to_bytes() {
            match b {
                b'?' => nullable = true,
                b'0'..=b'9' => {}
                _ => {
                    out.push((b, nullable));
                    nullable = false;
                }
            }
        }
        out
    }

    /// The interface of each argument (its object's or new id's; none for the others).
    pub fn types(&self) -> &'static [Option<&'static Interface>] {
        let n = argument_count(self.signature().to_bytes());
        if n == 0 {
            return &[];
        }
        // SAFETY: `new` checked that its `'static` types slice has an entry for each of the signature's arguments.
        unsafe { std::slice::from_raw_parts(self.types, n) }
    }
}

impl fmt::Debug for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Message({} \"{}\")", self.name().to_string_lossy(), self.signature().to_string_lossy())
    }
}

/// `union wl_argument`.
#[repr(C)]
#[derive(Clone, Copy)]
union Argument {
    i: i32,
    u: u32,
    f: i32,
    s: *const c_char,
    o: Ptr,
    n: u32,
    a: *mut WlArray,
    h: RawFd,
}

/// `struct wl_array`.
#[repr(C)]
struct WlArray {
    size: usize,
    alloc: usize,
    data: Ptr,
}

/// `wl_fixed_t` (24.8 fixed point) as a number.
pub fn fixed_to_f64(f: i32) -> f64 {
    f as f64 / 256.0
}

/// A number as `wl_fixed_t` (rounded to the nearest 256th; saturating).
pub fn fixed_from_f64(v: f64) -> i32 {
    (v * 256.0).round() as i32
}

/// `s` as a C string for a request (cut at a NUL inside it, which a C string can't hold).
pub fn c_string(s: &str) -> CString {
    let s = s.split('\0').next().unwrap_or_default();
    CString::new(s).unwrap_or_default()
}

// ---- libwayland-client ----------------------------------------------------------------------------------------------

/// `wl_dispatcher_func_t`.
type Dispatcher = unsafe extern "C" fn(*const c_void, Ptr, u32, *const Message, *mut Argument) -> c_int;

/// `WL_MARSHAL_FLAG_DESTROY`.
const MARSHAL_FLAG_DESTROY: u32 = 1;

/// libwayland-client's functions, looked up once.
struct Lib {
    display_connect: unsafe extern "C" fn(*const c_char) -> Ptr,
    display_disconnect: unsafe extern "C" fn(Ptr),
    display_get_fd: unsafe extern "C" fn(Ptr) -> c_int,
    display_dispatch_pending: unsafe extern "C" fn(Ptr) -> c_int,
    display_roundtrip: unsafe extern "C" fn(Ptr) -> c_int,
    display_flush: unsafe extern "C" fn(Ptr) -> c_int,
    display_prepare_read: unsafe extern "C" fn(Ptr) -> c_int,
    display_read_events: unsafe extern "C" fn(Ptr) -> c_int,
    display_cancel_read: unsafe extern "C" fn(Ptr),
    display_get_error: unsafe extern "C" fn(Ptr) -> c_int,
    display_get_protocol_error: unsafe extern "C" fn(Ptr, *mut *const c_void, *mut u32) -> u32,
    proxy_marshal_array_flags: unsafe extern "C" fn(Ptr, u32, *const Interface, u32, u32, *mut Argument) -> Ptr,
    proxy_add_dispatcher: unsafe extern "C" fn(Ptr, Dispatcher, *const c_void, Ptr) -> c_int,
    proxy_destroy: unsafe extern "C" fn(Ptr),
    proxy_get_version: unsafe extern "C" fn(Ptr) -> u32,
    proxy_get_user_data: unsafe extern "C" fn(Ptr) -> Ptr,
    // (Last: closed after the functions are gone; never, as the library is kept for the process.)
    _library: Library,
}

/// Looks `name` up in `library` as a function pointer of type `F`.
///
/// # Safety
/// `F` must be the symbol's C signature as an `unsafe extern "C" fn`.
unsafe fn function<F: Copy>(library: &Library, name: &CStr) -> Result<F, String> {
    let p = library.symbol(name).ok_or_else(|| format!("no {} in the library", name.to_string_lossy()))?;
    debug_assert_eq!(std::mem::size_of::<F>(), std::mem::size_of::<Ptr>());
    // SAFETY: the caller says `F` is the symbol's signature, a function pointer as big as `p`.
    Ok(unsafe { std::mem::transmute_copy::<Ptr, F>(&p) })
}

impl Lib {
    fn load() -> Result<Lib, String> {
        let library = Library::open(&["libwayland-client.so.0", "libwayland-client.so"])?;
        let l = &library;
        // SAFETY: each `function` call's type is wayland-client-core.h's signature of its symbol, as `Lib` declares
        // it (pointers to libwayland's structs as `Ptr`; `wl_interface` and `wl_message` as ours, the same layout).
        unsafe {
            Ok(Lib {
                display_connect: function(l, c"wl_display_connect")?,
                display_disconnect: function(l, c"wl_display_disconnect")?,
                display_get_fd: function(l, c"wl_display_get_fd")?,
                display_dispatch_pending: function(l, c"wl_display_dispatch_pending")?,
                display_roundtrip: function(l, c"wl_display_roundtrip")?,
                display_flush: function(l, c"wl_display_flush")?,
                display_prepare_read: function(l, c"wl_display_prepare_read")?,
                display_read_events: function(l, c"wl_display_read_events")?,
                display_cancel_read: function(l, c"wl_display_cancel_read")?,
                display_get_error: function(l, c"wl_display_get_error")?,
                display_get_protocol_error: function(l, c"wl_display_get_protocol_error")?,
                proxy_marshal_array_flags: function(l, c"wl_proxy_marshal_array_flags")?,
                proxy_add_dispatcher: function(l, c"wl_proxy_add_dispatcher")?,
                proxy_destroy: function(l, c"wl_proxy_destroy")?,
                proxy_get_version: function(l, c"wl_proxy_get_version")?,
                proxy_get_user_data: function(l, c"wl_proxy_get_user_data")?,
                _library: library,
            })
        }
    }
}

/// libwayland-client, loaded once and kept for the process (its objects, and the Vulkan driver's, may outlive any
/// one connection's owner); why not if it can't be.
fn lib() -> Result<&'static Lib, String> {
    static LIB: OnceLock<Result<Lib, String>> = OnceLock::new();
    LIB.get_or_init(Lib::load).as_ref().map_err(Clone::clone)
}

/// Whether libwayland-client can be loaded here.
pub fn available() -> bool {
    lib().is_ok()
}

// ---- Objects --------------------------------------------------------------------------------------------------------

/// An object's identity: its proxy's address (unique while it lives), what events name objects by. Null: none.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct ObjectId(usize);

impl ObjectId {
    pub const NULL: ObjectId = ObjectId(0);

    pub fn is_null(self) -> bool {
        self.0 == 0
    }
}

/// An object of an interface, as the generated types are: the proxy it owns.
pub trait Object: Sized {
    const INTERFACE: &'static Interface;
    /// The object a proxy of this interface is (the generated code's: other code gets objects from requests and
    /// events).
    fn from_proxy(proxy: Proxy) -> Self;
    fn proxy(&self) -> &Proxy;
    fn id(&self) -> ObjectId {
        self.proxy().id()
    }
    /// The version it was made with (0: libwayland's display, which has none).
    fn version(&self) -> u32 {
        self.proxy().version()
    }
}

/// A request's argument, as the generated functions give them (checked against the message's signature).
#[derive(Clone, Copy)]
pub enum Arg<'a> {
    Int(i32),
    Uint(u32),
    Fixed(f64),
    Str(Option<&'a CStr>),
    Object(Option<&'a Proxy>),
    /// The object the request makes (its interface the message's, or the one asked for: `create_versioned`).
    NewId,
    Array(&'a [u8]),
    /// Duplicated by libwayland when it is marshalled: still the caller's after.
    Fd(BorrowedFd<'a>),
}

/// A `wl_proxy`: an object on a connection, its events queued there. Owned (destroyed when dropped, its destructor
/// request sent first if it has one) unless it is libwayland's or another library's (the display, a cursor theme's
/// buffers), which are only borrowed.
pub struct Proxy {
    raw: Cell<Ptr>,
    interface: &'static Interface,
    connection: Weak<Inner>,
    owned: bool,
}

impl fmt::Debug for Proxy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{:p}", self.interface.name().to_string_lossy(), self.raw.get())
    }
}

impl Proxy {
    /// A proxy libwayland made (`raw`, of `interface`, on `connection`), owned from now on, its events queued on the
    /// connection.
    ///
    /// # Safety
    /// `raw` must be a live `wl_proxy` of `connection`'s display whose interface is `interface`, with no dispatcher or
    /// listener yet, owned by no one else.
    unsafe fn adopt(raw: Ptr, interface: &'static Interface, connection: &Rc<Inner>) -> Proxy {
        if interface.event_count > 0 {
            // SAFETY: as the caller says; the dispatcher's data is the connection's `Inner`, alive while the proxy is
            // dispatched (dispatching happens only through a live `Connection`).
            unsafe {
                (connection.lib.proxy_add_dispatcher)(
                    raw,
                    dispatch,
                    ptr::from_ref(interface).cast(),
                    Rc::as_ptr(connection).cast_mut().cast(),
                )
            };
        }
        Proxy { raw: Cell::new(raw), interface, connection: Rc::downgrade(connection), owned: true }
    }

    /// A proxy another library owns (`raw`, of `interface`), borrowed: never destroyed here, its events not ours.
    ///
    /// # Safety
    /// `raw` must be a `wl_proxy` of `connection`'s display whose interface is `interface`, alive while the result
    /// is used.
    unsafe fn borrowed(raw: Ptr, interface: &'static Interface, connection: &Rc<Inner>) -> Proxy {
        Proxy { raw: Cell::new(raw), interface, connection: Rc::downgrade(connection), owned: false }
    }

    /// The connection and the proxy, while both live.
    fn live(&self) -> Option<(Rc<Inner>, Ptr)> {
        let raw = self.raw.get();
        if raw.is_null() {
            return None;
        }
        Some((self.connection.upgrade()?, raw))
    }

    pub fn id(&self) -> ObjectId {
        ObjectId(self.raw.get() as usize)
    }

    pub fn interface(&self) -> &'static Interface {
        self.interface
    }

    /// The version it was made with (0: the display's, or an object of the display's, which have none).
    pub fn version(&self) -> u32 {
        let Some((connection, raw)) = self.live() else { return 0 };
        // SAFETY: a live proxy of a live display.
        unsafe { (connection.lib.proxy_get_version)(raw) }
    }

    /// The `wl_proxy*` (what a C library takes: the Vulkan driver's WSI); null once destroyed.
    pub fn as_ptr(&self) -> Ptr {
        self.raw.get()
    }

    /// Sends request `opcode` (the generated functions'): not if an argument isn't what the signature says or the
    /// object is older than the request.
    pub fn send(&self, opcode: u16, args: &[Arg]) {
        let Some((connection, raw)) = self.live() else { return };
        marshal(&connection, raw, self.interface, opcode, args, None, false);
    }

    /// Sends request `opcode`, which makes a `T` (of the object's version); the object made.
    pub fn create<T: Object>(&self, opcode: u16, args: &[Arg]) -> Option<T> {
        let version = self.version();
        self.create_versioned(opcode, version, args)
    }

    /// Sends request `opcode`, which makes a `T` of `version` (`wl_registry.bind`'s); the object made.
    pub fn create_versioned<T: Object>(&self, opcode: u16, version: u32, args: &[Arg]) -> Option<T> {
        let (connection, raw) = self.live()?;
        let made = marshal(&connection, raw, self.interface, opcode, args, Some((T::INTERFACE, version)), false)?;
        // SAFETY: libwayland just made it, of T's interface, for this display; no one else has it.
        (!made.is_null()).then(|| T::from_proxy(unsafe { Proxy::adopt(made, T::INTERFACE, &connection) }))
    }

    /// Sends destructor request `opcode` and destroys the proxy (else, if it can't be sent, only destroys it).
    pub fn destroy_with(self, opcode: u16, args: &[Arg]) {
        let Some((connection, raw)) = self.live() else { return };
        if self.owned && marshal(&connection, raw, self.interface, opcode, args, None, true).is_some() {
            self.raw.set(ptr::null_mut()); // (libwayland destroyed it)
        }
    }
}

impl Drop for Proxy {
    fn drop(&mut self) {
        let Some((connection, raw)) = self.live() else { return };
        self.raw.set(ptr::null_mut());
        if !self.owned {
            return;
        }
        if let Some(opcode) = self.interface.destructor()
            && marshal(&connection, raw, self.interface, opcode, &[], None, true).is_some()
        {
            return;
        }
        // SAFETY: a live proxy of a live display, owned by us and not used after.
        unsafe { (connection.lib.proxy_destroy)(raw) };
    }
}

/// Request `opcode` of `raw` (of `interface`) marshalled with `args`, checked against its signature first; `new`:
/// the interface and version of the object it makes; `destroy`: the proxy destroyed after. What libwayland returned
/// (the new proxy, or null), or none if it wasn't sent.
fn marshal(
    connection: &Inner,
    raw: Ptr,
    interface: &'static Interface,
    opcode: u16,
    args: &[Arg],
    new: Option<(&'static Interface, u32)>,
    destroy: bool,
) -> Option<Ptr> {
    let lib = connection.lib;
    let message = interface.requests().get(opcode as usize)?;
    // SAFETY: a live proxy of a live display (the caller's).
    let version = unsafe { (lib.proxy_get_version)(raw) };
    if version != 0 && version < message.since() {
        return None; // (a protocol error: the object's version hasn't it)
    }
    let spec = message.arguments();
    let types = message.types();
    if spec.len() != args.len() {
        debug_assert!(false, "{message:?}: {} arguments given", args.len());
        return None;
    }
    // The arrays first, where they stay while their arguments point at them.
    let mut arrays: Vec<WlArray> = args
        .iter()
        .filter_map(|a| match a {
            Arg::Array(bytes) => {
                Some(WlArray { size: bytes.len(), alloc: bytes.len(), data: bytes.as_ptr().cast_mut().cast() })
            }
            _ => None,
        })
        .collect();
    let mut next_array = 0;
    let mut makes = false;
    let mut c = Vec::with_capacity(args.len());
    for (i, (&(ty, nullable), arg)) in spec.iter().zip(args).enumerate() {
        c.push(match (ty, arg) {
            (b'i', &Arg::Int(v)) => Argument { i: v },
            (b'u', &Arg::Uint(v)) => Argument { u: v },
            (b'f', &Arg::Fixed(v)) => Argument { f: fixed_from_f64(v) },
            (b's', &Arg::Str(s)) => match s {
                Some(s) => Argument { s: s.as_ptr() },
                None if nullable => Argument { s: ptr::null() },
                None => return None,
            },
            (b'o', &Arg::Object(o)) => {
                // (A destroyed object, or one of another connection, as none.)
                let p =
                    o.filter(|o| o.connection.ptr_eq(&connection.this)).map(|o| o.raw.get()).filter(|p| !p.is_null());
                match p {
                    Some(p) => Argument { o: p },
                    None if nullable => Argument { o: ptr::null_mut() },
                    None => return None,
                }
            }
            (b'n', Arg::NewId) => {
                let (made, _) = new?;
                if types[i].is_some_and(|t| !ptr::eq(t, made)) {
                    return None;
                }
                makes = true;
                Argument { o: ptr::null_mut() }
            }
            (b'a', Arg::Array(_)) => {
                let a = &mut arrays[next_array];
                next_array += 1;
                Argument { a: ptr::from_mut(a) }
            }
            (b'h', Arg::Fd(fd)) => Argument { h: fd.as_raw_fd() },
            _ => return None,
        });
    }
    if makes != new.is_some() {
        return None;
    }
    let (new_interface, new_version) = match new {
        Some((i, v)) => (ptr::from_ref(i), v),
        None => (ptr::null(), version),
    };
    let flags = if destroy { MARSHAL_FLAG_DESTROY } else { 0 };
    // SAFETY: a live proxy of a live display; `c` holds an argument for each of the message's, of the type its
    // signature says (checked above), their strings, arrays and objects alive for the call; the new id's interface is
    // a `'static` table.
    Some(unsafe {
        (lib.proxy_marshal_array_flags)(raw, opcode as u32, new_interface, new_version, flags, c.as_mut_ptr())
    })
}

// ---- Events --------------------------------------------------------------------------------------------------------

/// An event's argument, decoded and owned.
#[derive(Debug)]
pub enum Value {
    Int(i32),
    Uint(u32),
    /// `wl_fixed_t`.
    Fixed(i32),
    Str(Option<String>),
    Object(Option<ObjectId>),
    /// The object the event made (none if libwayland couldn't).
    NewId(Option<Proxy>),
    Array(Vec<u8>),
    Fd(Option<OwnedFd>),
}

/// An event as queued: who sent it (of which interface), which one, its arguments.
#[derive(Debug)]
pub struct RawEvent {
    pub sender: ObjectId,
    pub interface: &'static Interface,
    pub opcode: u16,
    pub args: Vec<Value>,
}

/// An event's arguments, read in order by the generated decoders (each read none if the argument isn't that).
pub struct Args(std::vec::IntoIter<Value>);

impl Args {
    pub fn new(args: Vec<Value>) -> Args {
        Args(args.into_iter())
    }

    pub fn int(&mut self) -> Option<i32> {
        match self.0.next()? {
            Value::Int(v) => Some(v),
            _ => None,
        }
    }

    pub fn uint(&mut self) -> Option<u32> {
        match self.0.next()? {
            Value::Uint(v) => Some(v),
            _ => None,
        }
    }

    pub fn fixed(&mut self) -> Option<f64> {
        match self.0.next()? {
            Value::Fixed(v) => Some(fixed_to_f64(v)),
            _ => None,
        }
    }

    /// A string (a null one, which only a nullable argument should be, as empty).
    pub fn string(&mut self) -> Option<String> {
        self.opt_string().map(Option::unwrap_or_default)
    }

    pub fn opt_string(&mut self) -> Option<Option<String>> {
        match self.0.next()? {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    /// An object (one libwayland no longer knows, as null).
    pub fn object(&mut self) -> Option<ObjectId> {
        self.opt_object().map(Option::unwrap_or_default)
    }

    pub fn opt_object(&mut self) -> Option<Option<ObjectId>> {
        match self.0.next()? {
            Value::Object(o) => Some(o),
            _ => None,
        }
    }

    /// The object the event made, as a `T`.
    pub fn new_id<T: Object>(&mut self) -> Option<T> {
        match self.0.next()? {
            Value::NewId(Some(p)) if ptr::eq(p.interface, T::INTERFACE) => Some(T::from_proxy(p)),
            _ => None,
        }
    }

    pub fn array(&mut self) -> Option<Vec<u8>> {
        match self.0.next()? {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }

    pub fn fd(&mut self) -> Option<OwnedFd> {
        match self.0.next()? {
            Value::Fd(fd) => fd,
            _ => None,
        }
    }
}

/// The dispatcher of every object's events: their arguments decoded into owned values and queued on the connection
/// (nothing else: no platform code runs inside libwayland's dispatch; it allocates, and can't unwind).
unsafe extern "C" fn dispatch(
    implementation: *const c_void,
    target: Ptr,
    opcode: u32,
    _message: *const Message,
    args: *mut Argument,
) -> c_int {
    let Ok(lib) = lib() else { return 0 };
    // SAFETY: `implementation` is what `Proxy::adopt` gave: a `'static` Interface, the target's.
    let interface: &'static Interface = unsafe { &*implementation.cast::<Interface>() };
    // SAFETY: the target is a live proxy (libwayland is dispatching it).
    let data = unsafe { (lib.proxy_get_user_data)(target) }.cast::<Inner>();
    if data.is_null() {
        return 0;
    }
    // SAFETY: the data `adopt` gave: the connection's Inner, alive while its display dispatches (only a live
    // Connection does).
    let inner = unsafe { &*data };
    let Some(connection) = inner.this.upgrade() else { return 0 };
    let Some(message) = interface.events().get(opcode as usize) else { return 0 };
    let types = message.types();
    let mut values = Vec::with_capacity(types.len());
    for (i, (ty, _)) in message.arguments().into_iter().enumerate() {
        // SAFETY: libwayland passes an argument for each of the message's (its signature is ours).
        let a = unsafe { *args.add(i) };
        // SAFETY: each field read is the one the signature's type says libwayland wrote; strings and arrays are
        // libwayland's, valid during the dispatch (copied); a new id is a fresh proxy of the message's type; a
        // received fd is ours.
        values.push(unsafe {
            match ty {
                b'i' => Value::Int(a.i),
                b'u' => Value::Uint(a.u),
                b'f' => Value::Fixed(a.f),
                // (Strings and arrays are libwayland's until the dispatch returns: copied.)
                b's' => Value::Str((!a.s.is_null()).then(|| CStr::from_ptr(a.s).to_string_lossy().into_owned())),
                b'o' => Value::Object((!a.o.is_null()).then_some(ObjectId(a.o as usize))),
                // (A new proxy libwayland made, of the message's type, with no dispatcher yet: ours from now on.)
                b'n' => Value::NewId(match types.get(i).copied().flatten() {
                    Some(t) if !a.o.is_null() => Some(Proxy::adopt(a.o, t, &connection)),
                    _ => None,
                }),
                b'a' => Value::Array(match a.a.as_ref() {
                    Some(array) if !array.data.is_null() => {
                        std::slice::from_raw_parts(array.data.cast::<u8>(), array.size).to_vec()
                    }
                    _ => Vec::new(),
                }),
                // (A received fd is the handler's: libwayland doesn't close it after a dispatcher.)
                b'h' => Value::Fd((a.h >= 0).then(|| OwnedFd::from_raw_fd(a.h))),
                _ => return 0,
            }
        });
    }
    let event = RawEvent { sender: ObjectId(target as usize), interface, opcode: opcode as u16, args: values };
    // (The queue is never borrowed across a dispatch; if it were, the event would be lost rather than the borrow
    // broken.)
    if let Ok(mut queue) = inner.queue.try_borrow_mut() {
        queue.push_back(event);
    }
    0
}

// ---- The connection -------------------------------------------------------------------------------------------------

struct Inner {
    lib: &'static Lib,
    display: Ptr,
    queue: RefCell<VecDeque<RawEvent>>,
    this: Weak<Inner>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        // (Objects queued go first; their proxies, of a connection going, are left to libwayland.)
        self.queue.get_mut().clear();
        // SAFETY: the display connected by `Connection::connect`, disconnected once; every object of it holds it
        // weakly and is inert from now on.
        unsafe { (self.lib.display_disconnect)(self.display) };
    }
}

/// A connection to the compositor (`wl_display`), and the queue its objects' events go to. Cheap to clone; the
/// display is disconnected when the last clone goes.
#[derive(Clone)]
pub struct Connection(Rc<Inner>);

impl Connection {
    /// The compositor's (`WAYLAND_DISPLAY`, or `WAYLAND_SOCKET`); why not if there is none or libwayland-client
    /// isn't installed.
    pub fn connect() -> Result<Connection, String> {
        let lib = lib()?;
        // SAFETY: null asks for the environment's display.
        let display = unsafe { (lib.display_connect)(ptr::null()) };
        if display.is_null() {
            return Err(format!("can't connect to the Wayland display ({})", io::Error::last_os_error()));
        }
        Ok(Connection(Rc::new_cyclic(|this| Inner { lib, display, queue: RefCell::default(), this: this.clone() })))
    }

    /// The display's object (borrowed: libwayland's).
    pub fn display(&self) -> WlDisplay {
        // SAFETY: the display is a wl_proxy of itself (wl_display_interface's, which the generated table matches),
        // alive while this connection is.
        WlDisplay::from_proxy(unsafe { Proxy::borrowed(self.0.display, &WL_DISPLAY_INTERFACE, &self.0) })
    }

    /// The `wl_display*` (for the Vulkan driver's WSI).
    pub fn as_ptr(&self) -> Ptr {
        self.0.display
    }

    /// The connection's socket, to poll.
    pub fn fd(&self) -> RawFd {
        // SAFETY: a live display.
        unsafe { (self.0.lib.display_get_fd)(self.0.display) }
    }

    fn result(&self, r: c_int) -> io::Result<c_int> {
        if r >= 0 { Ok(r) } else { Err(self.error_or(io::Error::last_os_error())) }
    }

    /// The display's error, as an io::Error (`fallback` if it has none).
    fn error_or(&self, fallback: io::Error) -> io::Error {
        match self.protocol_error() {
            Some(why) => io::Error::other(why),
            None => fallback,
        }
    }

    /// Dispatches the events read: queued (how many).
    pub fn dispatch_pending(&self) -> io::Result<usize> {
        // SAFETY: a live display; the dispatcher only queues (the queue isn't borrowed now).
        self.result(unsafe { (self.0.lib.display_dispatch_pending)(self.0.display) }).map(|n| n as usize)
    }

    /// Sends what is queued and waits until the compositor has handled it, dispatching (queuing) what comes back.
    pub fn roundtrip(&self) -> io::Result<()> {
        // SAFETY: as `dispatch_pending`.
        self.result(unsafe { (self.0.lib.display_roundtrip)(self.0.display) }).map(drop)
    }

    /// Sends the requests made; false if not all could be (the socket full: again when it is writable).
    pub fn flush(&self) -> io::Result<bool> {
        // SAFETY: a live display.
        let r = unsafe { (self.0.lib.display_flush)(self.0.display) };
        if r >= 0 {
            return Ok(true);
        }
        let e = io::Error::last_os_error();
        if e.kind() == io::ErrorKind::WouldBlock { Ok(false) } else { Err(self.error_or(e)) }
    }

    /// Gets ready to read the socket (`wl_display_prepare_read`): false if events are already read but not
    /// dispatched (dispatch them first). Then `read_events` or `cancel_read`, once.
    pub fn prepare_read(&self) -> bool {
        // SAFETY: a live display.
        unsafe { (self.0.lib.display_prepare_read)(self.0.display) == 0 }
    }

    /// Reads what the socket has (after `prepare_read`).
    pub fn read_events(&self) -> io::Result<()> {
        // SAFETY: a live display, prepared to read.
        self.result(unsafe { (self.0.lib.display_read_events)(self.0.display) }).map(drop)
    }

    /// Doesn't read after all (after `prepare_read`).
    pub fn cancel_read(&self) {
        // SAFETY: a live display, prepared to read.
        unsafe { (self.0.lib.display_cancel_read)(self.0.display) }
    }

    /// The next event queued.
    pub fn take_event(&self) -> Option<RawEvent> {
        self.0.queue.borrow_mut().pop_front()
    }

    pub fn has_events(&self) -> bool {
        !self.0.queue.borrow().is_empty()
    }

    /// What broke the connection (a protocol error: the object and interface it was on; or the error number), if it
    /// is broken.
    pub fn protocol_error(&self) -> Option<String> {
        // SAFETY: a live display.
        let e = unsafe { (self.0.lib.display_get_error)(self.0.display) };
        if e == 0 {
            return None;
        }
        if e != libc::EPROTO {
            return Some(io::Error::from_raw_os_error(e).to_string());
        }
        let mut interface: *const c_void = ptr::null();
        let mut id = 0u32;
        // SAFETY: a live display in error; both outputs are written.
        let code = unsafe { (self.0.lib.display_get_protocol_error)(self.0.display, &mut interface, &mut id) };
        let name = if interface.is_null() {
            "?".to_string()
        } else {
            // SAFETY: a `wl_interface` (ours or libwayland's), whose first field is its name, a C string.
            unsafe { CStr::from_ptr(*interface.cast::<*const c_char>()) }.to_string_lossy().into_owned()
        };
        Some(format!("Wayland protocol error {code} on {name}@{id}"))
    }

    /// The object of another library's `raw` proxy of `T`'s interface, borrowed.
    ///
    /// # Safety
    /// `raw` must be a `wl_proxy` of this connection's display, of `T`'s interface, alive while the result is used.
    unsafe fn borrow<T: Object>(&self, raw: Ptr) -> T {
        // SAFETY: as the caller says.
        T::from_proxy(unsafe { Proxy::borrowed(raw, T::INTERFACE, &self.0) })
    }
}

/// Bitfield operations for a generated bitfield's newtype (`|`, `&`, `contains`).
macro_rules! bitfield {
    ($t:ident) => {
        impl std::ops::BitOr for $t {
            type Output = $t;
            fn bitor(self, other: $t) -> $t {
                $t(self.0 | other.0)
            }
        }
        impl std::ops::BitOrAssign for $t {
            fn bitor_assign(&mut self, other: $t) {
                self.0 |= other.0;
            }
        }
        impl std::ops::BitAnd for $t {
            type Output = $t;
            fn bitand(self, other: $t) -> $t {
                $t(self.0 & other.0)
            }
        }
        impl $t {
            /// Whether all of `other`'s bits are set.
            pub fn contains(self, other: $t) -> bool {
                self.0 & other.0 == other.0
            }
        }
    };
}
pub(crate) use bitfield;

// ---- libwayland-cursor ----------------------------------------------------------------------------------------------

/// `struct wl_cursor_image`.
#[repr(C)]
struct RawCursorImage {
    width: u32,
    height: u32,
    hotspot_x: u32,
    hotspot_y: u32,
    delay: u32,
}

/// `struct wl_cursor`.
#[repr(C)]
struct RawCursor {
    image_count: c_uint,
    images: *mut *mut RawCursorImage,
    name: *mut c_char,
}

/// libwayland-cursor's functions (the cursor themes, where the compositor has no cursor-shape-v1).
struct CursorLib {
    theme_load: unsafe extern "C" fn(*const c_char, c_int, Ptr) -> Ptr,
    theme_destroy: unsafe extern "C" fn(Ptr),
    theme_get_cursor: unsafe extern "C" fn(Ptr, *const c_char) -> *mut RawCursor,
    image_get_buffer: unsafe extern "C" fn(*mut RawCursorImage) -> Ptr,
    frame_and_duration: unsafe extern "C" fn(*mut RawCursor, u32, *mut u32) -> c_int,
    _library: Library,
}

impl CursorLib {
    fn load() -> Result<CursorLib, String> {
        let library = Library::open(&["libwayland-cursor.so.0", "libwayland-cursor.so"])?;
        let l = &library;
        // SAFETY: each `function` call's type is wayland-cursor.h's signature of its symbol, as `CursorLib` declares
        // it (the theme and buffers as pointers, wl_cursor and wl_cursor_image as ours, the same layout).
        unsafe {
            Ok(CursorLib {
                theme_load: function(l, c"wl_cursor_theme_load")?,
                theme_destroy: function(l, c"wl_cursor_theme_destroy")?,
                theme_get_cursor: function(l, c"wl_cursor_theme_get_cursor")?,
                image_get_buffer: function(l, c"wl_cursor_image_get_buffer")?,
                frame_and_duration: function(l, c"wl_cursor_frame_and_duration")?,
                _library: library,
            })
        }
    }
}

fn cursor_lib() -> Result<&'static CursorLib, String> {
    static LIB: OnceLock<Result<CursorLib, String>> = OnceLock::new();
    LIB.get_or_init(CursorLib::load).as_ref().map_err(Clone::clone)
}

/// An Xcursor theme loaded at a size by libwayland-cursor (its images in buffers of a pool of its own on `wl_shm`),
/// destroyed when dropped.
pub struct CursorTheme {
    lib: &'static CursorLib,
    raw: NonNull<c_void>,
    connection: Connection,
}

impl CursorTheme {
    /// Theme `name` (none: the default) with its cursors `size` pixels big; why not if it can't be loaded.
    pub fn load(connection: &Connection, name: Option<&str>, size: i32, shm: &WlShm) -> Result<CursorTheme, String> {
        let lib = cursor_lib()?;
        let shm = shm.proxy().as_ptr();
        if shm.is_null() {
            return Err("no wl_shm".into());
        }
        let name = name.map(c_string);
        // SAFETY: a valid C string or null, a live wl_shm proxy of the connection's display.
        let raw = unsafe { (lib.theme_load)(name.as_ref().map_or(ptr::null(), |n| n.as_ptr()), size, shm) };
        let raw = NonNull::new(raw).ok_or("wl_cursor_theme_load failed")?;
        Ok(CursorTheme { lib, raw, connection: connection.clone() })
    }

    /// The theme's cursor `name` ("default", "pointer", "text"…), if it has it.
    pub fn cursor(&self, name: &str) -> Option<Cursor<'_>> {
        let name = c_string(name);
        // SAFETY: a live theme, a valid C string; the cursor returned is the theme's, alive while it is.
        let raw = unsafe { (self.lib.theme_get_cursor)(self.raw.as_ptr(), name.as_ptr()) };
        NonNull::new(raw).map(|raw| Cursor { theme: self, raw })
    }
}

impl Drop for CursorTheme {
    fn drop(&mut self) {
        // SAFETY: the theme loaded by `load`, destroyed once (its cursors borrow it, so none is left).
        unsafe { (self.lib.theme_destroy)(self.raw.as_ptr()) };
    }
}

/// A theme's cursor: its images (several, for an animated one).
pub struct Cursor<'a> {
    theme: &'a CursorTheme,
    raw: NonNull<RawCursor>,
}

impl<'a> Cursor<'a> {
    pub fn image_count(&self) -> usize {
        // SAFETY: the theme's cursor, alive while it is.
        unsafe { self.raw.as_ref() }.image_count as usize
    }

    /// The image shown `ms` milliseconds into its animation, and how long it stays (0: for good).
    pub fn frame_at(&self, ms: u32) -> (usize, u32) {
        let mut duration = 0;
        // SAFETY: the theme's cursor; `duration` is written.
        let frame = unsafe { (self.theme.lib.frame_and_duration)(self.raw.as_ptr(), ms, &mut duration) };
        (frame.max(0) as usize, duration)
    }

    /// Image `index`.
    pub fn image(&self, index: usize) -> Option<CursorImage<'a>> {
        if index >= self.image_count() {
            return None;
        }
        // SAFETY: `images` has `image_count` entries (checked), each a live image of the theme.
        let raw = unsafe { *self.raw.as_ref().images.add(index) };
        // SAFETY: as above: an image, alive while the theme is.
        let image = unsafe { raw.as_ref()? };
        // SAFETY: the image's buffer is a wl_buffer proxy the theme made on our display (its wl_shm's), alive while
        // the theme is: borrowed for as long as the image (which borrows the theme).
        let buffer = unsafe { (self.theme.lib.image_get_buffer)(raw) };
        if buffer.is_null() {
            return None;
        }
        Some(CursorImage {
            width: image.width as i32,
            height: image.height as i32,
            hotspot_x: image.hotspot_x as i32,
            hotspot_y: image.hotspot_y as i32,
            // SAFETY: as above, of wl_buffer's interface.
            buffer: unsafe { self.theme.connection.borrow::<WlBuffer>(buffer) },
            _theme: PhantomData,
        })
    }
}

/// A cursor's image: its size, its hotspot (pixels), its buffer (the theme's).
pub struct CursorImage<'a> {
    pub width: i32,
    pub height: i32,
    pub hotspot_x: i32,
    pub hotspot_y: i32,
    buffer: WlBuffer,
    _theme: PhantomData<&'a CursorTheme>,
}

impl CursorImage<'_> {
    /// Its buffer, to attach to a cursor surface.
    pub fn buffer(&self) -> &WlBuffer {
        &self.buffer
    }
}

#[cfg(test)]
mod tests {
    use super::super::generated::*;
    use super::*;
    use std::mem::{offset_of, size_of};

    #[test]
    fn tables_have_libwaylands_layout() {
        // (wl_interface's fields first, at their C offsets; wl_message exactly; wl_argument a word.)
        assert_eq!(offset_of!(Interface, name), 0);
        assert_eq!(offset_of!(Interface, version), size_of::<Ptr>());
        assert_eq!(offset_of!(Interface, method_count), size_of::<Ptr>() + 4);
        assert_eq!(offset_of!(Interface, methods), size_of::<Ptr>() + 8);
        assert_eq!(offset_of!(Interface, event_count), 2 * size_of::<Ptr>() + 8);
        assert_eq!(offset_of!(Interface, events), 3 * size_of::<Ptr>() + if size_of::<Ptr>() == 8 { 8 } else { 4 });
        assert_eq!(size_of::<Message>(), 3 * size_of::<Ptr>());
        assert_eq!(size_of::<Argument>(), 8.max(size_of::<Ptr>()));
        assert_eq!(size_of::<Option<&Interface>>(), size_of::<Ptr>());
    }

    #[test]
    fn fixed_point_is_24_8() {
        assert_eq!(fixed_to_f64(256), 1.0);
        assert_eq!(fixed_to_f64(-384), -1.5);
        assert_eq!(fixed_to_f64(1), 1.0 / 256.0);
        assert_eq!(fixed_from_f64(12.25), 3136);
        assert_eq!(fixed_from_f64(-0.5), -128);
        assert_eq!(fixed_to_f64(fixed_from_f64(123.456)), 123.45703125);
    }

    #[test]
    fn signatures_are_read() {
        let bind = &WL_REGISTRY_INTERFACE.requests()[0];
        assert_eq!(bind.name(), c"bind");
        assert_eq!(bind.signature(), c"usun");
        assert_eq!(bind.arguments(), [(b'u', false), (b's', false), (b'u', false), (b'n', false)]);
        assert_eq!(bind.types().len(), 4);
        let attach = &WL_SURFACE_INTERFACE.requests()[1];
        assert_eq!(attach.signature(), c"?oii");
        assert_eq!(attach.arguments()[0], (b'o', true));
        assert!(ptr::eq(attach.types()[0].unwrap(), &WL_BUFFER_INTERFACE));
        let damage_buffer = WL_SURFACE_INTERFACE.requests().iter().find(|m| m.name() == c"damage_buffer").unwrap();
        assert_eq!(damage_buffer.since(), 4);
        assert_eq!(WL_SURFACE_INTERFACE.destructor(), Some(0));
        assert_eq!(WL_CALLBACK_INTERFACE.destructor(), None);
        // (wl_pointer's release is a destructor since version 3.)
        let release = &WL_POINTER_INTERFACE.requests()[WL_POINTER_INTERFACE.destructor().unwrap() as usize];
        assert_eq!((release.name(), release.since()), (c"release", 3));
        let get_toplevel = &XDG_SURFACE_INTERFACE.requests()[1];
        assert!(ptr::eq(get_toplevel.types()[0].unwrap(), &XDG_TOPLEVEL_INTERFACE));
        let get_xdg_surface = &XDG_WM_BASE_INTERFACE.requests()[2];
        assert!(ptr::eq(get_xdg_surface.types()[1].unwrap(), &WL_SURFACE_INTERFACE));
    }

    #[test]
    fn enums_have_the_protocols_values() {
        assert_eq!(WlSeatCapability::POINTER | WlSeatCapability::KEYBOARD, WlSeatCapability(3));
        assert!(WlSeatCapability(7).contains(WlSeatCapability::TOUCH));
        assert!(!WlSeatCapability(3).contains(WlSeatCapability::TOUCH));
        assert_eq!(WlOutputTransform::_270.0, 3);
        assert_eq!(XdgToplevelState::TILED_BOTTOM.0, 8);
        assert_eq!(XdgToplevelResizeEdge::BOTTOM_RIGHT.0, 10);
        assert_eq!(WpCursorShapeDeviceV1Shape::NOT_ALLOWED.0, 15);
        assert_eq!(WpCursorShapeDeviceV1Shape::NS_RESIZE.0, 27);
        assert_eq!(ZwpTextInputV3ContentPurpose::PIN.0, 9);
        assert_eq!(ZwpTextInputV3ContentHint::SENSITIVE_DATA.0, 0x80);
        assert_eq!(WlDataDeviceManagerDndAction::COPY.0, 1);
        assert_eq!(ZxdgToplevelDecorationV1Mode::SERVER_SIDE.0, 2);
    }

    #[test]
    fn events_are_decoded_from_their_values() {
        let raw = RawEvent {
            sender: ObjectId(8),
            interface: &WL_POINTER_INTERFACE,
            opcode: 0,
            args: vec![Value::Uint(7), Value::Object(Some(ObjectId(16))), Value::Fixed(256), Value::Fixed(-128)],
        };
        let (sender, event) = Event::decode(raw).unwrap();
        assert_eq!(sender, ObjectId(8));
        let Event::WlPointer(WlPointerEvent::Enter { serial, surface, surface_x, surface_y }) = event else {
            panic!("{event:?}")
        };
        assert_eq!((serial, surface, surface_x, surface_y), (7, ObjectId(16), 1.0, -0.5));
        // (Arguments that aren't the event's: none.)
        let wrong = RawEvent { sender: ObjectId(8), interface: &WL_POINTER_INTERFACE, opcode: 0, args: vec![] };
        assert!(Event::decode(wrong).is_none());
        let states = RawEvent {
            sender: ObjectId(8),
            interface: &XDG_TOPLEVEL_INTERFACE,
            opcode: 0,
            args: vec![Value::Int(800), Value::Int(600), Value::Array(vec![4, 0, 0, 0])],
        };
        let Some((_, Event::XdgToplevel(XdgToplevelEvent::Configure { width, height, states }))) =
            Event::decode(states)
        else {
            panic!()
        };
        assert_eq!((width, height, states), (800, 600, vec![4, 0, 0, 0]));
    }

    /// libwayland-client's own core tables, where it is installed: ours say the same of every message both have.
    #[test]
    fn core_tables_match_libwaylands() {
        let Ok(lib) = lib() else { return };
        for ours in [
            &WL_DISPLAY_INTERFACE,
            &WL_REGISTRY_INTERFACE,
            &WL_CALLBACK_INTERFACE,
            &WL_COMPOSITOR_INTERFACE,
            &WL_SHM_POOL_INTERFACE,
            &WL_SHM_INTERFACE,
            &WL_BUFFER_INTERFACE,
            &WL_DATA_OFFER_INTERFACE,
            &WL_DATA_SOURCE_INTERFACE,
            &WL_DATA_DEVICE_INTERFACE,
            &WL_DATA_DEVICE_MANAGER_INTERFACE,
            &WL_SURFACE_INTERFACE,
            &WL_SEAT_INTERFACE,
            &WL_POINTER_INTERFACE,
            &WL_KEYBOARD_INTERFACE,
            &WL_TOUCH_INTERFACE,
            &WL_OUTPUT_INTERFACE,
            &WL_REGION_INTERFACE,
            &WL_SUBCOMPOSITOR_INTERFACE,
            &WL_SUBSURFACE_INTERFACE,
        ] {
            let symbol = CString::new(format!("{}_interface", ours.name().to_string_lossy())).unwrap();
            let Some(theirs) = lib._library.symbol(&symbol) else { continue };
            // SAFETY: libwayland's exported wl_interface: its first fields are ours (only those are read).
            let (name, methods, method_count, events, event_count) = unsafe {
                let p = theirs.cast::<u8>();
                let at = |offset: usize| p.add(offset);
                (
                    *at(offset_of!(Interface, name)).cast::<*const c_char>(),
                    *at(offset_of!(Interface, methods)).cast::<*const Message>(),
                    *at(offset_of!(Interface, method_count)).cast::<c_int>(),
                    *at(offset_of!(Interface, events)).cast::<*const Message>(),
                    *at(offset_of!(Interface, event_count)).cast::<c_int>(),
                )
            };
            // SAFETY: libwayland's name, a C string.
            assert_eq!(unsafe { CStr::from_ptr(name) }, ours.name());
            for (mine, (theirs, count)) in
                [(ours.requests(), (methods, method_count)), (ours.events(), (events, event_count))]
            {
                for (i, a) in mine.iter().enumerate().take(count as usize) {
                    // SAFETY: libwayland's message array of `count` wl_messages (read as raw fields: name, signature,
                    // types), each type null or a wl_interface whose first field is its name.
                    let (name, signature, types) = unsafe {
                        let m = theirs.cast::<*const c_void>().add(3 * i);
                        (CStr::from_ptr((*m).cast()), CStr::from_ptr((*m.add(1)).cast()), *m.add(2))
                    };
                    assert_eq!(a.name(), name, "{ours:?}");
                    assert_eq!(a.signature(), signature, "{ours:?} {a:?}");
                    for (j, ta) in a.types().iter().enumerate() {
                        // SAFETY: as above: an entry for each of the signature's arguments.
                        let tb = unsafe { *types.cast::<*const *const c_char>().add(j) };
                        // SAFETY: as above.
                        let tb = (!tb.is_null()).then(|| unsafe { CStr::from_ptr(*tb) });
                        assert_eq!(ta.map(|t| t.name()), tb, "{ours:?} {a:?}");
                    }
                }
            }
        }
    }
}
