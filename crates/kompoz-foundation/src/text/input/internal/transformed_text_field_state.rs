//! A [`TextFieldState`] as its field shows and edits it (Compose's TransformedTextFieldState): the text an
//! [`OutputTransformation`] makes of it ([`output_text`](TransformedTextFieldState::output_text): what semantics and
//! tests read), then what a [`CodepointTransformation`] shows ([`visual_text`](TransformedTextFieldState::
//! visual_text): what is laid out and drawn), each with the selection and composition mapped onto it, kept in
//! derived states; the edits the field makes, in the shown text's terms, mapped back and made as the user (through
//! the field's [`InputTransformation`]); offsets mapped both ways.

use super::codepoint_transformation::{CodepointTransformation, same_codepoint_transformation, to_visual_text};
use super::offset_mapping_calculator::OffsetMappingCalculator;
use super::undo::TextFieldEditUndoBehavior;
use crate::text::input::{
    InputTransformation, NotifyImeListener, OutputTransformation, TextFieldBuffer, TextFieldCharSequence,
    TextFieldState, TextHighlightType,
};
use kompoz_runtime::{DerivedState, MutableState, derived_state_of, mutable_state_of};
use kompoz_text::TextRange;
use std::cell::RefCell;
use std::convert::Infallible;
use std::rc::Rc;

/// Which side of a wedge (an insertion or replacement of the transformed text, where offsets are ambiguous) a
/// caret keeps to (Compose's WedgeAffinity).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum WedgeAffinity {
    Start,
    End,
}

/// The wedge affinities of a selection's start and end (Compose's SelectionWedgeAffinity).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct SelectionWedgeAffinity {
    pub start_affinity: WedgeAffinity,
    pub end_affinity: WedgeAffinity,
}

impl SelectionWedgeAffinity {
    /// Both ends the same.
    pub fn new(affinity: WedgeAffinity) -> SelectionWedgeAffinity {
        SelectionWedgeAffinity { start_affinity: affinity, end_affinity: affinity }
    }
}

/// What a transformed offset is in the untransformed text (Compose's IndexTransformationType).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum IndexTransformationType {
    Untransformed,
    Insertion,
    Replacement,
    Deletion,
}

/// A transformed text and how its offsets map.
#[derive(Clone, PartialEq, Debug)]
struct TransformedText {
    text: TextFieldCharSequence,
    offset_mapping: OffsetMappingCalculator,
}

struct Inner {
    text_field_state: TextFieldState,
    input_transformation: RefCell<Option<InputTransformation>>,
    codepoint_transformation: Option<Rc<dyn CodepointTransformation>>,
    output_transformation: Option<Rc<dyn OutputTransformation>>,
    output_transformed_text: Option<DerivedState<Option<TransformedText>>>,
    codepoint_transformed_text: Option<DerivedState<Option<TransformedText>>>,
    selection_wedge_affinity: MutableState<SelectionWedgeAffinity>,
}

/// A field's state as it shows and edits it (Compose's TransformedTextFieldState): cheap to clone.
#[derive(Clone)]
pub struct TransformedTextFieldState(Rc<Inner>);

impl TransformedTextFieldState {
    /// `text_field_state` edited through `input_transformation`, shown through `output_transformation` then
    /// `codepoint_transformation`.
    pub fn new(
        text_field_state: TextFieldState,
        input_transformation: Option<InputTransformation>,
        codepoint_transformation: Option<Rc<dyn CodepointTransformation>>,
        output_transformation: Option<Rc<dyn OutputTransformation>>,
    ) -> TransformedTextFieldState {
        let wedge = mutable_state_of(SelectionWedgeAffinity::new(WedgeAffinity::Start));
        let output_transformed_text = output_transformation.clone().map(|transformation| {
            let (state, wedge) = (text_field_state.clone(), wedge.clone());
            derived_state_of(move || calculate_output_transformed_text(&state.value(), &*transformation, wedge.get()))
        });
        let codepoint_transformed_text = codepoint_transformation.clone().map(|transformation| {
            let (state, wedge, output) = (text_field_state.clone(), wedge.clone(), output_transformed_text.clone());
            derived_state_of(move || {
                let source = output.as_ref().and_then(|o| o.get()).map_or_else(|| state.value(), |t| t.text);
                calculate_codepoint_transformed_text(&source, &*transformation, wedge.get())
            })
        });
        TransformedTextFieldState(Rc::new(Inner {
            text_field_state,
            input_transformation: RefCell::new(input_transformation),
            codepoint_transformation,
            output_transformation,
            output_transformed_text,
            codepoint_transformed_text,
            selection_wedge_affinity: wedge,
        }))
    }

    /// Whether `other` is this very object (Compose's `===`).
    pub fn same_as(&self, other: &TransformedTextFieldState) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }

    /// The state it shows.
    pub fn text_field_state(&self) -> &TextFieldState {
        &self.0.text_field_state
    }

    /// The state's contents (Compose's `untransformedText`).
    pub fn untransformed_text(&self) -> TextFieldCharSequence {
        self.0.text_field_state.value()
    }

    /// The state's composition.
    pub fn untransformed_composition(&self) -> Option<TextRange> {
        self.0.text_field_state.composition()
    }

    /// The last change was the user's.
    pub fn user_commit(&self) -> bool {
        self.0.text_field_state.user_commit()
    }

    /// A transliteration's suggestion is being chosen.
    pub fn suggestion_selected(&self) -> bool {
        self.0.text_field_state.suggestion_selected()
    }

    fn output_transformed(&self) -> Option<TransformedText> {
        self.0.output_transformed_text.as_ref().and_then(|s| s.get())
    }

    fn codepoint_transformed(&self) -> Option<TransformedText> {
        self.0.codepoint_transformed_text.as_ref().and_then(|s| s.get())
    }

    /// The text after the output transformation (Compose's `outputText`): what semantics read.
    pub fn output_text(&self) -> TextFieldCharSequence {
        self.output_transformed().map_or_else(|| self.untransformed_text(), |t| t.text)
    }

    /// The text shown (Compose's `visualText`): after the code point transformation too.
    pub fn visual_text(&self) -> TextFieldCharSequence {
        self.codepoint_transformed().map_or_else(|| self.output_text(), |t| t.text)
    }

    /// Whether it transforms the text at all.
    pub fn is_transformed(&self) -> bool {
        self.0.output_transformation.is_some() || self.0.codepoint_transformation.is_some()
    }

    /// The selection's wedge affinity (Compose's `selectionWedgeAffinity`, a snapshot state).
    pub fn selection_wedge_affinity(&self) -> SelectionWedgeAffinity {
        self.0.selection_wedge_affinity.get()
    }

    pub fn set_selection_wedge_affinity(&self, value: SelectionWedgeAffinity) {
        self.0.selection_wedge_affinity.set(value);
    }

    /// Takes `input_transformation` as its input transformation from now on (Compose's `update`).
    pub fn update(&self, input_transformation: Option<InputTransformation>) {
        *self.0.input_transformation.borrow_mut() = input_transformation;
    }

    fn input_transformation(&self) -> Option<InputTransformation> {
        self.0.input_transformation.borrow().clone()
    }

    fn edit_as_user(
        &self,
        restart_ime_if_content_changes: bool,
        undo_behavior: TextFieldEditUndoBehavior,
        block: impl FnOnce(&mut TextFieldBuffer),
    ) {
        let transformation = self.input_transformation();
        self.0.text_field_state.edit_as_user(
            transformation.as_ref(),
            restart_ime_if_content_changes,
            undo_behavior,
            block,
        );
    }

    /// The caret before the shown text's character at `transformed_offset`.
    pub fn place_cursor_before_char_at(&self, transformed_offset: usize) {
        self.select_chars_in(TextRange::at(transformed_offset));
    }

    /// Selects `transformed_range` of the shown text.
    pub fn select_chars_in(&self, transformed_range: TextRange) {
        let untransformed = self.map_from_transformed_range(transformed_range);
        self.select_untransformed_chars_in(untransformed);
    }

    /// Selects `untransformed_range` of the state's text.
    pub fn select_untransformed_chars_in(&self, untransformed_range: TextRange) {
        self.edit_as_user(true, TextFieldEditUndoBehavior::MergeIfPossible, |b| {
            b.set_selection_coerced(untransformed_range.start, untransformed_range.end);
        });
    }

    /// Highlights `transformed_range` of the shown text as `highlight_type`.
    pub fn highlight_chars_in(&self, highlight_type: TextHighlightType, transformed_range: TextRange) {
        let untransformed = self.map_from_transformed_range(transformed_range);
        self.edit_as_user(true, TextFieldEditUndoBehavior::MergeIfPossible, |b| {
            b.set_highlight(highlight_type, untransformed.start, untransformed.end);
        });
    }

    /// Replaces all the text with `new_text` (what semantics' SetText does).
    pub fn replace_all(&self, new_text: &str) {
        let wedge = self.0.selection_wedge_affinity.clone();
        self.edit_as_user(true, TextFieldEditUndoBehavior::MergeIfPossible, |b| {
            let len = b.length();
            b.delete(0, len);
            b.append(new_text);
            update_wedge_affinity(b, &wedge);
        });
    }

    /// Selects all the text.
    pub fn select_all(&self) {
        self.edit_as_user(true, TextFieldEditUndoBehavior::MergeIfPossible, |b| {
            let len = b.length();
            b.set_selection_coerced(0, len);
        });
    }

    /// Deletes the selected text (a step of its own in the undo history).
    pub fn delete_selected_text(&self, is_from_hardware_source: bool) {
        let wedge = self.0.selection_wedge_affinity.clone();
        self.edit_as_user(true, TextFieldEditUndoBehavior::NeverMerge, |b| {
            let selection = b.selection();
            b.replace_from(selection.min(), selection.max(), "", 0, 0, is_from_hardware_source);
            b.set_selection_coerced(selection.min(), selection.min());
            update_wedge_affinity(b, &wedge);
        });
    }

    /// Replaces `range` of the shown text with `new_text`, the caret after it.
    pub fn replace_text(
        &self,
        new_text: &str,
        range: TextRange,
        undo_behavior: TextFieldEditUndoBehavior,
        restart_ime_if_content_changes: bool,
        is_from_hardware_source: bool,
    ) {
        let selection = self.map_from_transformed_range(range);
        let wedge = self.0.selection_wedge_affinity.clone();
        self.edit_as_user(restart_ime_if_content_changes, undo_behavior, |b| {
            b.replace_from(selection.min(), selection.max(), new_text, 0, new_text.len(), is_from_hardware_source);
            let cursor = selection.min() + new_text.len();
            b.set_selection_coerced(cursor, cursor);
            update_wedge_affinity(b, &wedge);
        });
    }

    /// Replaces the selection with `new_text`, the caret after it; the composition finished first if
    /// `clear_composition`.
    pub fn replace_selected_text(
        &self,
        new_text: &str,
        clear_composition: bool,
        undo_behavior: TextFieldEditUndoBehavior,
        restart_ime_if_content_changes: bool,
        is_from_hardware_source: bool,
    ) {
        let wedge = self.0.selection_wedge_affinity.clone();
        self.edit_as_user(restart_ime_if_content_changes, undo_behavior, |b| {
            if clear_composition {
                b.commit_composition();
            }
            let selection = b.selection();
            b.replace_from(selection.min(), selection.max(), new_text, 0, new_text.len(), is_from_hardware_source);
            let cursor = selection.min() + new_text.len();
            b.set_selection_coerced(cursor, cursor);
            update_wedge_affinity(b, &wedge);
        });
    }

    /// The selection collapsed to its larger end.
    pub fn collapse_selection_to_max(&self) {
        self.edit_as_user(true, TextFieldEditUndoBehavior::MergeIfPossible, |b| {
            let max = b.selection().max();
            b.set_selection_coerced(max, max);
        });
    }

    /// The selection collapsed to its end.
    pub fn collapse_selection_to_end(&self) {
        self.edit_as_user(true, TextFieldEditUndoBehavior::MergeIfPossible, |b| {
            let end = b.selection().end;
            b.set_selection_coerced(end, end);
        });
    }

    /// Undoes the state's last edit.
    pub fn undo(&self) {
        self.0.text_field_state.undo_state().undo();
    }

    /// Redoes the state's last undone edit.
    pub fn redo(&self) {
        self.0.text_field_state.undo_state().redo();
    }

    /// Edits the state's text as the user, in its own terms (Compose's `editUntransformedTextAsUser`): offsets the
    /// block uses must be the untransformed text's.
    pub fn edit_untransformed_text_as_user(
        &self,
        restart_ime_if_content_changes: bool,
        block: impl FnOnce(&mut TextFieldBuffer),
    ) {
        let wedge = self.0.selection_wedge_affinity.clone();
        self.edit_as_user(restart_ime_if_content_changes, TextFieldEditUndoBehavior::MergeIfPossible, |b| {
            block(b);
            update_wedge_affinity(b, &wedge);
        });
    }

    /// Where `offset` of the state's text is in the shown text (Compose's `mapToTransformed(Int)`).
    pub fn map_to_transformed(&self, offset: usize) -> TextRange {
        let present = self.output_transformed().map(|t| t.offset_mapping);
        let visual = self.codepoint_transformed().map(|t| t.offset_mapping);
        let intermediate = present.map_or(TextRange::at(offset), |m| m.map_from_source(offset));
        match visual {
            Some(v) => map_to_transformed(intermediate, &v, Some(self.selection_wedge_affinity())),
            None => intermediate,
        }
    }

    /// Where `range` of the state's text is in the shown text (Compose's `mapToTransformed(TextRange)`).
    pub fn map_to_transformed_range(&self, range: TextRange) -> TextRange {
        let present = self.output_transformed().map(|t| t.offset_mapping);
        let visual = self.codepoint_transformed().map(|t| t.offset_mapping);
        let intermediate = present.map_or(range, |m| map_to_transformed(range, &m, None));
        match visual {
            Some(v) => map_to_transformed(intermediate, &v, Some(self.selection_wedge_affinity())),
            None => intermediate,
        }
    }

    /// Where `offset` of the shown text is in the state's text (Compose's `mapFromTransformed(Int)`).
    pub fn map_from_transformed(&self, offset: usize) -> TextRange {
        let present = self.output_transformed().map(|t| t.offset_mapping);
        let visual = self.codepoint_transformed().map(|t| t.offset_mapping);
        let intermediate = visual.map_or(TextRange::at(offset), |m| m.map_from_dest(offset));
        match present {
            Some(p) => map_from_transformed(intermediate, &p),
            None => intermediate,
        }
    }

    /// Where `range` of the shown text is in the state's text (Compose's `mapFromTransformed(TextRange)`).
    pub fn map_from_transformed_range(&self, range: TextRange) -> TextRange {
        let present = self.output_transformed().map(|t| t.offset_mapping);
        let visual = self.codepoint_transformed().map(|t| t.offset_mapping);
        let intermediate = visual.map_or(range, |m| map_from_transformed(range, &m));
        match present {
            Some(p) => map_from_transformed(intermediate, &p),
            None => intermediate,
        }
    }

    /// What `transformed_query_index` is in the state's text, with where it maps there and back (Compose's
    /// `getIndexTransformationType`).
    pub fn get_index_transformation_type(
        &self,
        transformed_query_index: usize,
    ) -> (IndexTransformationType, TextRange, TextRange) {
        let untransformed = self.map_from_transformed(transformed_query_index);
        let retransformed = self.map_to_transformed_range(untransformed);
        let kind = match (untransformed.collapsed(), retransformed.collapsed()) {
            (true, true) => IndexTransformationType::Untransformed,
            (false, false) => IndexTransformationType::Replacement,
            (true, false) => IndexTransformationType::Insertion,
            (false, true) => IndexTransformationType::Deletion,
        };
        (kind, untransformed, retransformed)
    }

    /// Tells `listener` of each change of the state until cancelled (Compose's `collectImeNotifications`): with an
    /// output transformation, the value before as shown then and the shown text after.
    pub async fn collect_ime_notifications(&self, listener: NotifyImeListener) -> Infallible {
        let transformed: NotifyImeListener = match self.0.output_transformation.clone() {
            Some(transformation) => {
                let me = self.clone();
                Rc::new(move |old: &TextFieldCharSequence, _new: &TextFieldCharSequence, restart: bool| {
                    let wedge = kompoz_runtime::without_read_observation(|| me.selection_wedge_affinity());
                    let old = calculate_output_transformed_text(old, &*transformation, wedge)
                        .map_or_else(|| old.clone(), |t| t.text);
                    let new = kompoz_runtime::without_read_observation(|| me.visual_text());
                    listener(&old, &new, restart);
                })
            }
            None => listener,
        };
        struct Remove(TextFieldState, NotifyImeListener);
        impl Drop for Remove {
            fn drop(&mut self) {
                self.0.remove_notify_ime_listener(&self.1);
            }
        }
        self.0.text_field_state.add_notify_ime_listener(transformed.clone());
        let _remove = Remove(self.0.text_field_state.clone(), transformed);
        std::future::pending().await
    }
}

/// The same state, transformations (Compose's equality: not the input transformation).
impl PartialEq for TransformedTextFieldState {
    fn eq(&self, other: &Self) -> bool {
        if Rc::ptr_eq(&self.0, &other.0) {
            return true;
        }
        let codepoint = match (&self.0.codepoint_transformation, &other.0.codepoint_transformation) {
            (Some(a), Some(b)) => same_codepoint_transformation(&**a, &**b),
            (None, None) => true,
            _ => false,
        };
        let output = match (&self.0.output_transformation, &other.0.output_transformation) {
            (Some(a), Some(b)) => Rc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        self.0.text_field_state == other.0.text_field_state && codepoint && output
    }
}

impl std::fmt::Debug for TransformedTextFieldState {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let (output, visual) = kompoz_runtime::without_read_observation(|| (self.output_text(), self.visual_text()));
        write!(
            f,
            "TransformedTextFieldState(textFieldState={:?}, outputText=\"{}\", visualText=\"{}\")",
            self.0.text_field_state,
            output.text(),
            visual.text()
        )
    }
}

/// After an edit that left a caret, it keeps to the start of a wedge (Compose's `updateWedgeAffinity`).
fn update_wedge_affinity(buffer: &TextFieldBuffer, wedge: &MutableState<SelectionWedgeAffinity>) {
    use crate::text::input::ChangeList;
    if buffer.changes().change_count() > 0 && buffer.selection().collapsed() {
        wedge.set(SelectionWedgeAffinity::new(WedgeAffinity::Start));
    }
}

fn calculate_output_transformed_text(
    untransformed: &TextFieldCharSequence,
    transformation: &dyn OutputTransformation,
    wedge: SelectionWedgeAffinity,
) -> Option<TransformedText> {
    let mut buffer = TextFieldBuffer::new(untransformed.clone(), None, None, Some(OffsetMappingCalculator::new()));
    transformation.transform_output(&mut buffer);
    use crate::text::input::ChangeList;
    if buffer.changes().change_count() == 0 {
        return None;
    }
    let mapping = buffer.take_offset_mapping_calculator().unwrap_or_default();
    let selection = map_to_transformed(untransformed.selection(), &mapping, Some(wedge));
    let composition = untransformed.composition().map(|c| map_to_transformed(c, &mapping, Some(wedge)));
    let text = buffer.to_text_field_char_sequence_with(selection, composition, None);
    Some(TransformedText { text, offset_mapping: mapping })
}

fn calculate_codepoint_transformed_text(
    untransformed: &TextFieldCharSequence,
    transformation: &dyn CodepointTransformation,
    wedge: SelectionWedgeAffinity,
) -> Option<TransformedText> {
    let mut mapping = OffsetMappingCalculator::new();
    let text = to_visual_text(untransformed, transformation, &mut mapping)?;
    let selection = map_to_transformed(untransformed.selection(), &mapping, Some(wedge));
    let composition = untransformed.composition().map(|c| map_to_transformed(c, &mapping, Some(wedge)));
    Some(TransformedText { text: TextFieldCharSequence::new(text, selection, composition), offset_mapping: mapping })
}

fn map_to_transformed(
    range: TextRange,
    mapping: &OffsetMappingCalculator,
    selection_wedge_affinity: Option<SelectionWedgeAffinity>,
) -> TextRange {
    let mut transformed_start = mapping.map_from_source(range.start);
    let mut transformed_end = if range.collapsed() { transformed_start } else { mapping.map_from_source(range.end) };
    let start_affinity = selection_wedge_affinity.map(|w| w.start_affinity);
    let end_affinity =
        if range.collapsed() { start_affinity } else { selection_wedge_affinity.map(|w| w.end_affinity) };
    if let Some(a) = start_affinity
        && !transformed_start.collapsed()
    {
        transformed_start = match a {
            WedgeAffinity::Start => TextRange::at(transformed_start.start),
            WedgeAffinity::End => TextRange::at(transformed_start.end),
        };
    }
    if let Some(a) = end_affinity
        && !transformed_end.collapsed()
    {
        transformed_end = match a {
            WedgeAffinity::Start => TextRange::at(transformed_end.start),
            WedgeAffinity::End => TextRange::at(transformed_end.end),
        };
    }
    let min = transformed_start.min().min(transformed_end.min());
    let max = transformed_start.max().max(transformed_end.max());
    if range.reversed() { TextRange::new(max, min) } else { TextRange::new(min, max) }
}

fn map_from_transformed(range: TextRange, mapping: &OffsetMappingCalculator) -> TextRange {
    let start = mapping.map_from_dest(range.start);
    let end = if range.collapsed() { start } else { mapping.map_from_dest(range.end) };
    let min = start.min().min(end.min());
    let max = start.max().max(end.max());
    if range.reversed() { TextRange::new(max, min) } else { TextRange::new(min, max) }
}
