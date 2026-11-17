//! Where a scrollable brings what asks to be seen (Compose's BringIntoViewSpec.kt and its desktop actual): how far
//! it scrolls for an item at an offset of a size in a container of a size.

use kompoz_animation::{AnimationSpec, spring};
use kompoz_runtime::KompozitionLocal;
use std::any::Any;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

/// How a scrollable brings something into view (Compose's BringIntoViewSpec): how far to scroll for an item
/// `offset` px from the container's start (its leading edge), `size` long, in a container `container_size` long
/// (positive: forward). By default just enough to show it, nothing if it shows or is bigger than the container.
/// Shared as `Rc<dyn BringIntoViewSpec>`; equal when their types say so.
pub trait BringIntoViewSpec: Any + BringIntoViewSpecEq {
    /// The spring a scrollable animates along to bring things into view (Compose's deprecated
    /// `scrollAnimationSpec`: its customization is no longer supported; the default spring).
    fn scroll_animation_spec(&self) -> Arc<dyn AnimationSpec<f32>> {
        default_scroll_animation_spec()
    }

    /// How far to scroll (Compose's `calculateScrollDistance`).
    fn calculate_scroll_distance(&self, offset: f32, size: f32, container_size: f32) -> f32 {
        default_calculate_scroll_distance(offset, size, container_size)
    }
}

/// How specs compare (a blanket impl for every `PartialEq` one).
#[doc(hidden)]
pub trait BringIntoViewSpecEq {
    fn bring_into_view_spec_eq(&self, other: &dyn Any) -> bool;
    fn as_any(&self) -> &dyn Any;
}

impl<T: PartialEq + Any> BringIntoViewSpecEq for T {
    fn bring_into_view_spec_eq(&self, other: &dyn Any) -> bool {
        other.downcast_ref::<T>().is_some_and(|o| self == o)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl PartialEq for dyn BringIntoViewSpec {
    fn eq(&self, other: &dyn BringIntoViewSpec) -> bool {
        self.bring_into_view_spec_eq(other.as_any())
    }
}

impl fmt::Debug for dyn BringIntoViewSpec {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "BringIntoViewSpec")
    }
}

/// Whether two optional specs are the same.
pub(crate) fn same_spec(a: &Option<Rc<dyn BringIntoViewSpec>>, b: &Option<Rc<dyn BringIntoViewSpec>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => **a == **b,
        (None, None) => true,
        _ => false,
    }
}

/// The default spring (Compose's `DefaultScrollAnimationSpec`).
fn default_scroll_animation_spec() -> Arc<dyn AnimationSpec<f32>> {
    Arc::new(spring::<f32>())
}

/// Compose's `defaultCalculateScrollDistance`: nothing for an item that shows or is bigger than the container,
/// else the least scroll putting one of its edges on the container's.
pub fn default_calculate_scroll_distance(offset: f32, size: f32, container_size: f32) -> f32 {
    let trailing_edge = offset + size;
    let leading_edge = offset;
    if leading_edge >= 0.0 && trailing_edge <= container_size {
        // Visible already: no need to scroll.
        0.0
    } else if leading_edge < 0.0 && trailing_edge > container_size {
        // Visible but bigger than the container: no scroll.
        0.0
    } else if leading_edge.abs() < (trailing_edge - container_size).abs() {
        // The least scroll putting an edge on the container's.
        leading_edge
    } else {
        trailing_edge - container_size
    }
}

/// The default spec (Compose's DefaultBringIntoViewSpec).
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct DefaultBringIntoViewSpec;

impl BringIntoViewSpec for DefaultBringIntoViewSpec {}

/// The spec the scrollables below bring things into view by unless given one (Compose's LocalBringIntoViewSpec,
/// static: Compose for desktop's, the default; Android's TV pivots, stage 15's).
pub static LocalBringIntoViewSpec: KompozitionLocal<Rc<dyn BringIntoViewSpec>> =
    KompozitionLocal::new_static(|| Rc::new(DefaultBringIntoViewSpec));
