//! Windows of the UI's own (Compose's Popup and Dialog): their roots are emitted inline where they are composed
//! (so composition locals flow into them), split out of their anchor's children, laid out against the window,
//! drawn after the main tree and hit first (C++'s overlays).
//!
//! What is here is the host's overlay list and its passes; `Popup`, `Dialog`, their position providers and
//! properties, and the splitting out are the windows package of this stage.

use crate::graphics_context::GraphicsContext;
use crate::node::LayoutNode;
use std::rc::{Rc, Weak};

/// The overlays of a host: the roots of its popups and dialogs, bottom first.
#[derive(Default)]
pub struct Overlays {
    pub(crate) roots: Vec<Weak<LayoutNode>>,
}

impl Overlays {
    /// The overlays' roots that are still in the tree, bottom first.
    pub fn roots(&self) -> Vec<Rc<LayoutNode>> {
        self.roots.iter().filter_map(Weak::upgrade).filter(|n| n.is_attached()).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.roots().is_empty()
    }

    /// Draws them over the main tree, bottom first.
    pub(crate) fn draw(&self, canvas: &mut dyn GraphicsContext) {
        for root in self.roots() {
            if root.is_placed() {
                root.draw(canvas);
            }
        }
    }

    pub(crate) fn prune(&mut self) {
        self.roots.retain(|w| w.upgrade().is_some_and(|n| n.is_attached()));
    }
}
