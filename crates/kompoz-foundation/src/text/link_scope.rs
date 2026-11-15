//! Links in a text (Compose's TextLinkScope.kt and LinkStateInteractionSourceObserver): a node over each link —
//! clipped to the link's shape, hoverable, clickable (by touch, mouse, keyboard and accessibility) with a hand
//! pointer — that opens its URL or tells its listener, and the link's styles applied to the text as it is focused,
//! hovered or pressed.

use crate::clickable::ClickableModifiers;
use crate::hoverable::HoverableModifiers;
use crate::interaction::{
    FocusInteraction, HoverInteraction, Interaction, InteractionSource, MutableInteractionSource, PressInteraction,
};
use crate::layout::Box;
use kompoz_coroutines::LaunchedEffect;
use kompoz_graphics::{Density, IntOffset, Offset, Outline, OutlineShape, Path};
use kompoz_runtime::{DisposableEffect, MutableState, key, kompozable, mutable_state_of, on_dispose, remember};
use kompoz_text::{AnnotatedString, Annotation, LinkAnnotation, Range, SpanStyle, TextLayoutResult, TextLinkStyles};
use kompoz_ui::layout::{Measurable, ParentDataModifierNode};
use kompoz_ui::platform::{LocalUriHandler, PointerIcon, UriHandler};
use kompoz_ui::semantics::SemanticsProperties;
use kompoz_ui::{Modifier, ModifierNode, ModifierNodeElement, node_capabilities};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

/// A link of a text (Compose's LinkRange).
pub(crate) type LinkRange = Range<LinkAnnotation>;

/// What restyles a text's links ([`TextLinkScope`]'s annotators).
type Annotator = dyn Fn(&mut TextAnnotatorScope);

/// The links of a text (Compose's TextLinkScope): the text with its links' styles, the layout the links' nodes are
/// placed over, and the styles their states add. Made only for a text that has links.
pub(crate) struct TextLinkScope {
    /// The text with each styled link's style after it (Compose's `text`), restyled by the annotators.
    text: RefCell<AnnotatedString>,
    /// The text's last layout (Compose's `textLayoutResult` state).
    text_layout_result: RefCell<Option<TextLayoutResult>>,
    /// Bumped when it is laid out anew: what reads the layout reads it, and runs again.
    layout_generation: MutableState<u64>,
    /// The styles the links' states add (Compose's `annotators`).
    annotators: RefCell<Vec<Rc<Annotator>>>,
    /// Bumped when they change: composing the text reads it.
    annotators_generation: MutableState<u64>,
}

impl TextLinkScope {
    /// The scope of `initial_text`'s links: each link with styles followed by a span of its style (the default
    /// span style where it has none for the plain state), so the states' styles can replace it.
    pub(crate) fn new(initial_text: AnnotatedString) -> TextLinkScope {
        let text = initial_text.flat_map_annotations(|a| match &a.item {
            Annotation::Link(link) if !is_null_or_empty(link.styles()) => {
                let style = link.styles().and_then(|s| s.style.clone()).unwrap_or(SpanStyle::Default);
                vec![a.clone(), Range::new(Annotation::SpanStyle(style), a.start, a.end)]
            }
            _ => vec![a.clone()],
        });
        TextLinkScope {
            text: RefCell::new(text),
            text_layout_result: RefCell::new(None),
            layout_generation: mutable_state_of(0),
            annotators: RefCell::new(Vec::new()),
            annotators_generation: mutable_state_of(0),
        }
    }

    /// The text's last layout, read (what reads it runs again when it is laid out anew).
    pub(crate) fn text_layout_result(&self) -> Option<TextLayoutResult> {
        self.layout_generation.get();
        self.text_layout_result.borrow().clone()
    }

    /// The text was laid out anew.
    pub(crate) fn set_text_layout_result(&self, result: &TextLayoutResult) {
        *self.text_layout_result.borrow_mut() = Some(result.clone());
        self.layout_generation.update(|g| *g += 1);
    }

    /// Whether the links' nodes are measured: the layout is of the text it styles (a substitution measured before
    /// the text composes again is a plain text, and its links go away then) (Compose's shouldMeasureLinks).
    pub(crate) fn should_measure_links(&self) -> bool {
        let layout = self.text_layout_result();
        layout.is_some_and(|r| *self.text.borrow() == r.layout_input.text)
    }

    /// `link` cut to the visible text of `layout` (Compose's calculateVisibleLinkRange): none where it is all past
    /// it (its paragraph cut off by max lines), its end at the last visible offset where it reaches past it.
    fn calculate_visible_link_range(link: &LinkRange, layout: &TextLayoutResult) -> Option<LinkRange> {
        let last_offset = layout.get_line_end(layout.line_count().checked_sub(1)?, false);
        (link.start < last_offset).then(|| link.moved(link.start, link.end.min(last_offset)))
    }

    /// The node measured to the bounds of `link`'s text and placed over it (Compose's Modifier.textRange).
    fn text_range(self: &Rc<Self>, link: &LinkRange) -> Modifier {
        let (scope, link) = (self.clone(), link.clone());
        Modifier::element(TextRangeLayoutElement {
            measure_policy: Rc::new(move |m: &TextRangeLayoutMeasureScope| {
                let Some(layout) = scope.text_layout_result() else {
                    return m.layout(0, 0, || IntOffset::Zero);
                };
                let Some(range) = TextLinkScope::calculate_visible_link_range(&link, &layout) else {
                    return m.layout(0, 0, || IntOffset::Zero);
                };
                let bounds = layout.get_path_for_range(range.start, range.end).get_bounds().round_to_int_rect();
                let top_left = IntOffset::new(bounds.left, bounds.top);
                m.layout(bounds.width(), bounds.height(), move || top_left)
            }),
        })
    }

    /// The node cut to the shape of `link`'s text (Compose's Modifier.clipLink).
    fn clip_link(self: &Rc<Self>, link: &LinkRange) -> Modifier {
        let (scope, link) = (self.clone(), link.clone());
        Modifier.graphics_layer_block(move |layer| {
            if let Some(path) = scope.path_for_range_in_range_coordinates(&link) {
                let shape = OutlineShape(move |_, _, _: &Density| Outline::Generic(path.clone()));
                layer.shape = Arc::new(shape);
                layer.clip = true;
            }
        })
    }

    /// The shape of `link`'s text from the top left of its node (Compose's pathForRangeInRangeCoordinates): on one
    /// line from its leftmost character, on more from the text's left; from its first character's top.
    fn path_for_range_in_range_coordinates(&self, link: &LinkRange) -> Option<Path> {
        if !self.should_measure_links() {
            return None;
        }
        let layout = self.text_layout_result()?;
        let range = TextLinkScope::calculate_visible_link_range(link, &layout)?;
        let mut path = layout.get_path_for_range(range.start, range.end);
        let first = layout.get_bounding_box(range.start);
        let last_char = previous_char_boundary(layout.layout_input.text.text(), range.end);
        let last = layout.get_bounding_box(last_char);
        let x = if layout.get_line_for_offset(range.start) == layout.get_line_for_offset(last_char) {
            last.left.min(first.left)
        } else {
            0.0
        };
        path.translate(Offset::new(-x, -first.top));
        Some(path)
    }

    /// The text with the links' states' styles over it (Compose's applyAnnotators): read in composition, so the
    /// text composes again when they change.
    pub(crate) fn apply_annotators(&self) -> AnnotatedString {
        self.annotators_generation.get();
        let annotators = self.annotators.borrow().clone();
        if annotators.is_empty() {
            return self.text.borrow().clone();
        }
        let mut scope = TextAnnotatorScope::new(self.text.borrow().clone());
        for a in &annotators {
            a(&mut scope);
        }
        *self.text.borrow_mut() = scope.styled_text.clone();
        scope.styled_text
    }

    fn add_annotator(&self, annotator: Rc<Annotator>) {
        self.annotators.borrow_mut().push(annotator);
        self.annotators_generation.update(|g| *g += 1);
    }

    fn remove_annotator(&self, annotator: &Rc<Annotator>) {
        self.annotators.borrow_mut().retain(|a| !Rc::ptr_eq(a, annotator));
        self.annotators_generation.update(|g| *g += 1);
    }
}

/// A link clicked (Compose's handleLink): its listener told, or else a URL opened by the uri handler.
fn handle_link(link: &LinkAnnotation, uri_handler: &UriHandler) {
    match link {
        LinkAnnotation::Url { url, link_interaction_listener, .. } => match link_interaction_listener {
            Some(l) => l.on_click(link),
            None => uri_handler.open_uri(url),
        },
        LinkAnnotation::Clickable { link_interaction_listener, .. } => {
            if let Some(l) = link_interaction_listener {
                l.on_click(link);
            }
        }
    }
}

/// Whether link styles give no style for any state (Compose's isNullOrEmpty).
fn is_null_or_empty(styles: Option<&TextLinkStyles>) -> bool {
    styles.is_none_or(|s| {
        s.style.is_none() && s.focused_style.is_none() && s.hovered_style.is_none() && s.pressed_style.is_none()
    })
}

/// `other` over `style`, or whichever there is (Compose's mergeOrUse).
fn merge_or_use(style: Option<SpanStyle>, other: Option<&SpanStyle>) -> Option<SpanStyle> {
    match style {
        Some(s) => Some(s.merge(other)),
        None => other.cloned(),
    }
}

/// The offset of the character before `offset` in `text`.
fn previous_char_boundary(text: &str, offset: usize) -> usize {
    text[..offset].char_indices().next_back().map_or(0, |(i, _)| i)
}

/// The nodes of a text's links (Compose's TextLinkScope.LinksComposables): a node over each link that isn't empty
/// — clipped to it, marked for tests, measured and placed over it by the text's measure policy, hoverable, a hand
/// over it, clickable (opening it) — and, for a link with styles, its state followed and its style for that state
/// put on the text.
#[kompozable]
pub(crate) fn LinksComposables(scope: Rc<TextLinkScope>) {
    let uri_handler = LocalUriHandler.current();
    let links = {
        let text = scope.text.borrow();
        text.get_link_annotations(0, text.len())
    };
    for (index, range) in links.into_iter().enumerate() {
        if range.start == range.end {
            continue;
        }
        let scope = scope.clone();
        let uri_handler = uri_handler.clone();
        key(&index, move || LinkNode(scope, range, uri_handler));
    }
}

/// One link's node and its style (part of [`LinksComposables`]).
#[kompozable]
fn LinkNode(scope: Rc<TextLinkScope>, range: LinkRange, uri_handler: UriHandler) {
    let interaction_source = remember(MutableInteractionSource::new);
    let link = range.item.clone();
    let on_click = move || handle_link(&link, &uri_handler);
    Box(scope
        .clip_link(&range)
        .semantics(|c| c.set(SemanticsProperties::LinkTestMarker, ()))
        .then(scope.text_range(&range))
        .hoverable(interaction_source.clone())
        .pointer_hover_icon(PointerIcon::Hand)
        .combined_clickable_indication_with(
            Some(interaction_source.clone()),
            None,
            true,
            None,
            None,
            None,
            None,
            None,
            true,
            on_click,
        ));
    let styles = range.item.styles().cloned();
    if !is_null_or_empty(styles.as_ref()) {
        let styles = styles.unwrap_or_default();
        let observer =
            remember(|| Rc::new(LinkStateInteractionSourceObserver::new(interaction_source.as_interaction_source())));
        let collecting = observer.collect_interactions_for_links();
        LaunchedEffect((), collecting);
        let (hovered, focused, pressed) = (observer.is_hovered(), observer.is_focused(), observer.is_pressed());
        let link_range = range.clone();
        StyleAnnotation(
            scope,
            (hovered, focused, pressed, styles.clone()),
            Rc::new(move |annotator: &mut TextAnnotatorScope| {
                // The latest style for the link's state over its own (merged, not replacing what the text has).
                let merged =
                    merge_or_use(styles.style.clone(), focused.then_some(()).and(styles.focused_style.as_ref()));
                let merged = merge_or_use(merged, hovered.then_some(()).and(styles.hovered_style.as_ref()));
                let merged = merge_or_use(merged, pressed.then_some(()).and(styles.pressed_style.as_ref()));
                annotator.replace_style(&link_range, merged);
            }),
        );
    }
}

/// Puts `block`'s styles on the text while it is composed with these `keys` (Compose's StyleAnnotation).
#[kompozable]
fn StyleAnnotation(scope: Rc<TextLinkScope>, keys: (bool, bool, bool, TextLinkStyles), block: Rc<Annotator>) {
    let _ = &keys;
    DisposableEffect(keys, move || {
        scope.add_annotator(block.clone());
        on_dispose(move || scope.remove_annotator(&block))
    });
}

/// Restyles a text's links ([`TextLinkScope`]'s annotators; Compose's TextAnnotatorScope).
pub(crate) struct TextAnnotatorScope {
    styled_text: AnnotatedString,
}

impl TextAnnotatorScope {
    fn new(initial_text: AnnotatedString) -> TextAnnotatorScope {
        TextAnnotatorScope { styled_text: initial_text }
    }

    /// The span of `link`'s style (the one right after it, on its range) replaced by `new_style` (the default
    /// span style for none). Each replacement goes over the ones before it (Compose's starts each from the text
    /// the scope was made with, so with two links restyled at once, focus moving from one to the other, the first
    /// one's new style was lost).
    fn replace_style(&mut self, link: &LinkRange, new_style: Option<SpanStyle>) {
        let link_found = Cell::new(false);
        let new_style = new_style.unwrap_or(SpanStyle::Default);
        self.styled_text = self.styled_text.map_annotations(|a| {
            let replaced = if link_found.get()
                && matches!(a.item, Annotation::SpanStyle(_))
                && a.start == link.start
                && a.end == link.end
            {
                Range::tagged(Annotation::SpanStyle(new_style.clone()), a.start, a.end, a.tag.clone())
            } else {
                a.clone()
            };
            link_found.set(
                matches!(&a.item, Annotation::Link(l) if **l == link.item)
                    && a.start == link.start
                    && a.end == link.end
                    && a.tag == link.tag,
            );
            replaced
        });
    }
}

// ---- The links' nodes' layout ----------------------------------------------------------------------------------

/// The size of a link's node and where it goes (Compose's TextRangeLayoutMeasureResult).
pub(crate) struct TextRangeLayoutMeasureResult {
    pub(crate) width: i32,
    pub(crate) height: i32,
    pub(crate) place: Rc<dyn Fn() -> IntOffset>,
}

/// What a link's node is measured in (Compose's TextRangeLayoutMeasureScope).
pub(crate) struct TextRangeLayoutMeasureScope;

impl TextRangeLayoutMeasureScope {
    pub(crate) fn layout(
        &self,
        width: i32,
        height: i32,
        place: impl Fn() -> IntOffset + 'static,
    ) -> TextRangeLayoutMeasureResult {
        TextRangeLayoutMeasureResult { width, height, place: Rc::new(place) }
    }
}

/// How a link's node is sized and placed (Compose's TextRangeScopeMeasurePolicy).
pub(crate) type TextRangeScopeMeasurePolicy = Rc<dyn Fn(&TextRangeLayoutMeasureScope) -> TextRangeLayoutMeasureResult>;

/// The parent data a link's node gives the text's measure policy (Compose's TextRangeLayoutModifier).
#[derive(Clone)]
pub(crate) struct TextRangeLayoutModifier {
    pub(crate) measure_policy: TextRangeScopeMeasurePolicy,
}

/// The element of [`TextRangeLayoutModifier`]: equal only to itself.
struct TextRangeLayoutElement {
    measure_policy: TextRangeScopeMeasurePolicy,
}

impl PartialEq for TextRangeLayoutElement {
    fn eq(&self, o: &TextRangeLayoutElement) -> bool {
        Rc::ptr_eq(&self.measure_policy, &o.measure_policy)
    }
}

impl ModifierNodeElement for TextRangeLayoutElement {
    type Node = TextRangeLayoutNode;
    fn create(&self) -> TextRangeLayoutNode {
        TextRangeLayoutNode { measure_policy: self.measure_policy.clone() }
    }
    fn update(&self, node: &mut TextRangeLayoutNode) {
        node.measure_policy = self.measure_policy.clone();
    }
    fn name(&self) -> &'static str {
        "textRange"
    }
}

struct TextRangeLayoutNode {
    measure_policy: TextRangeScopeMeasurePolicy,
}

impl ModifierNode for TextRangeLayoutNode {
    node_capabilities!(parent_data);
}

impl ParentDataModifierNode for TextRangeLayoutNode {
    fn modify_parent_data(&mut self, _: &Density, _: Option<Box<dyn Any>>) -> Option<Box<dyn Any>> {
        Some(Box::new(TextRangeLayoutModifier { measure_policy: self.measure_policy.clone() }))
    }
}

/// The parent data of a link's node, if `measurable` is one.
pub(crate) fn text_range_of(measurable: &dyn Measurable) -> Option<TextRangeLayoutModifier> {
    measurable.parent_data()?.downcast_ref::<TextRangeLayoutModifier>().cloned()
}

// ---- A link's state ---------------------------------------------------------------------------------------------

const FOCUSED: i32 = 0b001;
const HOVERED: i32 = 0b010;
const PRESSED: i32 = 0b100;

/// Whether a link is focused, hovered or pressed, from its interactions (Compose's
/// LinkStateInteractionSourceObserver).
pub(crate) struct LinkStateInteractionSourceObserver {
    interaction_source: InteractionSource,
    interaction_state: MutableState<i32>,
}

impl LinkStateInteractionSourceObserver {
    fn new(interaction_source: InteractionSource) -> LinkStateInteractionSourceObserver {
        LinkStateInteractionSourceObserver { interaction_source, interaction_state: mutable_state_of(0) }
    }

    /// Follows the interactions (Compose's collectInteractionsForLinks): the ones going on kept, its state theirs.
    fn collect_interactions_for_links(&self) -> impl std::future::Future<Output = ()> + 'static {
        let state = self.interaction_state.clone();
        let mut going_on: Vec<Arc<dyn Interaction>> = Vec::new();
        self.interaction_source.interactions().collect(move |interaction: Arc<dyn Interaction>| {
            let i = &*interaction;
            if i.is::<HoverInteraction::Enter>()
                || i.is::<FocusInteraction::Focus>()
                || i.is::<PressInteraction::Press>()
            {
                going_on.push(interaction.clone());
            } else if let Some(exit) = i.downcast_ref::<HoverInteraction::Exit>() {
                going_on.retain(|g| g.downcast_ref::<HoverInteraction::Enter>() != Some(&exit.enter));
            } else if let Some(unfocus) = i.downcast_ref::<FocusInteraction::Unfocus>() {
                going_on.retain(|g| g.downcast_ref::<FocusInteraction::Focus>() != Some(&unfocus.focus));
            } else if let Some(release) = i.downcast_ref::<PressInteraction::Release>() {
                going_on.retain(|g| g.downcast_ref::<PressInteraction::Press>() != Some(&release.press));
            } else if let Some(cancel) = i.downcast_ref::<PressInteraction::Cancel>() {
                going_on.retain(|g| g.downcast_ref::<PressInteraction::Press>() != Some(&cancel.press));
            }
            let mut s = 0;
            for g in &going_on {
                if g.is::<HoverInteraction::Enter>() {
                    s |= HOVERED;
                } else if g.is::<FocusInteraction::Focus>() {
                    s |= FOCUSED;
                } else if g.is::<PressInteraction::Press>() {
                    s |= PRESSED;
                }
            }
            state.set(s);
        })
    }

    fn is_focused(&self) -> bool {
        self.interaction_state.get() & FOCUSED != 0
    }
    fn is_hovered(&self) -> bool {
        self.interaction_state.get() & HOVERED != 0
    }
    fn is_pressed(&self) -> bool {
        self.interaction_state.get() & PRESSED != 0
    }
}
