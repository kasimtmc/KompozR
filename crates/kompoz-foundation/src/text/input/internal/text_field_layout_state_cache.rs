//! A state-based field's text layout, made lazily and kept (Compose's TextFieldLayoutStateCache): a derived state
//! of everything text layout depends on — the shown text, the field's style and single-lineness (the inputs that
//! don't come from the measure pass) and the density, direction, resolver of fonts and constraints (those that
//! do) — laid out again only when one of them changes in a way that matters to it.

use crate::text::input::internal::TransformedTextFieldState;
use crate::text::input::{PlacedAnnotation, TextFieldCharSequence};
use crate::text::keyboard_options::KeyboardOptions;
use kompoz_graphics::{Constraints, Density, LayoutDirection};
use kompoz_runtime::{MutableState, SnapshotMutationPolicy, mutable_confined_state_of_with_policy};
use kompoz_text::{AnnotatedString, FontFamilyResolver, TextDirection, TextLayoutResult, TextMeasurer, TextStyle};
use kompoz_ui::platform::KeyboardType;
use std::cell::RefCell;
use std::rc::Rc;

/// The inputs that don't come from the measure pass (Compose's NonMeasureInputs).
#[derive(Clone)]
struct NonMeasureInputs {
    text_field_state: TransformedTextFieldState,
    text_style: TextStyle,
    single_line: bool,
    soft_wrap: bool,
    is_keyboard_type_phone: bool,
}

impl NonMeasureInputs {
    /// Compose's mutation policy: the same state (its text is read where the layout is made), an equal style,
    /// the same flags.
    fn equivalent(&self, other: &NonMeasureInputs) -> bool {
        self.text_field_state.same_as(&other.text_field_state)
            && self.text_style == other.text_style
            && self.single_line == other.single_line
            && self.soft_wrap == other.soft_wrap
            && self.is_keyboard_type_phone == other.is_keyboard_type_phone
    }
}

/// The inputs that come from the measure pass (Compose's MeasureInputs).
#[derive(Clone)]
struct MeasureInputs {
    density: Density,
    layout_direction: LayoutDirection,
    font_family_resolver: Rc<dyn FontFamilyResolver>,
    constraints: Constraints,
}

impl MeasureInputs {
    /// Compose's mutation policy: the density's values (not its object), the direction, the same resolver, equal
    /// constraints.
    fn equivalent(&self, other: &MeasureInputs) -> bool {
        self.density.density == other.density.density
            && self.density.font_scale == other.density.font_scale
            && self.layout_direction == other.layout_direction
            && same_resolver(&self.font_family_resolver, &other.font_family_resolver)
            && self.constraints == other.constraints
    }
}

fn same_resolver(a: &Rc<dyn FontFamilyResolver>, b: &Rc<dyn FontFamilyResolver>) -> bool {
    std::ptr::eq(Rc::as_ptr(a) as *const (), Rc::as_ptr(b) as *const ())
}

/// Compose's `NonMeasureInputs.mutationPolicy`: equivalent inputs, or none on both sides.
fn non_measure_inputs_policy(a: &Option<NonMeasureInputs>, b: &Option<NonMeasureInputs>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a.equivalent(b),
        (a, b) => a.is_none() == b.is_none(),
    }
}

/// Compose's `MeasureInputs.mutationPolicy`: equivalent inputs, or none on both sides.
fn measure_inputs_policy(a: &Option<MeasureInputs>, b: &Option<MeasureInputs>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a.equivalent(b),
        (a, b) => a.is_none() == b.is_none(),
    }
}

/// What the last layout was made from (Compose's CacheRecord).
struct CacheRecord {
    visual_text: Option<TextFieldCharSequence>,
    annotations: Option<Vec<PlacedAnnotation>>,
    composition: Option<kompoz_text::TextRange>,
    text_style: Option<TextStyle>,
    single_line: bool,
    soft_wrap: bool,
    density_value: f32,
    font_scale: f32,
    layout_direction: Option<LayoutDirection>,
    font_family_resolver: Option<Rc<dyn FontFamilyResolver>>,
    constraints: Constraints,
    layout_result: Option<TextLayoutResult>,
}

impl Default for CacheRecord {
    fn default() -> Self {
        CacheRecord {
            visual_text: None,
            annotations: None,
            composition: None,
            text_style: None,
            single_line: false,
            soft_wrap: false,
            density_value: f32::NAN,
            font_scale: f32::NAN,
            layout_direction: None,
            font_family_resolver: None,
            constraints: Constraints::default(),
            layout_result: None,
        }
    }
}

/// Lays a field's text out on demand and keeps it (Compose's TextFieldLayoutStateCache): [`value`](Self::value)
/// reads the inputs as states (the shown text, the two groups of inputs) and gives the kept layout while they are
/// what it was made from, a new one otherwise. A style differing only in what paints reuses the layout with the
/// new style.
pub struct TextFieldLayoutStateCache {
    /// Compose's `nonMeasureInputs by mutableStateOf(null, NonMeasureInputs.mutationPolicy)` (confined: the inputs
    /// hold handles that aren't `Send`).
    non_measure_inputs: MutableState<Option<NonMeasureInputs>>,
    /// Compose's `measureInputs by mutableStateOf(null, MeasureInputs.mutationPolicy)` (confined).
    measure_inputs: MutableState<Option<MeasureInputs>>,
    record: RefCell<CacheRecord>,
    text_measurer: RefCell<Option<TextMeasurer>>,
}

impl Default for TextFieldLayoutStateCache {
    fn default() -> Self {
        TextFieldLayoutStateCache::new()
    }
}

impl TextFieldLayoutStateCache {
    pub fn new() -> TextFieldLayoutStateCache {
        TextFieldLayoutStateCache {
            non_measure_inputs: mutable_confined_state_of_with_policy(
                None,
                SnapshotMutationPolicy::new(non_measure_inputs_policy),
            ),
            measure_inputs: mutable_confined_state_of_with_policy(
                None,
                SnapshotMutationPolicy::new(measure_inputs_policy),
            ),
            record: RefCell::new(CacheRecord::default()),
            text_measurer: RefCell::new(None),
        }
    }

    /// The layout for the field's state and inputs now, or none before both groups of inputs were given
    /// (Compose's `value`): read as a state.
    pub fn value(&self) -> Option<TextLayoutResult> {
        let non_measure = self.non_measure_inputs.get()?;
        let measure = self.measure_inputs.get()?;
        Some(self.get_or_compute_layout(&non_measure, &measure))
    }

    /// The inputs that don't come from the measure pass (Compose's `updateNonMeasureInputs`): the readers of
    /// [`value`](Self::value) are told when they changed.
    pub fn update_non_measure_inputs(
        &self,
        text_field_state: &TransformedTextFieldState,
        text_style: &TextStyle,
        single_line: bool,
        soft_wrap: bool,
        keyboard_options: &KeyboardOptions,
    ) {
        self.non_measure_inputs.set(Some(NonMeasureInputs {
            text_field_state: text_field_state.clone(),
            text_style: text_style.clone(),
            single_line,
            soft_wrap,
            is_keyboard_type_phone: keyboard_options.keyboard_type == KeyboardType::Phone,
        }));
    }

    /// The inputs from the measure pass, and the layout they make (Compose's `layoutWithNewMeasureInputs`).
    /// Panics before [`update_non_measure_inputs`](Self::update_non_measure_inputs).
    pub fn layout_with_new_measure_inputs(
        &self,
        density: Density,
        layout_direction: LayoutDirection,
        font_family_resolver: Rc<dyn FontFamilyResolver>,
        constraints: Constraints,
    ) -> TextLayoutResult {
        let measure = MeasureInputs { density, layout_direction, font_family_resolver, constraints };
        self.measure_inputs.set(Some(measure.clone()));
        let non_measure =
            self.non_measure_inputs.peek().expect("Called layoutWithNewMeasureInputs before updateNonMeasureInputs");
        self.get_or_compute_layout(&non_measure, &measure)
    }

    fn get_or_compute_layout(&self, non_measure: &NonMeasureInputs, measure: &MeasureInputs) -> TextLayoutResult {
        let visual_text = non_measure.text_field_state.visual_text();
        // (ComposeFoundationFlags.isBasicTextFieldStyledTextEnabled, on: the tracked styles)
        let annotations = merge_nullable_lists(
            visual_text.composing_annotations().map(<[PlacedAnnotation]>::to_vec),
            visual_text.text_field_text_styles().map(|s| s.text_style_buffer().get_all_styles()),
        );
        {
            let cached = self.record.borrow();
            if let Some(result) = &cached.layout_result
                && cached.visual_text.as_ref().is_some_and(|t| t.content_equals(visual_text.text()))
                && cached.annotations == annotations
                && cached.composition == visual_text.composition()
                && cached.single_line == non_measure.single_line
                && cached.soft_wrap == non_measure.soft_wrap
                && cached.layout_direction == Some(measure.layout_direction)
                && cached.density_value == measure.density.density
                && cached.font_scale == measure.density.font_scale
                && cached.constraints == measure.constraints
                && cached.font_family_resolver.as_ref().is_some_and(|r| same_resolver(r, &measure.font_family_resolver))
            {
                let style = cached.text_style.as_ref();
                let layout_same =
                    style.is_some_and(|s| s.has_same_layout_affecting_attributes(&non_measure.text_style));
                let draw_same = style.is_some_and(|s| s.has_same_draw_affecting_attributes(&non_measure.text_style));
                if layout_same && draw_same {
                    return result.clone();
                }
                if layout_same {
                    // The layout stays; its input takes the new style (what draws it).
                    let mut input = (*result.layout_input).clone();
                    input.style = non_measure.text_style.clone();
                    return result.copy(Rc::new(input), result.size);
                }
            }
        }
        let result = self.compute_layout(&visual_text, annotations.clone(), non_measure, measure);
        let mut record = self.record.borrow_mut();
        *record = CacheRecord {
            visual_text: Some(visual_text.clone()),
            annotations,
            composition: visual_text.composition(),
            text_style: Some(non_measure.text_style.clone()),
            single_line: non_measure.single_line,
            soft_wrap: non_measure.soft_wrap,
            density_value: measure.density.density,
            font_scale: measure.density.font_scale,
            layout_direction: Some(measure.layout_direction),
            font_family_resolver: Some(measure.font_family_resolver.clone()),
            constraints: measure.constraints,
            layout_result: Some(result.clone()),
        };
        result
    }

    fn compute_layout(
        &self,
        visual_text: &TextFieldCharSequence,
        annotations: Option<Vec<PlacedAnnotation>>,
        non_measure: &NonMeasureInputs,
        measure: &MeasureInputs,
    ) -> TextLayoutResult {
        let mut measurer = self.text_measurer.borrow_mut();
        let measurer = measurer.get_or_insert_with(|| {
            TextMeasurer::new(measure.font_family_resolver.clone(), measure.density, measure.layout_direction, 1)
        });
        let style = if non_measure.is_keyboard_type_phone {
            // A phone number's digits go the way the locale's number system does (desktop: left to right,
            // Compose's skiko `resolveTextDirectionForKeyboardTypePhone`).
            non_measure.text_style.merge(&TextStyle { text_direction: TextDirection::Ltr, ..TextStyle::Default })
        } else {
            non_measure.text_style.clone()
        };
        let text = AnnotatedString::from_annotations(visual_text.text_arc(), annotations.unwrap_or_default());
        measurer.measure_with(
            text,
            style,
            kompoz_text::TextOverflow::Clip,
            non_measure.soft_wrap,
            if non_measure.single_line { 1 } else { kompoz_text::DefaultMaxLines },
            Vec::new(),
            measure.constraints,
            measure.layout_direction,
            measure.density,
            measure.font_family_resolver.clone(),
            false,
        )
    }
}

/// Two optional lists as one, none when both are empty (Compose's `mergeNullableLists`).
fn merge_nullable_lists<T>(first: Option<Vec<T>>, second: Option<Vec<T>>) -> Option<Vec<T>> {
    let first = first.filter(|f| !f.is_empty());
    let second = second.filter(|s| !s.is_empty());
    match (first, second) {
        (None, None) => None,
        (None, s) => s,
        (f, None) => f,
        (Some(mut f), Some(s)) => {
            f.extend(s);
            Some(f)
        }
    }
}
