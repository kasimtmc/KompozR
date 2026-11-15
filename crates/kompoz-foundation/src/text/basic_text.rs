//! `BasicText`, as Compose's BasicText.kt: text that lays itself out, draws itself and says itself to
//! accessibility — a plain string through its fast node, an annotated string through its own, links' and inline
//! content's nodes over it, its text node a selectable one in a selection container.

use super::auto_size::TextAutoSize;
use super::inline_content::{InlineChildren, InlineContentMap, has_inline_content, resolve_inline_content};
use super::link_scope::{LinksComposables, TextLinkScope, TextRangeLayoutMeasureScope, text_range_of};
use super::modifiers::SelectionController;
use super::modifiers::selectable_text_annotated_string::SelectableTextAnnotatedStringElement;
use super::modifiers::text_annotated_string::{
    OnPlaceholderLayout, OnShowTranslation, OnTextLayout, TextAnnotatedStringElement, TextSubstitutionValue,
};
use super::modifiers::text_string_simple::TextStringSimpleElement;
use super::selection::{LocalSelectionRegistrar, LocalTextSelectionColors, SelectionRegistrarRef, has_selection};
use kompoz_graphics::{ColorProducer, Constraints, IntOffset, Rect};
use kompoz_runtime::{
    MutableState, SavedValue, Saver, kompozable, mutable_state_of, remember, remember_keyed,
    remember_saveable_keyed_with, remember_updated_state,
};
use kompoz_text::{AnnotatedString, FontFamilyResolver, Placeholder, Range, TextLayoutResult, TextOverflow, TextStyle};
use kompoz_ui::Modifier;
use kompoz_ui::layout::{Measurable, MeasurePolicy, MeasureResult, MeasureScope, Placeable, layout_into};
use kompoz_ui::platform::LocalFontFamilyResolver;
use std::rc::Rc;

/// What [`BasicText`] shows: a plain string (Compose's `BasicText(text: String, …)`, laid out by its fast node
/// unless a layout callback, an auto size or a selection asks for more) or an annotated one (Compose's
/// `BasicText(text: AnnotatedString, …)`).
#[derive(Clone, PartialEq, Debug)]
pub enum BasicTextValue {
    Plain(String),
    Annotated(AnnotatedString),
}

impl From<&str> for BasicTextValue {
    fn from(s: &str) -> BasicTextValue {
        BasicTextValue::Plain(s.to_string())
    }
}
impl From<String> for BasicTextValue {
    fn from(s: String) -> BasicTextValue {
        BasicTextValue::Plain(s)
    }
}
impl From<&String> for BasicTextValue {
    fn from(s: &String) -> BasicTextValue {
        BasicTextValue::Plain(s.clone())
    }
}
impl From<AnnotatedString> for BasicTextValue {
    fn from(s: AnnotatedString) -> BasicTextValue {
        BasicTextValue::Annotated(s)
    }
}
impl From<&AnnotatedString> for BasicTextValue {
    fn from(s: &AnnotatedString) -> BasicTextValue {
        BasicTextValue::Annotated(s.clone())
    }
}

/// Text that lays itself out, draws itself and says itself to accessibility (Compose's BasicText): `text` in
/// `style` (14 sp black where it gives no size or color), told to `on_text_layout` each time it is laid out
/// anew, cut as `overflow` says, wrapping where `soft_wrap` says, at most `max_lines` lines and as tall as
/// `min_lines` at least (1 ≤ min ≤ max, else it panics), its `inline_content` in the rooms its
/// `append_inline_content` marks (an annotated string's), in `color` where given (read as it draws: a change
/// redraws only), sized by `auto_size` to fit where given. In a selection container its text can be selected.
///
/// `on_text_layout` is called with the latest one given, not compared (as Compose's remembered lambdas): another
/// closure at each composition doesn't lay the text out again.
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn BasicText(
    text: impl Into<BasicTextValue>,
    #[default(Modifier)] modifier: Modifier,
    #[default(TextStyle::Default)] style: TextStyle,
    #[default(None)] on_text_layout: Option<OnTextLayout>,
    #[default(TextOverflow::Clip)] overflow: TextOverflow,
    #[default(true)] soft_wrap: bool,
    #[default(usize::MAX)] max_lines: usize,
    #[default(1)] min_lines: usize,
    #[default(InlineContentMap::new())] inline_content: InlineContentMap,
    #[default(None)] color: Option<ColorProducer>,
    #[default(None)] auto_size: Option<Rc<dyn TextAutoSize>>,
) {
    validate_min_max_lines(min_lines, max_lines);
    let selection_controller = remember_selection_controller();
    let font_family_resolver = LocalFontFamilyResolver.current();
    let on_text_layout = remember_on_text_layout(on_text_layout);
    let args = TextArgs {
        style,
        overflow,
        soft_wrap,
        max_lines,
        min_lines,
        font_family_resolver,
        selection_controller,
        color,
        auto_size,
    };
    match text {
        BasicTextValue::Plain(text) => {
            let final_modifier = if args.selection_controller.is_some()
                || on_text_layout.is_some()
                || args.auto_size.is_some()
            {
                text_modifier(modifier, AnnotatedString::new(text.as_str()), &args, on_text_layout, None, None, None)
            } else {
                modifier.then_element(TextStringSimpleElement {
                    text,
                    style: args.style,
                    font_family_resolver: args.font_family_resolver,
                    overflow: args.overflow,
                    soft_wrap: args.soft_wrap,
                    max_lines: args.max_lines,
                    min_lines: args.min_lines,
                    color: args.color,
                })
            };
            layout_into(final_modifier, EMPTY_MEASURE_POLICY.with(Rc::clone), false, || {});
        }
        BasicTextValue::Annotated(text) => {
            let has_inline_content = has_inline_content(&text);
            let has_links = text.has_link_annotations(0, text.len());
            if !has_inline_content && !has_links {
                // As a plain string's, every early exit taken.
                let m = text_modifier(modifier, text, &args, on_text_layout, None, None, None);
                layout_into(m, EMPTY_MEASURE_POLICY.with(Rc::clone), false, || {});
            } else {
                // What it shows: a substitution (a translation) shown is a plain text, its links gone.
                let displayed_text = remember_keyed(text.clone(), || mutable_state_of(text.clone()));
                let shown = displayed_text.clone();
                let on_show_translation: OnShowTranslation = remember_keyed(displayed_text.clone(), move || {
                    Rc::new(move |value: &TextSubstitutionValue| {
                        shown.set(if value.is_showing_substitution {
                            value.substitution.clone()
                        } else {
                            value.original.clone()
                        });
                    })
                });
                LayoutWithLinksAndInlineContent(
                    modifier,
                    displayed_text.get(),
                    on_text_layout,
                    has_inline_content,
                    inline_content,
                    args,
                    on_show_translation,
                );
            }
        }
    }
}

/// What BasicText passes on to its text node.
#[derive(Clone)]
struct TextArgs {
    style: TextStyle,
    overflow: TextOverflow,
    soft_wrap: bool,
    max_lines: usize,
    min_lines: usize,
    font_family_resolver: Rc<dyn FontFamilyResolver>,
    selection_controller: Option<Rc<SelectionController>>,
    color: Option<ColorProducer>,
    auto_size: Option<Rc<dyn TextAutoSize>>,
}

/// Panics unless 1 ≤ `min_lines` ≤ `max_lines` (Compose's validateMinMaxLines).
#[track_caller]
fn validate_min_max_lines(min_lines: usize, max_lines: usize) {
    assert!(
        min_lines > 0 && max_lines > 0,
        "kompoz: both minLines {min_lines} and maxLines {max_lines} must be greater than zero"
    );
    assert!(min_lines <= max_lines, "kompoz: minLines {min_lines} must be less than or equal to maxLines {max_lines}");
}

/// The text's selection controller in a selection container (Compose's BasicText: a selectable id from the
/// registrar, saved while the text has a selection, and the controller for it and the selection's background
/// color), none outside one.
#[kompozable]
fn remember_selection_controller() -> Option<Rc<SelectionController>> {
    let registrar = LocalSelectionRegistrar.current()?;
    let background_selection_color = LocalTextSelectionColors.current().background_color;
    let saver = selection_id_saver(&registrar);
    let r = registrar.clone();
    let selectable_id = remember_saveable_keyed_with(registrar.clone(), &saver, move || r.0.next_selectable_id());
    Some(remember_keyed((selectable_id, registrar.clone(), background_selection_color), move || {
        Rc::new(SelectionController::new(selectable_id, registrar.0.clone(), background_selection_color))
    }))
}

/// Saves a selectable id only while its text has a selection (Compose's selectionIdSaver).
fn selection_id_saver(registrar: &SelectionRegistrarRef) -> Saver<u64> {
    let registrar = registrar.clone();
    Saver::new(
        move |id: &u64| if has_selection(Some(&*registrar.0), *id) { SavedValue::from(*id) } else { SavedValue::Null },
        |saved: &SavedValue| saved.as_int().map(|id| id as u64),
    )
}

/// `on_text_layout` kept the same while it is given (its latest one called), so a new closure at each composition
/// doesn't lay the text out again.
#[kompozable]
fn remember_on_text_layout(on_text_layout: Option<OnTextLayout>) -> Option<OnTextLayout> {
    let given = on_text_layout.is_some();
    let latest = remember_updated_state(on_text_layout);
    remember_keyed(given, move || {
        given.then(|| {
            let f: OnTextLayout = Rc::new(move |r: &TextLayoutResult| {
                if let Some(f) = latest.get() {
                    f(r)
                }
            });
            f
        })
    })
}

/// The text node's modifier (Compose's Modifier.textModifier): an annotated string's text node, or in a
/// selection container the controller's modifier and a selectable text node.
fn text_modifier(
    modifier: Modifier,
    text: AnnotatedString,
    args: &TextArgs,
    on_text_layout: Option<OnTextLayout>,
    placeholders: Option<Vec<Range<Placeholder>>>,
    on_placeholder_layout: Option<OnPlaceholderLayout>,
    on_show_translation: Option<OnShowTranslation>,
) -> Modifier {
    let element = TextAnnotatedStringElement {
        text,
        style: args.style.clone(),
        font_family_resolver: args.font_family_resolver.clone(),
        on_text_layout,
        overflow: args.overflow,
        soft_wrap: args.soft_wrap,
        max_lines: args.max_lines,
        min_lines: args.min_lines,
        placeholders,
        on_placeholder_layout,
        selection_controller: args.selection_controller.clone(),
        color: args.color.clone(),
        auto_size: args.auto_size.clone(),
        on_show_translation,
    };
    match &args.selection_controller {
        None => modifier.then_element(element),
        Some(controller) => modifier.then(controller.modifier()).then_element(SelectableTextAnnotatedStringElement {
            text: TextAnnotatedStringElement { on_show_translation: None, ..element },
        }),
    }
}

/// A text with links or inline content (Compose's LayoutWithLinksAndInlineContent): the links' styles applied, the
/// inline content's placeholders resolved, the text node told where they are, the links' and inline content's
/// nodes composed as its children and placed by its measure policy.
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn LayoutWithLinksAndInlineContent(
    modifier: Modifier,
    text: AnnotatedString,
    on_text_layout: Option<OnTextLayout>,
    has_inline_content: bool,
    inline_content: InlineContentMap,
    args: TextArgs,
    on_show_translation: OnShowTranslation,
) {
    let text_scope: Option<Rc<TextLinkScope>> = if text.has_link_annotations(0, text.len()) {
        Some(remember_keyed(text.clone(), || Rc::new(TextLinkScope::new(text.clone()))))
    } else {
        None
    };
    // Only adds the links' styles: the text stays what it was.
    let styled_text = match &text_scope {
        Some(s) => s.apply_annotators(),
        None => text.clone(),
    };
    let (placeholders, inline_composables) = if has_inline_content {
        let (p, c) = resolve_inline_content(&text, &inline_content);
        (Some(p), Some(c))
    } else {
        (None, None)
    };
    let measured_placeholder_positions: Option<MutableState<Option<Vec<Option<Rect>>>>> =
        if has_inline_content { Some(remember(|| mutable_state_of(None))) } else { None };
    let on_placeholder_layout: Option<OnPlaceholderLayout> = measured_placeholder_positions.as_ref().map(|positions| {
        let positions = positions.clone();
        remember_keyed(positions.clone(), move || {
            let f: OnPlaceholderLayout = Rc::new(move |rects: &[Option<Rect>]| positions.set(Some(rects.to_vec())));
            f
        })
    });
    // The scope told of each layout first, then the text's own callback.
    let latest = remember_updated_state(on_text_layout);
    let scope_identity = text_scope.as_ref().map(|s| Rc::as_ptr(s) as usize);
    let scope = text_scope.clone();
    let on_layout: OnTextLayout = remember_keyed(scope_identity, move || {
        Rc::new(move |r: &TextLayoutResult| {
            if let Some(s) = &scope {
                s.set_text_layout_result(r);
            }
            if let Some(f) = latest.get() {
                f(r)
            }
        })
    });
    let policy_scope = text_scope.clone();
    let policy: Rc<dyn MeasurePolicy> =
        remember_keyed((scope_identity, measured_placeholder_positions.clone()), move || {
            match &measured_placeholder_positions {
                None => Rc::new(LinksTextMeasurePolicy { scope: policy_scope.clone() }) as Rc<dyn MeasurePolicy>,
                Some(positions) => {
                    Rc::new(TextMeasurePolicy { scope: policy_scope.clone(), placements: positions.clone() })
                }
            }
        });
    let m = text_modifier(
        modifier,
        styled_text,
        &args,
        Some(on_layout),
        placeholders,
        on_placeholder_layout,
        Some(on_show_translation),
    );
    layout_into(m, policy, false, move || {
        if let Some(s) = &text_scope {
            LinksComposables(s.clone());
        }
        if let Some(inline) = inline_composables {
            InlineChildren(text.clone(), inline);
        }
    });
}

thread_local! {
    /// Compose's EmptyMeasurePolicy, shared.
    static EMPTY_MEASURE_POLICY: Rc<dyn MeasurePolicy> = Rc::new(EmptyMeasurePolicy);
}

/// A text's layout policy without children (Compose's BasicText's EmptyMeasurePolicy): all it may be (its text
/// node gives it its size).
struct EmptyMeasurePolicy;

impl MeasurePolicy for EmptyMeasurePolicy {
    fn measure(&self, _: &MeasureScope, _: &[&dyn Measurable], constraints: Constraints) -> MeasureResult {
        MeasureResult::new(constraints.max_width, constraints.max_height, Vec::new(), |_| {})
    }
}

/// A text's layout policy with inline content and links (Compose's TextMeasurePolicy): each inline content
/// measured to its room and placed there (one cut off isn't), each link's node measured and placed over its link.
struct TextMeasurePolicy {
    scope: Option<Rc<TextLinkScope>>,
    placements: MutableState<Option<Vec<Option<Rect>>>>,
}

impl MeasurePolicy for TextMeasurePolicy {
    fn measure(&self, _: &MeasureScope, measurables: &[&dyn Measurable], constraints: Constraints) -> MeasureResult {
        let inline: Vec<&dyn Measurable> =
            measurables.iter().copied().filter(|m| text_range_of(*m).is_none()).collect();
        let inline_to_place: Vec<(Placeable, IntOffset)> = self
            .placements
            .get()
            .unwrap_or_default()
            .iter()
            .enumerate()
            .filter_map(|(index, rect)| {
                // A room cut off (ellipsized, past the lines): its content isn't measured or placed.
                let rect = (*rect)?;
                let measurable = inline.get(index)?;
                let placeable = measurable.measure(Constraints::new(
                    0,
                    rect.width().floor() as i32,
                    0,
                    rect.height().floor() as i32,
                ));
                Some((placeable, IntOffset::new(round(rect.left), round(rect.top))))
            })
            .collect();
        let links: Vec<&dyn Measurable> = measurables.iter().copied().filter(|m| text_range_of(*m).is_some()).collect();
        let links_to_place = measure_with_text_range_measure_constraints(&links, self.scope.as_deref());
        MeasureResult::new(constraints.max_width, constraints.max_height, Vec::new(), move |p| {
            for (placeable, position) in &inline_to_place {
                p.place_at(placeable, *position, 0.0);
            }
            for (placeable, place) in &links_to_place {
                p.place_at(placeable, place.as_ref().map_or(IntOffset::Zero, |f| f()), 0.0);
            }
        })
    }
}

/// A text's layout policy with links only (Compose's LinksTextMeasurePolicy): each link's node measured and
/// placed over its link. (Compose measures them as it places them; here as it measures, which reads the same
/// layout: the text node lays the text out before its children are measured.)
struct LinksTextMeasurePolicy {
    scope: Option<Rc<TextLinkScope>>,
}

impl MeasurePolicy for LinksTextMeasurePolicy {
    fn measure(&self, _: &MeasureScope, measurables: &[&dyn Measurable], constraints: Constraints) -> MeasureResult {
        let links_to_place = measure_with_text_range_measure_constraints(measurables, self.scope.as_deref());
        MeasureResult::new(constraints.max_width, constraints.max_height, Vec::new(), move |p| {
            for (placeable, place) in &links_to_place {
                p.place_at(placeable, place.as_ref().map_or(IntOffset::Zero, |f| f()), 0.0);
            }
        })
    }
}

type PlaceLink = Rc<dyn Fn() -> IntOffset>;

/// The links' nodes measured to their links' bounds, with where each goes (Compose's
/// measureWithTextRangeMeasureConstraints): none while the links aren't measured.
fn measure_with_text_range_measure_constraints(
    measurables: &[&dyn Measurable],
    scope: Option<&TextLinkScope>,
) -> Vec<(Placeable, Option<PlaceLink>)> {
    if !scope.is_some_and(TextLinkScope::should_measure_links) {
        return Vec::new();
    }
    let measure_scope = TextRangeLayoutMeasureScope;
    measurables
        .iter()
        .filter_map(|m| {
            let range = text_range_of(*m)?;
            let result = (range.measure_policy)(&measure_scope);
            let placeable = m.measure(Constraints::fit_prioritizing_width(
                result.width,
                result.width,
                result.height,
                result.height,
            ));
            Some((placeable, Some(result.place)))
        })
        .collect()
}

/// `v` to the nearest whole pixel, a half up (Compose's fastRoundToInt).
fn round(v: f32) -> i32 {
    (v + 0.5).floor() as i32
}
