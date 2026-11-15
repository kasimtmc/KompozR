//! `Canvas`, as Compose's: a space to draw in.

use crate::layout::Spacer;
use kompoz_runtime::kompozable;
use kompoz_ui::Modifier;
use kompoz_ui::draw::DrawScope;

/// A space of its modifier's size drawn by `on_draw` (Compose's `Canvas(modifier) { … }`: a Spacer drawing behind);
/// `content_description` (Compose's other Canvas) is what a screen reader says of it.
#[kompozable]
pub fn Canvas(
    modifier: Modifier,
    #[default(None)] content_description: Option<String>,
    on_draw: impl Fn(&mut DrawScope) + 'static,
) {
    let mut m = modifier.draw_behind(on_draw);
    if let Some(description) = content_description {
        m = m.semantics(move |c| c.set_content_description(description.clone()));
    }
    Spacer(m);
}
