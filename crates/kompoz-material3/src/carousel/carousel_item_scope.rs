//! The scope a carousel item's content is in (Compose's `CarouselItemScope.kt`).

use super::carousel_state::CarouselItemDrawInfo;

use kompoz_foundation::prelude::*;
use kompoz_graphics::{BorderStroke, Density, GenericShape, LayoutDirection, Offset, Path, Shape, Size};
use kompoz_runtime::{kompozable_block, named, remember_keyed};
use kompoz_ui::Modifier;
use kompoz_ui::platform::LocalDensity;
use std::fmt;
use std::sync::Arc;

/// The scope of a carousel item's content (Compose's sealed interface `CarouselItemScope` and its one
/// implementation `CarouselItemScopeImpl`: a value here, as the crate's other sealed scopes are; Kotlin's member
/// extensions `Modifier.maskClip(…)` are its methods taking the modifier first): the item's draw info, and
/// modifiers clipping and bordering content to the item's mask.
#[derive(Clone, PartialEq)]
pub struct CarouselItemScope {
    item_info: CarouselItemDrawInfo,
}

impl fmt::Debug for CarouselItemScope {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("CarouselItemScope").field("carousel_item_draw_info", &self.item_info).finish()
    }
}

impl CarouselItemScope {
    pub(crate) fn new(item_info: CarouselItemDrawInfo) -> CarouselItemScope {
        CarouselItemScope { item_info }
    }

    /// The item's draw info — its size, least and most size, mask (Compose's `carouselItemDrawInfo`). It changes at
    /// every scroll: read it where content draws or lays out, not in a composition (which would recompose at every
    /// change).
    pub fn carousel_item_draw_info(&self) -> CarouselItemDrawInfo {
        self.item_info.clone()
    }
}

#[named]
impl CarouselItemScope {
    /// `modifier` clipped to `shape` in the item's size across and its mask along the main axis (Compose's
    /// composable `Modifier.maskClip`).
    ///
    /// Kotlin: `maskClip(shape: Shape)`
    #[track_caller]
    pub fn mask_clip(&self, modifier: Modifier, shape: Arc<dyn Shape>) -> Modifier {
        let mask_shape = self.remember_mask_shape(shape);
        modifier.clip(mask_shape)
    }

    /// `modifier` with `border` drawn along `shape` in the item's size across and its mask along the main axis
    /// (Compose's composable `Modifier.maskBorder`).
    ///
    /// Kotlin: `maskBorder(border: BorderStroke, shape: Shape)`
    #[track_caller]
    pub fn mask_border(&self, modifier: Modifier, border: BorderStroke, shape: Arc<dyn Shape>) -> Modifier {
        let mask_shape = self.remember_mask_shape(shape);
        modifier.border_stroke_with(border, mask_shape)
    }

    /// `shape` as a remembered [`GenericShape`] whose bounds are the intersection of the item's mask and its size
    /// (Compose's composable `rememberMaskShape`): for modifiers other than [`mask_clip`](Self::mask_clip) and
    /// [`mask_border`](Self::mask_border) whose shape should follow the item's mask. As Compose's, it is remembered
    /// by the draw info and the density only: a later `shape` isn't taken.
    ///
    /// Kotlin: `rememberMaskShape(shape: Shape)`
    #[track_caller]
    pub fn remember_mask_shape(&self, shape: Arc<dyn Shape>) -> GenericShape {
        let item_info = self.item_info.clone();
        kompozable_block!({
            let density = LocalDensity.current();
            remember_keyed((item_info.clone(), density), move || mask_shape(item_info, density, shape))
        })
    }
}

/// The shape of [`CarouselItemScope::remember_mask_shape`]: `shape` in the intersection of the item's mask and size.
fn mask_shape(item_info: CarouselItemDrawInfo, density: Density, shape: Arc<dyn Shape>) -> GenericShape {
    GenericShape(move |path: &mut Path, size: Size, direction: LayoutDirection| {
        let rect = item_info.mask_rect().intersect(size.to_rect());
        path.add_outline(&shape.create_outline(rect.size(), direction, &density));
        path.translate(Offset::new(rect.left, rect.top));
    })
}
