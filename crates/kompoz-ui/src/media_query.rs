//! What the app runs on, to adapt to (Compose's `UiMediaScope`, `mediaQuery` and `derivedMediaQuery`): the window's
//! size, the most precise pointer there is, the keyboard, a microphone and a camera, how far away the screen is
//! seen, a foldable's posture. The host keeps it from what the platform tells it ([`MediaInfo`], the viewport, the
//! keyboard's insets), each part in a state of its own: what reads only the pointer isn't composed again as the
//! window is resized.
//!
//! ```text
//! if media_query(|m| m.pointer_precision() == PointerPrecision::Fine) {
//!     Text("Üzerine gelince ayrıntı görünür");
//! }
//! // Composed again only when the answer changes, not at each step of a resize:
//! let wide = derived_media_query(|m| m.window_width() >= 600.dp());
//! ```

use kompoz_graphics::Dp;
use kompoz_runtime::{
    DerivedState, KompozitionLocal, MutableState, derived_state_of, kompozable, mutable_state_of, remember_keyed,
    remember_updated_state,
};
use std::rc::Rc;

/// How the window lies on a foldable (Compose's UiMediaScope.Posture): flat (and every other device), across a
/// horizontal fold (tabletop, like a laptop) or a vertical one (book).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Posture {
    #[default]
    Flat,
    Tabletop,
    Book,
}

/// The most precise pointing device there is (Compose's UiMediaScope.PointerPrecision): fine (a mouse, a
/// trackpad, a stylus), coarse (a touch screen), blunt (a game controller's stick), or none.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum PointerPrecision {
    #[default]
    None,
    Blunt,
    Coarse,
    Fine,
}

/// The keyboard (Compose's UiMediaScope.KeyboardKind): a physical one if one is attached, else the on-screen one
/// while it shows, else none.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum KeyboardKind {
    #[default]
    None,
    Virtual,
    Physical,
}

/// How far the screen is seen from (Compose's UiMediaScope.ViewingDistance): near (phones, tablets, laptops,
/// desktops), medium (cars, a docked tablet), far (TVs).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum ViewingDistance {
    #[default]
    Near,
    Medium,
    Far,
}

/// What the platform knows of the device (C++'s MediaInfo, the host's `set_media_info`): the window's size and
/// the on-screen keyboard come from the viewport and the insets.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct MediaInfo {
    pub pointer_precision: PointerPrecision,
    pub physical_keyboard: bool,
    pub has_microphone: bool,
    pub has_camera: bool,
    pub viewing_distance: ViewingDistance,
    pub window_posture: Posture,
}

struct MediaStates {
    width: MutableState<Dp>,
    height: MutableState<Dp>,
    ime_visible: MutableState<bool>,
    info: MutableState<MediaInfo>,
}

/// The app's media (Compose's UiMediaScope), each part read through a state. Cheap to clone; equal when the same.
#[derive(Clone)]
pub struct UiMediaScope(Rc<MediaStates>);

impl UiMediaScope {
    /// One of a window 0 by 0, no pointer, no keyboard.
    pub fn new() -> UiMediaScope {
        UiMediaScope(Rc::new(MediaStates {
            width: mutable_state_of(Dp(0.0)),
            height: mutable_state_of(Dp(0.0)),
            ime_visible: mutable_state_of(false),
            info: mutable_state_of(MediaInfo::default()),
        }))
    }

    pub fn window_posture(&self) -> Posture {
        self.0.info.get().window_posture
    }
    /// The window's width; it changes often: query it in [`derived_media_query`].
    pub fn window_width(&self) -> Dp {
        self.0.width.get()
    }
    pub fn window_height(&self) -> Dp {
        self.0.height.get()
    }
    pub fn pointer_precision(&self) -> PointerPrecision {
        self.0.info.get().pointer_precision
    }
    pub fn keyboard_kind(&self) -> KeyboardKind {
        if self.0.info.get().physical_keyboard {
            KeyboardKind::Physical
        } else if self.0.ime_visible.get() {
            KeyboardKind::Virtual
        } else {
            KeyboardKind::None
        }
    }
    pub fn has_microphone(&self) -> bool {
        self.0.info.get().has_microphone
    }
    pub fn has_camera(&self) -> bool {
        self.0.info.get().has_camera
    }
    pub fn viewing_distance(&self) -> ViewingDistance {
        self.0.info.get().viewing_distance
    }

    /// The device as the platform told it last (read by no one).
    pub fn info(&self) -> MediaInfo {
        self.0.info.peek()
    }

    /// The host's: the window (dp), whether the on-screen keyboard shows, the device; what changed is told to what
    /// read it.
    pub fn update(&self, width: Dp, height: Dp, ime_visible: bool, info: MediaInfo) {
        let s = &self.0;
        if s.width.peek() != width {
            s.width.set(width);
        }
        if s.height.peek() != height {
            s.height.set(height);
        }
        if s.ime_visible.peek() != ime_visible {
            s.ime_visible.set(ime_visible);
        }
        if s.info.peek() != info {
            s.info.set(info);
        }
    }
}

impl Default for UiMediaScope {
    fn default() -> Self {
        UiMediaScope::new()
    }
}

impl PartialEq for UiMediaScope {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl std::fmt::Debug for UiMediaScope {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "UiMediaScope({:?})", self.info())
    }
}

/// The app's media (Compose's LocalUiMediaScope): the host's.
pub static LocalUiMediaScope: KompozitionLocal<UiMediaScope> = KompozitionLocal::new_static(UiMediaScope::new);

/// `query` against the app's media now (Compose's `mediaQuery`): the caller is composed again when what it read
/// changes.
pub fn media_query<T>(query: impl FnOnce(&UiMediaScope) -> T) -> T {
    query(&LocalUiMediaScope.current())
}

/// The same as a state that changes only when the answer does (Compose's `derivedMediaQuery`): for queries of the
/// window's size, which changes with every step of a resize. The latest `query` is the one asked.
#[kompozable]
pub fn derived_media_query<T: PartialEq + Clone + 'static>(
    query: impl Fn(&UiMediaScope) -> T + 'static,
) -> DerivedState<T> {
    let media = LocalUiMediaScope.current();
    let updated_query = remember_updated_state!(Rc::new(query) as Rc<dyn Fn(&UiMediaScope) -> T>);
    let identity = Rc::as_ptr(&media.0) as usize;
    remember_keyed(identity, move || derived_state_of(move || updated_query.get()(&media)))
}
