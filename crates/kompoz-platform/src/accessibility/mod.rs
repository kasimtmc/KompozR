//! What every platform's accessibility bridge is made of, whatever it speaks (AT-SPI on Linux; UI Automation,
//! Android's node provider, the web's ARIA mirror and NSAccessibility as their platforms come): the semantics tree a
//! frame publishes, kept as a [`Snapshot`] (kompoz-ui's [`AccessibilityNode`](kompoz_ui::semantics::AccessibilityNode)s,
//! each one's parent and children, the window it is in), what changed from one snapshot to the next ([`changes`]:
//! subtrees come and gone, names, descriptions, states, values, texts typed or taken away, live regions to say, the
//! focus moved), and a text's pieces by code points ([`text`]: what screen readers read by characters, words and
//! lines). Each bridge turns these into its own protocol's objects and events.

mod diff;
mod snapshot;
pub mod text;

pub use diff::{Change, changes};
pub use snapshot::{Snapshot, Window};
