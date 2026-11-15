//! Composables inside text (Compose's InlineTextContent.kt and AnnotatedStringResolveInlineContent.kt): a stretch
//! of text marked by `append_inline_content` replaced by a composable laid out in a placeholder's room.

use kompoz_graphics::Constraints;
use kompoz_runtime::kompozable;
use kompoz_text::{AnnotatedString, INLINE_CONTENT_TAG, Placeholder, Range};
use kompoz_ui::Modifier;
use kompoz_ui::layout::{Measurable, MeasurePolicy, MeasureResult, MeasureScope, layout_into};
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

/// A composable put into text (Compose's InlineTextContent): the room it takes in the line ([`Placeholder`]: its
/// size and how it sits in the line; it is measured to that size), and `children`, given the alternate text of
/// the stretch it replaces.
#[derive(Clone)]
pub struct InlineTextContent {
    pub placeholder: Placeholder,
    pub children: Rc<dyn Fn(&str)>,
}

impl InlineTextContent {
    pub fn new(placeholder: Placeholder, children: impl Fn(&str) + 'static) -> InlineTextContent {
        InlineTextContent { placeholder, children: Rc::new(children) }
    }
}

/// Equal with the same placeholder and the same children (the same closure).
impl PartialEq for InlineTextContent {
    fn eq(&self, o: &InlineTextContent) -> bool {
        self.placeholder == o.placeholder
            && std::ptr::eq(Rc::as_ptr(&self.children) as *const (), Rc::as_ptr(&o.children) as *const ())
    }
}

impl fmt::Debug for InlineTextContent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("InlineTextContent").field("placeholder", &self.placeholder).finish_non_exhaustive()
    }
}

/// The inline content of a text: what `append_inline_content` ids name.
pub type InlineContentMap = HashMap<String, InlineTextContent>;

/// A composable of inline content on a stretch of text (Compose's InlineContentRange).
pub(crate) type InlineContentRange = Range<Rc<dyn Fn(&str)>>;

/// Whether `text` marks any inline content (Compose's hasInlineContent).
pub(crate) fn has_inline_content(text: &AnnotatedString) -> bool {
    text.has_string_annotations(INLINE_CONTENT_TAG, 0, text.len())
}

/// The placeholders and composables of `text`'s inline content that `inline_content` has (Compose's
/// resolveInlineContent): one of each for every marked stretch whose id it has; the others left as text.
pub(crate) fn resolve_inline_content(
    text: &AnnotatedString,
    inline_content: &InlineContentMap,
) -> (Vec<Range<Placeholder>>, Vec<InlineContentRange>) {
    if inline_content.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let mut placeholders = Vec::new();
    let mut composables = Vec::new();
    for annotation in text.get_string_annotations(INLINE_CONTENT_TAG, 0, text.len()) {
        if let Some(content) = inline_content.get(&annotation.item) {
            placeholders.push(Range::new(content.placeholder, annotation.start, annotation.end));
            composables.push(Range::new(content.children.clone(), annotation.start, annotation.end));
        }
    }
    (placeholders, composables)
}

/// Each inline content in a layout of its own, as big as the room it is measured in, its children placed in it
/// (Compose's InlineChildren).
#[kompozable]
pub(crate) fn InlineChildren(text: AnnotatedString, inline_contents: Vec<InlineContentRange>) {
    for range in inline_contents {
        let alternate = text.text()[range.start..range.end].to_string();
        let content = range.item.clone();
        layout_into(Modifier, INLINE_CHILD_POLICY.with(Rc::clone), false, move || content(&alternate));
    }
}

thread_local! {
    static INLINE_CHILD_POLICY: Rc<dyn MeasurePolicy> = Rc::new(InlineChildPolicy);
}

/// An inline content's layout: its children measured in its room, it as big as the room (Compose's InlineChildren
/// measure policy).
struct InlineChildPolicy;

impl MeasurePolicy for InlineChildPolicy {
    fn measure(&self, _: &MeasureScope, measurables: &[&dyn Measurable], constraints: Constraints) -> MeasureResult {
        let placeables: Vec<_> = measurables.iter().map(|m| m.measure(constraints)).collect();
        MeasureResult::new(constraints.max_width, constraints.max_height, Vec::new(), move |p| {
            for placeable in &placeables {
                p.place_relative(placeable, 0, 0);
            }
        })
    }
}
