//! `PointerMatcher` (Compose for desktop's PointerMatcher.kt): which pointer events a gesture takes — a mouse's
//! button, fingers, a stylus (with or without a button), an eraser, several of them (`+`), or a test of one's own.
//! The desktop's `Modifier.on_click`, `Modifier.on_drag` and the matcher-taking gesture detectors use it.

use kompoz_ui::input::{PointerButton, PointerEvent, PointerType};
use std::borrow::Cow;
use std::fmt;
use std::rc::Rc;

/// A condition (or several, any of which does) a pointer event has to meet to count for a gesture (Compose's
/// `fun interface PointerMatcher`). Made by its associated functions and constants (Kotlin's companion:
/// `PointerMatcher.mouse(PointerButton.Primary) + PointerMatcher.touch`), or from a test of one's own
/// ([`PointerMatcher::new`], Kotlin's `PointerMatcher { event -> … }`). Cheap to clone; two are equal when they are
/// the same condition (the same closure for one's own).
#[derive(Clone)]
pub struct PointerMatcher {
    kind: Kind,
}

#[derive(Clone)]
enum Kind {
    /// Compose's PointerTypeAndButtonMatcher.
    TypeAndButton { pointer_type: PointerType, button: Option<PointerButton>, match_all_buttons: bool },
    /// Compose's CombinedPointerMatcher.
    Combined(Cow<'static, [PointerMatcher]>),
    /// One's own.
    Custom(Rc<dyn Fn(&PointerEvent) -> bool>),
}

impl PartialEq for Kind {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Kind::TypeAndButton { pointer_type: a, button: b, match_all_buttons: c },
                Kind::TypeAndButton { pointer_type: x, button: y, match_all_buttons: z },
            ) => a == x && b == y && c == z,
            (Kind::Combined(a), Kind::Combined(b)) => a == b,
            (Kind::Custom(a), Kind::Custom(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl PartialEq for PointerMatcher {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
    }
}

impl fmt::Debug for PointerMatcher {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.kind {
            Kind::TypeAndButton { pointer_type, button, match_all_buttons } => f
                .debug_struct("PointerMatcher")
                .field("pointer_type", pointer_type)
                .field("button", button)
                .field("match_all_buttons", match_all_buttons)
                .finish(),
            Kind::Combined(sources) => f.debug_tuple("PointerMatcher::Combined").field(sources).finish(),
            Kind::Custom(_) => f.write_str("PointerMatcher(custom)"),
        }
    }
}

const fn type_and_button(
    pointer_type: PointerType,
    button: Option<PointerButton>,
    match_all_buttons: bool,
) -> PointerMatcher {
    PointerMatcher { kind: Kind::TypeAndButton { pointer_type, button, match_all_buttons } }
}

impl PointerMatcher {
    /// Events of a stylus (any button or none: Compose's `stylus` value is `stylus(button = null)`, which takes
    /// events of no button).
    /// Kotlin: `stylus`
    pub const Stylus: PointerMatcher = type_and_button(PointerType::Stylus, None, false);
    /// Events of fingers (Compose's `touch`).
    /// Kotlin: `touch`
    pub const Touch: PointerMatcher = type_and_button(PointerType::Touch, None, false);
    /// Events of an eraser, any button (Compose's `eraser`).
    /// Kotlin: `eraser`
    pub const Eraser: PointerMatcher = type_and_button(PointerType::Eraser, None, true);
    /// The common case (Compose's `Primary`): a mouse's primary button, fingers, a stylus without a button, an
    /// eraser.
    pub const Primary: PointerMatcher = PointerMatcher {
        kind: Kind::Combined(Cow::Borrowed(&[
            type_and_button(PointerType::Mouse, Some(PointerButton::Primary), false),
            PointerMatcher::Touch,
            PointerMatcher::Stylus,
            PointerMatcher::Eraser,
        ])),
    };

    /// A test of one's own (Kotlin's `PointerMatcher { event -> … }`).
    pub fn new(matches: impl Fn(&PointerEvent) -> bool + 'static) -> PointerMatcher {
        PointerMatcher { kind: Kind::Custom(Rc::new(matches)) }
    }

    /// Events all of whose pointers are of `pointer_type`, of `button` (none: events of no button, as a finger's)
    /// (Compose's `pointer(pointerType, button)`).
    pub const fn pointer(pointer_type: PointerType, button: Option<PointerButton>) -> PointerMatcher {
        type_and_button(pointer_type, button, false)
    }

    /// A mouse's `button` (Compose's `mouse(button)`).
    pub const fn mouse(button: PointerButton) -> PointerMatcher {
        type_and_button(PointerType::Mouse, Some(button), false)
    }

    /// A stylus's `button` (none: a stylus without one pressed; Compose's `stylus(button)`).
    pub const fn stylus(button: Option<PointerButton>) -> PointerMatcher {
        type_and_button(PointerType::Stylus, button, false)
    }

    /// Whether `event` meets it (Compose's `matches`).
    pub fn matches(&self, event: &PointerEvent) -> bool {
        match &self.kind {
            Kind::TypeAndButton { pointer_type, button, match_all_buttons } => {
                (*match_all_buttons || event.button == *button)
                    && event.changes.iter().all(|c| c.r#type == *pointer_type)
            }
            Kind::Combined(sources) => sources.iter().any(|m| m.matches(event)),
            Kind::Custom(f) => f(event),
        }
    }

    /// This one or `other` (Compose's `plus`): a combination of both's conditions, flattened.
    pub fn plus(&self, other: &PointerMatcher) -> PointerMatcher {
        let mut sources = Vec::new();
        for m in [self, other] {
            match &m.kind {
                Kind::Combined(s) => sources.extend(s.iter().cloned()),
                _ => sources.push(m.clone()),
            }
        }
        PointerMatcher { kind: Kind::Combined(Cow::Owned(sources)) }
    }
}

impl std::ops::Add for PointerMatcher {
    type Output = PointerMatcher;
    fn add(self, other: PointerMatcher) -> PointerMatcher {
        self.plus(&other)
    }
}

impl Default for PointerMatcher {
    /// [`PointerMatcher::Primary`].
    fn default() -> Self {
        PointerMatcher::Primary
    }
}
