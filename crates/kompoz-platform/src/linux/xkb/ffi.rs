//! libxkbcommon (and libxkbcommon-x11), loaded at run time as the C++ Kompoz loads it (`dlopen`: a binary that runs
//! where it isn't installed, headless), its objects in safe owning wrappers: the context, keymaps (from a string, as
//! Wayland hands them over; from rule names, as tests make them; from an X server's core keyboard), keyboard states,
//! compose tables and states. The only `unsafe` code of the keyboard module, each block saying why it holds.

#![allow(unsafe_code)]

use crate::linux::sys::Library;
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::sync::Arc;

type Ptr = *mut c_void;

/// `xkb_rule_names`.
#[repr(C)]
struct RuleNames {
    rules: *const c_char,
    model: *const c_char,
    layout: *const c_char,
    variant: *const c_char,
    options: *const c_char,
}

/// `XKB_KEYCODE_INVALID`.
const KEYCODE_INVALID: u32 = 0xffff_ffff;
/// `XKB_KEYMAP_FORMAT_TEXT_V1`.
const FORMAT_TEXT_V1: c_int = 1;
/// `XKB_STATE_MODS_EFFECTIVE`, `XKB_STATE_LAYOUT_EFFECTIVE`.
pub(super) const MODS_EFFECTIVE: c_int = 1 << 3;
pub(super) const LAYOUT_EFFECTIVE: c_int = 1 << 7;

/// The functions used, looked up once.
struct Functions {
    context_new: unsafe extern "C" fn(c_int) -> Ptr,
    context_unref: unsafe extern "C" fn(Ptr),
    keymap_new_from_names: unsafe extern "C" fn(Ptr, *const RuleNames, c_int) -> Ptr,
    keymap_new_from_string: unsafe extern "C" fn(Ptr, *const c_char, c_int, c_int) -> Ptr,
    keymap_unref: unsafe extern "C" fn(Ptr),
    keymap_key_repeats: unsafe extern "C" fn(Ptr, u32) -> c_int,
    keymap_key_by_name: unsafe extern "C" fn(Ptr, *const c_char) -> u32,
    keymap_key_get_syms_by_level: unsafe extern "C" fn(Ptr, u32, u32, u32, *mut *const u32) -> c_int,
    keymap_num_layouts: unsafe extern "C" fn(Ptr) -> u32,
    keymap_get_as_string: unsafe extern "C" fn(Ptr, c_int) -> *mut c_char,
    state_new: unsafe extern "C" fn(Ptr) -> Ptr,
    state_unref: unsafe extern "C" fn(Ptr),
    state_update_mask: unsafe extern "C" fn(Ptr, u32, u32, u32, u32, u32, u32) -> c_int,
    state_update_key: unsafe extern "C" fn(Ptr, u32, c_int) -> c_int,
    state_key_get_one_sym: unsafe extern "C" fn(Ptr, u32) -> u32,
    state_key_get_utf8: unsafe extern "C" fn(Ptr, u32, *mut c_char, usize) -> c_int,
    state_key_get_utf32: unsafe extern "C" fn(Ptr, u32) -> u32,
    state_key_get_layout: unsafe extern "C" fn(Ptr, u32) -> u32,
    state_mod_name_is_active: unsafe extern "C" fn(Ptr, *const c_char, c_int) -> c_int,
    state_serialize_mods: unsafe extern "C" fn(Ptr, c_int) -> u32,
    state_serialize_layout: unsafe extern "C" fn(Ptr, c_int) -> u32,
    keysym_from_name: unsafe extern "C" fn(*const c_char, c_int) -> u32,
    keysym_to_utf8: unsafe extern "C" fn(u32, *mut c_char, usize) -> c_int,
    keysym_to_utf32: unsafe extern "C" fn(u32) -> u32,
    compose_table_new_from_locale: unsafe extern "C" fn(Ptr, *const c_char, c_int) -> Ptr,
    compose_table_unref: unsafe extern "C" fn(Ptr),
    compose_state_new: unsafe extern "C" fn(Ptr, c_int) -> Ptr,
    compose_state_unref: unsafe extern "C" fn(Ptr),
    compose_state_feed: unsafe extern "C" fn(Ptr, u32) -> c_int,
    compose_state_reset: unsafe extern "C" fn(Ptr),
    compose_state_get_status: unsafe extern "C" fn(Ptr) -> c_int,
    compose_state_get_utf8: unsafe extern "C" fn(Ptr, *mut c_char, usize) -> c_int,
    compose_state_get_one_sym: unsafe extern "C" fn(Ptr) -> u32,
}

/// libxkbcommon-x11's functions (X11 only).
struct X11Functions {
    setup_xkb_extension: unsafe extern "C" fn(Ptr, u16, u16, c_int, *mut u16, *mut u16, *mut u8, *mut u8) -> c_int,
    get_core_keyboard_device_id: unsafe extern "C" fn(Ptr) -> i32,
    keymap_new_from_device: unsafe extern "C" fn(Ptr, Ptr, i32, c_int) -> Ptr,
    state_new_from_device: unsafe extern "C" fn(Ptr, Ptr, i32) -> Ptr,
}

/// The libraries, loaded, and their functions.
struct Lib {
    f: Functions,
    x11: Option<X11Functions>,
    // (Last: closed after the functions are gone.)
    _x11_library: Option<Library>,
    _library: Library,
}

/// Looks `name` up in `library` as a function pointer of type `F`.
///
/// # Safety
/// `F` must be the symbol's C signature as an `unsafe extern "C" fn`.
unsafe fn function<F: Copy>(library: &Library, name: &CStr) -> Result<F, String> {
    let p = library.symbol(name).ok_or_else(|| format!("libxkbcommon has no {}", name.to_string_lossy()))?;
    debug_assert_eq!(std::mem::size_of::<F>(), std::mem::size_of::<*mut c_void>());
    // SAFETY: the caller says `F` is the symbol's signature, a function pointer as big as `p`.
    Ok(unsafe { std::mem::transmute_copy::<*mut c_void, F>(&p) })
}

impl Lib {
    fn load() -> Result<Lib, String> {
        let library = Library::open(&["libxkbcommon.so.0", "libxkbcommon.so"])?;
        let l = &library;
        // SAFETY: each `function` call's type is xkbcommon.h's signature of its symbol, as `Functions` declares it.
        let f = unsafe {
            Functions {
                context_new: function(l, c"xkb_context_new")?,
                context_unref: function(l, c"xkb_context_unref")?,
                keymap_new_from_names: function(l, c"xkb_keymap_new_from_names")?,
                keymap_new_from_string: function(l, c"xkb_keymap_new_from_string")?,
                keymap_unref: function(l, c"xkb_keymap_unref")?,
                keymap_key_repeats: function(l, c"xkb_keymap_key_repeats")?,
                keymap_key_by_name: function(l, c"xkb_keymap_key_by_name")?,
                keymap_key_get_syms_by_level: function(l, c"xkb_keymap_key_get_syms_by_level")?,
                keymap_num_layouts: function(l, c"xkb_keymap_num_layouts")?,
                keymap_get_as_string: function(l, c"xkb_keymap_get_as_string")?,
                state_new: function(l, c"xkb_state_new")?,
                state_unref: function(l, c"xkb_state_unref")?,
                state_update_mask: function(l, c"xkb_state_update_mask")?,
                state_update_key: function(l, c"xkb_state_update_key")?,
                state_key_get_one_sym: function(l, c"xkb_state_key_get_one_sym")?,
                state_key_get_utf8: function(l, c"xkb_state_key_get_utf8")?,
                state_key_get_utf32: function(l, c"xkb_state_key_get_utf32")?,
                state_key_get_layout: function(l, c"xkb_state_key_get_layout")?,
                state_mod_name_is_active: function(l, c"xkb_state_mod_name_is_active")?,
                state_serialize_mods: function(l, c"xkb_state_serialize_mods")?,
                state_serialize_layout: function(l, c"xkb_state_serialize_layout")?,
                keysym_from_name: function(l, c"xkb_keysym_from_name")?,
                keysym_to_utf8: function(l, c"xkb_keysym_to_utf8")?,
                keysym_to_utf32: function(l, c"xkb_keysym_to_utf32")?,
                compose_table_new_from_locale: function(l, c"xkb_compose_table_new_from_locale")?,
                compose_table_unref: function(l, c"xkb_compose_table_unref")?,
                compose_state_new: function(l, c"xkb_compose_state_new")?,
                compose_state_unref: function(l, c"xkb_compose_state_unref")?,
                compose_state_feed: function(l, c"xkb_compose_state_feed")?,
                compose_state_reset: function(l, c"xkb_compose_state_reset")?,
                compose_state_get_status: function(l, c"xkb_compose_state_get_status")?,
                compose_state_get_utf8: function(l, c"xkb_compose_state_get_utf8")?,
                compose_state_get_one_sym: function(l, c"xkb_compose_state_get_one_sym")?,
            }
        };
        let x11_library = Library::open(&["libxkbcommon-x11.so.0", "libxkbcommon-x11.so"]).ok();
        let x11 = x11_library.as_ref().and_then(|l| {
            // SAFETY: each `function` call's type is xkbcommon-x11.h's signature of its symbol, as `X11Functions`
            // declares it (an xcb_connection_t* as a pointer).
            unsafe {
                Some(X11Functions {
                    setup_xkb_extension: function(l, c"xkb_x11_setup_xkb_extension").ok()?,
                    get_core_keyboard_device_id: function(l, c"xkb_x11_get_core_keyboard_device_id").ok()?,
                    keymap_new_from_device: function(l, c"xkb_x11_keymap_new_from_device").ok()?,
                    state_new_from_device: function(l, c"xkb_x11_state_new_from_device").ok()?,
                })
            }
        });
        Ok(Lib { f, x11, _x11_library: x11_library, _library: library })
    }
}

/// A C string of `s` (none for an empty one: xkbcommon's default then).
fn c_or_null(s: &str) -> Option<CString> {
    if s.is_empty() { None } else { CString::new(s).ok() }
}

/// Text a call wrote into `buffer` (`n` bytes, or the size it would need if bigger: then asked again bigger).
fn written(mut get: impl FnMut(*mut c_char, usize) -> c_int) -> String {
    let mut buffer = vec![0u8; 64];
    loop {
        let n = get(buffer.as_mut_ptr().cast(), buffer.len());
        if n <= 0 {
            return String::new();
        }
        let n = n as usize;
        if n < buffer.len() {
            buffer.truncate(n);
            return String::from_utf8_lossy(&buffer).into_owned();
        }
        buffer.resize(n + 1, 0);
    }
}

/// An xkbcommon context: what keymaps and compose tables are made in. Its library stays loaded while it or anything
/// made from it lives.
pub struct Context {
    lib: Arc<Lib>,
    raw: Ptr,
}

impl Context {
    /// A context with the default include paths (libxkbcommon loaded); why not if it can't be had.
    pub fn new() -> Result<Context, String> {
        let lib = Arc::new(Lib::load()?);
        // SAFETY: a plain call (no flags).
        let raw = unsafe { (lib.f.context_new)(0) };
        if raw.is_null() {
            return Err("xkb_context_new failed".into());
        }
        Ok(Context { lib, raw })
    }

    /// The keymap a compositor hands over (`XKB_KEYMAP_FORMAT_TEXT_V1`, Wayland's `wl_keyboard.keymap`), if it
    /// compiles.
    pub fn keymap_from_string(&self, text: &str) -> Option<Keymap> {
        let text = CString::new(text.trim_end_matches('\0')).ok()?;
        // SAFETY: the context is alive; `text` is a valid C string; the format and (no) flags are xkbcommon's.
        let raw = unsafe { (self.lib.f.keymap_new_from_string)(self.raw, text.as_ptr(), FORMAT_TEXT_V1, 0) };
        Keymap::from_raw(&self.lib, raw)
    }

    /// The keymap of RMLVO names ("evdev", "pc105", "tr", "", ""; empty: the default or the environment's), if
    /// it compiles.
    pub fn keymap_from_names(
        &self,
        rules: &str,
        model: &str,
        layout: &str,
        variant: &str,
        options: &str,
    ) -> Option<Keymap> {
        let strings = [rules, model, layout, variant, options].map(c_or_null);
        let p = |i: usize| strings[i].as_ref().map_or(std::ptr::null(), |s| s.as_ptr());
        let names = RuleNames { rules: p(0), model: p(1), layout: p(2), variant: p(3), options: p(4) };
        // SAFETY: the context is alive; `names` holds valid C strings (or nulls) that outlive the call.
        let raw = unsafe { (self.lib.f.keymap_new_from_names)(self.raw, &names, 0) };
        Keymap::from_raw(&self.lib, raw)
    }

    /// The compose table of `locale` ("tr_TR.UTF-8"; the X locale's Compose file, the user's ~/.XCompose and
    /// $XCOMPOSEFILE as xkbcommon finds them), if it has one.
    pub fn compose_table(&self, locale: &str) -> Option<ComposeTable> {
        let locale = CString::new(locale).ok()?;
        // SAFETY: the context is alive; `locale` is a valid C string; no flags.
        let raw = unsafe { (self.lib.f.compose_table_new_from_locale)(self.raw, locale.as_ptr(), 0) };
        (!raw.is_null()).then(|| ComposeTable { lib: self.lib.clone(), raw })
    }

    /// The keysym named `name` ("a", "dead_acute", "F13"), if there is one.
    pub fn keysym_from_name(&self, name: &str) -> Option<u32> {
        let name = CString::new(name).ok()?;
        // SAFETY: `name` is a valid C string; no flags (the name's case as given).
        let sym = unsafe { (self.lib.f.keysym_from_name)(name.as_ptr(), 0) };
        (sym != 0).then_some(sym)
    }

    /// What `keysym` types, as UTF-8 (empty: nothing).
    pub fn keysym_to_utf8(&self, keysym: u32) -> String {
        // SAFETY: the buffer is `size` writable bytes.
        written(|buffer, size| unsafe { (self.lib.f.keysym_to_utf8)(keysym, buffer, size) } - 1)
    }

    /// What `keysym` types, as a Unicode scalar (0: nothing).
    pub fn keysym_to_utf32(&self, keysym: u32) -> u32 {
        // SAFETY: a plain call.
        unsafe { (self.lib.f.keysym_to_utf32)(keysym) }
    }

    /// Whether libxkbcommon-x11 is loaded too (X11's keymaps from the server).
    pub fn has_x11(&self) -> bool {
        self.lib.x11.is_some()
    }

    /// Sets the XKB extension up on an X connection (`xkb_x11_setup_xkb_extension`, version 1.0); its first event
    /// code, if it could be.
    ///
    /// # Safety
    /// `connection` must be a live `xcb_connection_t*` (an Xlib display's, `XGetXCBConnection`).
    pub unsafe fn x11_setup_xkb_extension(&self, connection: *mut c_void) -> Option<u8> {
        let x11 = self.lib.x11.as_ref()?;
        let (mut major, mut minor, mut event, mut error) = (0u16, 0u16, 0u8, 0u8);
        // SAFETY: the caller says the connection is live; the out pointers are valid locals.
        let ok =
            unsafe { (x11.setup_xkb_extension)(connection, 1, 0, 0, &mut major, &mut minor, &mut event, &mut error) };
        (ok != 0).then_some(event)
    }

    /// The X server's core keyboard's device id, if it has one.
    ///
    /// # Safety
    /// As [`x11_setup_xkb_extension`](Context::x11_setup_xkb_extension), the extension set up on it.
    pub unsafe fn x11_core_keyboard(&self, connection: *mut c_void) -> Option<i32> {
        let x11 = self.lib.x11.as_ref()?;
        // SAFETY: the caller says the connection is live with the extension set up.
        let id = unsafe { (x11.get_core_keyboard_device_id)(connection) };
        (id >= 0).then_some(id)
    }

    /// The keymap of an X server's keyboard `device`, and a state following it, if they can be had.
    ///
    /// # Safety
    /// As [`x11_core_keyboard`](Context::x11_core_keyboard).
    pub unsafe fn x11_keymap(&self, connection: *mut c_void, device: i32) -> Option<(Keymap, KeyboardState)> {
        let x11 = self.lib.x11.as_ref()?;
        // SAFETY: the context is alive; the caller says the connection is live with the extension set up.
        let raw = unsafe { (x11.keymap_new_from_device)(self.raw, connection, device, 0) };
        let keymap = Keymap::from_raw(&self.lib, raw)?;
        // SAFETY: the keymap is alive; the connection as above.
        let state = unsafe { (x11.state_new_from_device)(keymap.raw, connection, device) };
        (!state.is_null()).then(|| {
            let state = KeyboardState { lib: self.lib.clone(), raw: state };
            (keymap, state)
        })
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        // SAFETY: our reference, released once (keymaps and tables hold their own).
        unsafe { (self.lib.f.context_unref)(self.raw) };
    }
}

/// A compiled keymap.
pub struct Keymap {
    lib: Arc<Lib>,
    raw: Ptr,
}

impl Keymap {
    fn from_raw(lib: &Arc<Lib>, raw: Ptr) -> Option<Keymap> {
        (!raw.is_null()).then(|| Keymap { lib: lib.clone(), raw })
    }

    /// A state of it (nothing pressed, the first layout).
    pub fn state(&self) -> Option<KeyboardState> {
        // SAFETY: the keymap is alive; the state holds its own reference to it.
        let raw = unsafe { (self.lib.f.state_new)(self.raw) };
        (!raw.is_null()).then(|| KeyboardState { lib: self.lib.clone(), raw })
    }

    /// Whether `key` (an X keycode: evdev's + 8) repeats while held.
    pub fn key_repeats(&self, key: u32) -> bool {
        // SAFETY: the keymap is alive.
        unsafe { (self.lib.f.keymap_key_repeats)(self.raw, key) != 0 }
    }

    /// The keycode of the key named `name` ("AC01": a US keyboard's A), if the keymap has it.
    pub fn key_by_name(&self, name: &str) -> Option<u32> {
        let name = CString::new(name).ok()?;
        // SAFETY: the keymap is alive; `name` is a valid C string.
        let key = unsafe { (self.lib.f.keymap_key_by_name)(self.raw, name.as_ptr()) };
        (key != KEYCODE_INVALID).then_some(key)
    }

    /// The keysyms `key` has at `level` of `layout` (level 0: unshifted).
    pub fn key_syms_by_level(&self, key: u32, layout: u32, level: u32) -> Vec<u32> {
        let mut syms: *const u32 = std::ptr::null();
        // SAFETY: the keymap is alive; `syms` is set to an array of the returned length owned by the keymap.
        let n = unsafe { (self.lib.f.keymap_key_get_syms_by_level)(self.raw, key, layout, level, &mut syms) };
        if n <= 0 || syms.is_null() {
            return Vec::new();
        }
        // SAFETY: as above: `n` keysyms there while the keymap lives, copied at once.
        unsafe { std::slice::from_raw_parts(syms, n as usize) }.to_vec()
    }

    /// How many layouts (groups) it has.
    pub fn layouts(&self) -> u32 {
        // SAFETY: the keymap is alive.
        unsafe { (self.lib.f.keymap_num_layouts)(self.raw) }
    }

    /// It as text (`XKB_KEYMAP_FORMAT_TEXT_V1`: what a compositor hands its clients, and X11 a keymap of its own).
    pub fn as_string(&self) -> Option<String> {
        // SAFETY: the keymap is alive; the format is xkbcommon's.
        let text = unsafe { (self.lib.f.keymap_get_as_string)(self.raw, FORMAT_TEXT_V1) };
        if text.is_null() {
            return None;
        }
        // SAFETY: not null: a C string xkbcommon allocated with malloc, copied, then freed once as it says.
        let copy = unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned();
        // SAFETY: as above: ours to free, not used after.
        unsafe { libc::free(text.cast()) };
        Some(copy)
    }
}

impl Drop for Keymap {
    fn drop(&mut self) {
        // SAFETY: our reference, released once.
        unsafe { (self.lib.f.keymap_unref)(self.raw) };
    }
}

/// A keyboard's state on a keymap: the modifiers and layout in effect, what each key gives in it.
pub struct KeyboardState {
    lib: Arc<Lib>,
    raw: Ptr,
}

impl KeyboardState {
    /// The modifiers and layout as a compositor or server says (Wayland's `wl_keyboard.modifiers`: depressed,
    /// latched and locked modifiers, the effective group as the locked layout).
    pub fn update_mask(
        &mut self,
        depressed_mods: u32,
        latched_mods: u32,
        locked_mods: u32,
        depressed_layout: u32,
        latched_layout: u32,
        locked_layout: u32,
    ) {
        // SAFETY: the state is alive (and ours alone: `&mut self`).
        unsafe {
            (self.lib.f.state_update_mask)(
                self.raw,
                depressed_mods,
                latched_mods,
                locked_mods,
                depressed_layout,
                latched_layout,
                locked_layout,
            )
        };
    }

    /// `key` pressed or let go, the state working out the modifiers itself (X11 without the server's, tests).
    pub fn update_key(&mut self, key: u32, down: bool) {
        // SAFETY: the state is alive; the direction is XKB_KEY_DOWN (1) or XKB_KEY_UP (0).
        unsafe { (self.lib.f.state_update_key)(self.raw, key, c_int::from(down)) };
    }

    /// The keysym `key` gives now (0: none, or several).
    pub fn key_sym(&self, key: u32) -> u32 {
        // SAFETY: the state is alive.
        unsafe { (self.lib.f.state_key_get_one_sym)(self.raw, key) }
    }

    /// What `key` types now, as UTF-8 (control characters included; empty: nothing).
    pub fn key_utf8(&self, key: u32) -> String {
        // SAFETY: the state is alive; the buffer is `size` writable bytes.
        written(|buffer, size| unsafe { (self.lib.f.state_key_get_utf8)(self.raw, key, buffer, size) })
    }

    /// What `key` types now, as a Unicode scalar (0: nothing).
    pub fn key_utf32(&self, key: u32) -> u32 {
        // SAFETY: the state is alive.
        unsafe { (self.lib.f.state_key_get_utf32)(self.raw, key) }
    }

    /// The layout `key` is in now.
    pub fn key_layout(&self, key: u32) -> u32 {
        // SAFETY: the state is alive.
        unsafe { (self.lib.f.state_key_get_layout)(self.raw, key) }
    }

    /// Whether the modifier named `name` ("Shift", "Control", "Mod1", "Mod4") is in effect.
    pub fn mod_name_is_active(&self, name: &CStr) -> bool {
        // SAFETY: the state is alive; `name` is a valid C string.
        unsafe { (self.lib.f.state_mod_name_is_active)(self.raw, name.as_ptr(), MODS_EFFECTIVE) > 0 }
    }

    /// The modifiers in effect as a mask.
    pub fn effective_mods(&self) -> u32 {
        // SAFETY: the state is alive.
        unsafe { (self.lib.f.state_serialize_mods)(self.raw, MODS_EFFECTIVE) }
    }

    /// The layout in effect.
    pub fn effective_layout(&self) -> u32 {
        // SAFETY: the state is alive.
        unsafe { (self.lib.f.state_serialize_layout)(self.raw, LAYOUT_EFFECTIVE) }
    }
}

impl Drop for KeyboardState {
    fn drop(&mut self) {
        // SAFETY: our reference, released once.
        unsafe { (self.lib.f.state_unref)(self.raw) };
    }
}

/// A locale's compose sequences (dead keys, the Compose key).
pub struct ComposeTable {
    lib: Arc<Lib>,
    raw: Ptr,
}

impl ComposeTable {
    /// A state for typing through it.
    pub fn state(&self) -> Option<RawComposeState> {
        // SAFETY: the table is alive; the state holds its own reference to it; no flags.
        let raw = unsafe { (self.lib.f.compose_state_new)(self.raw, 0) };
        (!raw.is_null()).then(|| RawComposeState { lib: self.lib.clone(), raw })
    }
}

impl Drop for ComposeTable {
    fn drop(&mut self) {
        // SAFETY: our reference, released once.
        unsafe { (self.lib.f.compose_table_unref)(self.raw) };
    }
}

/// A compose state, as xkbcommon has it (see [`ComposeState`](super::ComposeState)).
pub struct RawComposeState {
    lib: Arc<Lib>,
    raw: Ptr,
}

impl RawComposeState {
    /// Feeds `keysym` in; whether a sequence took it (`XKB_COMPOSE_FEED_ACCEPTED`).
    pub fn feed(&mut self, keysym: u32) -> bool {
        // SAFETY: the state is alive (and ours alone).
        unsafe { (self.lib.f.compose_state_feed)(self.raw, keysym) == 1 }
    }

    pub fn reset(&mut self) {
        // SAFETY: as above.
        unsafe { (self.lib.f.compose_state_reset)(self.raw) };
    }

    /// `XKB_COMPOSE_NOTHING` (0), `COMPOSING` (1), `COMPOSED` (2), `CANCELLED` (3).
    pub fn status(&self) -> i32 {
        // SAFETY: the state is alive.
        unsafe { (self.lib.f.compose_state_get_status)(self.raw) }
    }

    /// The composed text (while composed).
    pub fn utf8(&self) -> String {
        // SAFETY: the state is alive; the buffer is `size` writable bytes.
        written(|buffer, size| unsafe { (self.lib.f.compose_state_get_utf8)(self.raw, buffer, size) })
    }

    /// The composed keysym (while composed; 0 if none or several).
    pub fn one_sym(&self) -> u32 {
        // SAFETY: the state is alive.
        unsafe { (self.lib.f.compose_state_get_one_sym)(self.raw) }
    }
}

impl Drop for RawComposeState {
    fn drop(&mut self) {
        // SAFETY: our reference, released once.
        unsafe { (self.lib.f.compose_state_unref)(self.raw) };
    }
}
