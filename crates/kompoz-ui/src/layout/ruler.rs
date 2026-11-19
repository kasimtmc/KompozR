//! Rulers, as Compose's (`Ruler`, `VerticalRuler`, `HorizontalRuler`, `RulerScope`, `RectRulers`): lines a layout
//! provides with its measure result (`MeasureScope::layout`'s `rulers`), which what it places, however deep, reads
//! while placing (`PlacementScope::current`) in its own coordinates; one that read a ruler is placed again when
//! the value it read changes.

use super::{LayoutCoordinates, PlacementScope};
use crate::node::NodeCoordinator;
use kompoz_graphics::{Density, Offset};
use std::cell::Cell;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

/// A derived ruler's value, from the placement scope and the default asked with (Compose's `calculation`).
pub type RulerCalculation = Rc<dyn Fn(&PlacementScope, f32) -> f32>;

/// What `MeasureScope::layout` provides rulers with (Compose's `rulers: RulerScope.() -> Unit`).
pub type RulersBlock = Rc<dyn Fn(&RulerScope)>;

/// Whether a ruler provider may provide a ruler (Compose's `isRulerProvided`).
pub type IsRulerProvided = Rc<dyn Fn(&Ruler) -> bool>;

/// A provider of rulers one at a time, as they are read (Compose's `rulerProvider: RulerScope.(Ruler) -> Unit`).
pub type RulerProvider = Rc<dyn Fn(&RulerScope, &Ruler)>;

struct RulerData {
    name: Option<String>,
    calculate: Option<RulerCalculation>,
}

fn data(name: Option<String>, calculate: Option<RulerCalculation>) -> Rc<RulerData> {
    Rc::new(RulerData { name, calculate })
}

/// A vertical line a layout provides for what it places to align to horizontally (Compose's VerticalRuler): its
/// value is an x. Known by its identity: clones are the same ruler.
#[derive(Clone)]
pub struct VerticalRuler(Rc<RulerData>);

/// A horizontal line a layout provides for what it places to align to vertically (Compose's HorizontalRuler): its
/// value is a y.
#[derive(Clone)]
pub struct HorizontalRuler(Rc<RulerData>);

/// A vertical or a horizontal ruler (Compose's sealed Ruler).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Ruler {
    Vertical(VerticalRuler),
    Horizontal(HorizontalRuler),
}

macro_rules! ruler_kind {
    ($ty:ident, $variant:ident, $what:literal) => {
        impl $ty {
            /// A ruler whose values layouts provide (Compose's constructor).
            #[allow(clippy::new_without_default)]
            pub fn new() -> $ty {
                $ty(data(None, None))
            }

            /// The same, named (what it prints as).
            pub fn named(name: impl Into<String>) -> $ty {
                $ty(data(Some(name.into()), None))
            }

            /// A ruler worked out from what the placement scope gives, other rulers' values (Compose's `derived`):
            /// `calculation` gets the scope and the default asked with.
            pub fn derived(calculation: impl Fn(&PlacementScope, f32) -> f32 + 'static) -> $ty {
                $ty(data(None, Some(Rc::new(calculation))))
            }

            /// The greatest value of those of `rulers` provided, else the default (Compose's `maxOf`).
            pub fn max_of(rulers: &[$ty]) -> $ty {
                let rulers: Vec<Ruler> = rulers.iter().map(|r| Ruler::$variant(r.clone())).collect();
                $ty::derived(move |scope, default| merge_ruler_values(scope, true, &rulers, default))
            }

            /// The least value of those of `rulers` provided, else the default (Compose's `minOf`).
            pub fn min_of(rulers: &[$ty]) -> $ty {
                let rulers: Vec<Ruler> = rulers.iter().map(|r| Ruler::$variant(r.clone())).collect();
                $ty::derived(move |scope, default| merge_ruler_values(scope, false, &rulers, default))
            }

            /// Its value where `scope` places, else `default` (Compose's `Ruler.current` in a placement scope).
            pub fn current(&self, scope: &PlacementScope, default: f32) -> f32 {
                scope.current(self, default)
            }
        }

        impl PartialEq for $ty {
            fn eq(&self, other: &Self) -> bool {
                Rc::ptr_eq(&self.0, &other.0)
            }
        }

        impl Eq for $ty {}

        impl Hash for $ty {
            fn hash<H: Hasher>(&self, state: &mut H) {
                Rc::as_ptr(&self.0).hash(state);
            }
        }

        impl fmt::Debug for $ty {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                match &self.0.name {
                    Some(name) => write!(f, "{}({name})", $what),
                    None => write!(f, "{}@{:p}", $what, Rc::as_ptr(&self.0)),
                }
            }
        }

        impl From<$ty> for Ruler {
            fn from(ruler: $ty) -> Ruler {
                Ruler::$variant(ruler)
            }
        }

        impl From<&$ty> for Ruler {
            fn from(ruler: &$ty) -> Ruler {
                Ruler::$variant(ruler.clone())
            }
        }
    };
}

ruler_kind!(VerticalRuler, Vertical, "VerticalRuler");
ruler_kind!(HorizontalRuler, Horizontal, "HorizontalRuler");

impl Ruler {
    fn data(&self) -> &Rc<RulerData> {
        match self {
            Ruler::Vertical(r) => &r.0,
            Ruler::Horizontal(r) => &r.0,
        }
    }

    pub(crate) fn calculation(&self) -> Option<RulerCalculation> {
        self.data().calculate.clone()
    }

    /// `coordinate`, provided by `source`, in `target` (Compose's `calculateCoordinate`): an x mapped at the middle
    /// of `source`'s height, a y at the middle of its width.
    pub(crate) fn calculate_coordinate(
        &self,
        coordinate: f32,
        source: &LayoutCoordinates,
        target: &LayoutCoordinates,
    ) -> f32 {
        let size = source.size();
        match self {
            Ruler::Vertical(_) => target.local_position_of(source, Offset::new(coordinate, size.height as f32 / 2.0)).x,
            Ruler::Horizontal(_) => {
                target.local_position_of(source, Offset::new(size.width as f32 / 2.0, coordinate)).y
            }
        }
    }
}

/// The greatest (or least) of the values `rulers` have where `scope` places, else `default` (Compose's
/// `mergeRulerValues`).
fn merge_ruler_values(scope: &PlacementScope, use_greater: bool, rulers: &[Ruler], default: f32) -> f32 {
    let mut value = f32::NAN;
    for ruler in rulers {
        let next = scope.current(ruler.clone(), f32::NAN);
        if value.is_nan() || (use_greater == (next > value)) {
            value = next;
        }
    }
    if value.is_nan() { default } else { value }
}

/// Four rulers bounding a rectangle (Compose's RectRulers): its left, top, right and bottom.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct RectRulers {
    pub left: VerticalRuler,
    pub top: HorizontalRuler,
    pub right: VerticalRuler,
    pub bottom: HorizontalRuler,
    name: Option<Rc<str>>,
}

impl RectRulers {
    /// Four rulers of its own (Compose's `RectRulers()`).
    #[allow(clippy::new_without_default)]
    pub fn new() -> RectRulers {
        RectRulers::of(VerticalRuler::new(), HorizontalRuler::new(), VerticalRuler::new(), HorizontalRuler::new(), None)
    }

    /// The same, named (what it prints as).
    pub fn named(name: &str) -> RectRulers {
        let mut r = RectRulers::new();
        r.name = Some(name.into());
        r
    }

    fn of(
        left: VerticalRuler,
        top: HorizontalRuler,
        right: VerticalRuler,
        bottom: HorizontalRuler,
        name: Option<Rc<str>>,
    ) -> RectRulers {
        RectRulers { left, top, right, bottom, name }
    }

    /// The innermost of `rulers`' rectangles as their values are (Compose's `RectRulers.innermostOf`): the
    /// greatest left and top, the least right and bottom.
    pub fn innermost_of(rulers: &[RectRulers]) -> RectRulers {
        let name = format!("innermostOf({})", rulers.iter().map(|r| format!("{r:?}")).collect::<Vec<_>>().join(", "));
        RectRulers::of(
            VerticalRuler::max_of(&rulers.iter().map(|r| r.left.clone()).collect::<Vec<_>>()),
            HorizontalRuler::max_of(&rulers.iter().map(|r| r.top.clone()).collect::<Vec<_>>()),
            VerticalRuler::min_of(&rulers.iter().map(|r| r.right.clone()).collect::<Vec<_>>()),
            HorizontalRuler::min_of(&rulers.iter().map(|r| r.bottom.clone()).collect::<Vec<_>>()),
            Some(name.into()),
        )
    }

    /// The outermost of them (Compose's `RectRulers.outermostOf`): the least left and top, the greatest right and
    /// bottom.
    pub fn outermost_of(rulers: &[RectRulers]) -> RectRulers {
        let name = format!("outermostOf({})", rulers.iter().map(|r| format!("{r:?}")).collect::<Vec<_>>().join(", "));
        RectRulers::of(
            VerticalRuler::min_of(&rulers.iter().map(|r| r.left.clone()).collect::<Vec<_>>()),
            HorizontalRuler::min_of(&rulers.iter().map(|r| r.top.clone()).collect::<Vec<_>>()),
            VerticalRuler::max_of(&rulers.iter().map(|r| r.right.clone()).collect::<Vec<_>>()),
            HorizontalRuler::max_of(&rulers.iter().map(|r| r.bottom.clone()).collect::<Vec<_>>()),
            Some(name.into()),
        )
    }
}

impl fmt::Debug for RectRulers {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.name {
            Some(name) => write!(f, "RectRulers({name})"),
            None => write!(f, "RectRulers@{:p}", Rc::as_ptr(&self.left.0)),
        }
    }
}

/// What a ruler provider worked out its values from: whether it read its coordinates, and where it was and how big
/// then (its values are worked out again when it moves or is resized).
pub(crate) struct RulerScopeState {
    pub(crate) coordinates_accessed: Cell<bool>,
    pub(crate) position: Cell<Option<kompoz_graphics::IntOffset>>,
    pub(crate) size: Cell<kompoz_graphics::IntSize>,
}

impl Default for RulerScopeState {
    fn default() -> Self {
        RulerScopeState {
            coordinates_accessed: Cell::new(false),
            position: Cell::new(None),
            size: Cell::new(kompoz_graphics::IntSize::Zero),
        }
    }
}

/// What a layout provides its rulers' values in (Compose's RulerScope): its coordinates, and `provides`. It derefs
/// to the density.
pub struct RulerScope<'a> {
    pub(crate) coordinator: &'a Rc<NodeCoordinator>,
    pub(crate) state: &'a RulerScopeState,
    pub(crate) density: Density,
    /// Providing the coordinator's lookahead rulers (its lookahead delegate's: Compose's LookaheadDelegate as the
    /// LookaheadCapablePlaceable providing).
    pub(crate) lookahead: bool,
}

impl RulerScope<'_> {
    /// The coordinates of the layout providing (Compose's `RulerScope.coordinates`): reading them works the
    /// values out again whenever it moves or is resized.
    pub fn coordinates(&self) -> LayoutCoordinates {
        self.state.coordinates_accessed.set(true);
        let coordinates = if self.lookahead {
            LayoutCoordinates::lookahead_of(self.coordinator.clone())
        } else {
            LayoutCoordinates::of(self.coordinator.clone())
        };
        if self.state.position.get().is_none() {
            self.state.position.set(Some(coordinates.position_in_window().round()));
            self.state.size.set(coordinates.size());
        }
        self.coordinator.on_coordinates_used();
        coordinates
    }

    /// `ruler` at `value` (px from the layout's left for a vertical one, its top for a horizontal one): Compose's
    /// `Ruler.provides`.
    pub fn provides(&self, ruler: impl Into<Ruler>, value: f32) {
        self.coordinator.provide_ruler_value(ruler.into(), value, self.lookahead);
    }

    /// A vertical ruler at `value` from the start: from the left left to right, from the right right to left
    /// (Compose's `providesRelative`).
    pub fn provides_relative(&self, ruler: &VerticalRuler, value: f32) {
        self.coordinator.provide_relative_ruler_value(ruler.into(), value, self.lookahead);
    }
}

impl std::ops::Deref for RulerScope<'_> {
    type Target = Density;
    fn deref(&self) -> &Density {
        &self.density
    }
}
