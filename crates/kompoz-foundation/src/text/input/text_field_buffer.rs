//! A text buffer a field's text is edited in (Compose's TextFieldBuffer): its text, selection and composition,
//! the changes made to it ([`ChangeList`]), its tracked styles; what [`TextFieldState::edit`](super::TextFieldState::
//! edit), an [`InputTransformation`](super::InputTransformation) and an [`OutputTransformation`](super::
//! OutputTransformation) are given.
//!
//! Offsets are UTF-8 byte offsets (Compose's are UTF-16 units). An offset off a character boundary is moved to one:
//! a replaced range covers whole characters (its start back, its end forward), a caret goes back, a selection
//! covers whole characters.

use super::change_tracker::ChangeTracker;
use super::internal::offset_mapping_calculator::OffsetMappingCalculator;
use super::internal::text_style_buffer::{StyleKind, TextStyleBuffer};
use super::text_field_char_sequence::{PlacedAnnotation, TextFieldCharSequence, TextHighlightType};
use super::text_field_text_styles::TextFieldTextStylesImpl;
use super::tracked_range::{ExpandPolicy, TrackedRange};
use kompoz_text::annotated_string::Annotation;
use kompoz_text::{ParagraphStyle, SpanStyle, TextRange};

/// `at` (clamped to `text`) moved back to a character boundary.
pub(crate) fn floor_char(text: &str, at: usize) -> usize {
    let mut at = at.min(text.len());
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}

/// `at` (clamped to `text`) moved forward to a character boundary.
pub(crate) fn ceil_char(text: &str, at: usize) -> usize {
    let mut at = at.min(text.len());
    while !text.is_char_boundary(at) {
        at += 1;
    }
    at
}

/// A range of `text` on its character boundaries: a caret back, a range out to whole characters (its direction
/// kept).
pub(crate) fn range_on_chars(text: &str, range: TextRange) -> TextRange {
    if range.collapsed() {
        let at = floor_char(text, range.start);
        return TextRange::at(at);
    }
    let (min, max) = (floor_char(text, range.min()), ceil_char(text, range.max()));
    if range.reversed() { TextRange::new(max, min) } else { TextRange::new(min, max) }
}

/// A style a [`TextFieldBuffer`] tracks: a span or a paragraph style (Compose's `addStyle` overloads).
pub trait TextFieldStyle: Clone + Sized + private::Sealed {
    #[doc(hidden)]
    fn of(annotation: &Annotation) -> Option<&Self>;
    #[doc(hidden)]
    fn into_annotation(self) -> Annotation;
}

mod private {
    pub trait Sealed {}
    impl Sealed for kompoz_text::SpanStyle {}
    impl Sealed for kompoz_text::ParagraphStyle {}
}

impl TextFieldStyle for SpanStyle {
    fn of(annotation: &Annotation) -> Option<&SpanStyle> {
        <SpanStyle as StyleKind>::of(annotation)
    }
    fn into_annotation(self) -> Annotation {
        Annotation::SpanStyle(self)
    }
}

impl TextFieldStyle for ParagraphStyle {
    fn of(annotation: &Annotation) -> Option<&ParagraphStyle> {
        <ParagraphStyle as StyleKind>::of(annotation)
    }
    fn into_annotation(self) -> Annotation {
        Annotation::ParagraphStyle(self)
    }
}

/// A text being edited, with its selection and the changes made to it (Compose's TextFieldBuffer, Appendable).
pub struct TextFieldBuffer {
    buffer: String,
    pub(crate) original_value: TextFieldCharSequence,
    offset_mapping_calculator: Option<OffsetMappingCalculator>,
    pub(crate) text_style_buffer: Option<TextStyleBuffer>,
    /// A transliteration's suggestion is being chosen (Compose's `suggestionSelected`).
    pub(crate) suggestion_selected: bool,
    change_tracker: ChangeTracker,
    selection: TextRange,
    composition: Option<TextRange>,
    composing_annotations: Option<Vec<PlacedAnnotation>>,
    highlight: Option<(TextHighlightType, TextRange)>,
}

impl TextFieldBuffer {
    /// One holding `initial_value` (Compose's internal constructor): its changes `initial_changes`' (none if not
    /// given), what it reverts to `original_value` (else the initial value), its edits recorded in
    /// `offset_mapping_calculator` (an output transformation's buffer).
    pub fn new(
        initial_value: TextFieldCharSequence,
        initial_changes: Option<&ChangeTracker>,
        original_value: Option<TextFieldCharSequence>,
        offset_mapping_calculator: Option<OffsetMappingCalculator>,
    ) -> TextFieldBuffer {
        TextFieldBuffer {
            buffer: initial_value.text().to_string(),
            text_style_buffer: initial_value
                .text_field_text_styles()
                .map(|s| TextStyleBuffer::copy_of(&s.text_style_buffer)),
            suggestion_selected: false,
            change_tracker: initial_changes.cloned().unwrap_or_default(),
            selection: initial_value.selection(),
            composition: initial_value.composition().filter(|c| !c.collapsed()),
            composing_annotations: initial_value.composing_annotations().filter(|a| !a.is_empty()).map(|a| a.to_vec()),
            highlight: None,
            offset_mapping_calculator,
            original_value: original_value.unwrap_or(initial_value),
        }
    }

    /// One holding `value`, its changes none, reverting to it.
    pub fn of(value: TextFieldCharSequence) -> TextFieldBuffer {
        TextFieldBuffer::new(value, None, None, None)
    }

    /// Its text's length (bytes).
    pub fn length(&self) -> usize {
        self.buffer.len()
    }

    /// The text before any change (what [`revert_all_changes`](TextFieldBuffer::revert_all_changes) brings back).
    pub fn original_text(&self) -> &str {
        self.original_value.text()
    }

    /// The selection before any change.
    pub fn original_selection(&self) -> TextRange {
        self.original_value.selection()
    }

    /// The changes made to it (Compose's `changes`): always the whole list, however many come after reading it.
    pub fn changes(&self) -> &ChangeTracker {
        &self.change_tracker
    }

    pub(crate) fn change_tracker_mut(&mut self) -> &mut ChangeTracker {
        &mut self.change_tracker
    }

    /// Whether something is selected (else the selection is the caret).
    pub fn has_selection(&self) -> bool {
        !self.selection.collapsed()
    }

    /// The selection (a caret when collapsed).
    pub fn selection(&self) -> TextRange {
        self.selection
    }

    /// Selects `value` (on whole characters); a collapsed one places the caret (Compose's `selection = …`). Panics
    /// if it is outside the text, as Compose's.
    pub fn set_selection(&mut self, value: TextRange) {
        let valid = TextRange::new(0, self.length());
        assert!(valid.contains(value), "Expected {value:?} to be in {valid:?}");
        self.selection = range_on_chars(&self.buffer, value);
        self.highlight = None;
    }

    // ---- composition

    /// What an input method composes, if anything.
    pub fn composition(&self) -> Option<TextRange> {
        self.composition
    }

    fn set_composition_range(&mut self, value: Option<TextRange>) {
        match value {
            Some(c) if !c.collapsed() => self.composition = Some(c),
            _ => {
                self.composition = None;
                if let Some(a) = &mut self.composing_annotations {
                    a.clear();
                }
            }
        }
    }

    /// The composition's annotations (an input method's styling of it).
    pub fn composing_annotations(&self) -> Option<&[PlacedAnnotation]> {
        self.composing_annotations.as_deref()
    }

    /// Whether something is being composed.
    pub fn has_composition(&self) -> bool {
        self.composition.is_some()
    }

    /// Finishes the composition (the text stays).
    pub fn commit_composition(&mut self) {
        self.set_composition_range(None);
    }

    /// Marks `start..end` as composing, with `annotations` (placed relative to its start). Panics if outside the
    /// text, reversed or empty, as Compose's.
    pub fn set_composition(&mut self, start: usize, end: usize, annotations: Option<&[PlacedAnnotation]>) {
        if start > self.buffer.len() {
            panic!("start ({start}) offset is outside of text region {}", self.buffer.len());
        }
        if end > self.buffer.len() {
            panic!("end ({end}) offset is outside of text region {}", self.buffer.len());
        }
        if start >= end {
            panic!("Do not set reversed or empty range: {start} > {end}");
        }
        let (start, end) = (floor_char(&self.buffer, start), ceil_char(&self.buffer, end));
        self.set_composition_range(Some(TextRange::new(start, end)));
        if let Some(a) = &mut self.composing_annotations {
            a.clear();
        }
        if let Some(annotations) = annotations.filter(|a| !a.is_empty()) {
            let placed = self.composing_annotations.get_or_insert_with(Vec::new);
            for a in annotations {
                let mut a = a.clone();
                a.start += start;
                a.end += start;
                placed.push(a);
            }
        }
    }

    // ---- highlight

    /// A highlighted range (a handwriting gesture's preview), if any.
    pub fn highlight(&self) -> Option<(TextHighlightType, TextRange)> {
        self.highlight
    }

    /// Highlights `start..end` (clamped to the text). Panics if reversed or empty.
    pub fn set_highlight(&mut self, highlight_type: TextHighlightType, start: usize, end: usize) {
        if start >= end {
            panic!("Do not set reversed or empty range: {start} > {end}");
        }
        let (start, end) = (floor_char(&self.buffer, start), ceil_char(&self.buffer, end));
        self.highlight = Some((highlight_type, TextRange::new(start, end)));
    }

    /// No range highlighted.
    pub fn clear_highlight(&mut self) {
        self.highlight = None;
    }

    // ---- editing

    /// `text` in place of `start..end` (whole characters), the change recorded; the selection moved with it, the
    /// composition finished (Compose's `replace`). Panics if `start` is past `end`.
    pub fn replace(&mut self, start: usize, end: usize, text: impl AsRef<str>) {
        let text = text.as_ref();
        self.replace_from(start, end, text, 0, text.len(), false);
    }

    /// `text[text_start..text_end]` in place of `start..end`; `is_from_hardware_source`: typed on a physical
    /// keyboard.
    pub(crate) fn replace_from(
        &mut self,
        start: usize,
        end: usize,
        text: &str,
        text_start: usize,
        text_end: usize,
        is_from_hardware_source: bool,
    ) {
        assert!(start <= end, "Expected start={start} <= end={end}");
        assert!(text_start <= text_end, "Expected textStart={text_start} <= textEnd={text_end}");
        let start = floor_char(&self.buffer, start);
        let end = ceil_char(&self.buffer, end);
        let text_start = floor_char(text, text_start);
        let text_end = ceil_char(text, text_end);
        self.on_text_will_change(start, end, text_end - text_start, is_from_hardware_source);
        self.buffer.replace_range(start..end, &text[text_start..text_end]);
        self.commit_composition();
        self.clear_highlight();
    }

    /// Replaces the text with `new_text` where it differs (one change, its common prefix and suffix kept).
    pub(crate) fn set_text_if_changed(&mut self, new_text: &str) {
        if let Some((this_start, this_end, new_start, new_end)) = find_common_prefix_and_suffix(&self.buffer, new_text)
        {
            self.replace_from(this_start, this_end, new_text, new_start, new_end, false);
        }
    }

    /// Appends `text` (Appendable's `append`).
    pub fn append(&mut self, text: impl AsRef<str>) -> &mut Self {
        let text = text.as_ref();
        let len = self.length();
        self.on_text_will_change(len, len, text.len(), false);
        self.buffer.push_str(text);
        self
    }

    /// Appends `text[start..end]` (whole characters).
    pub fn append_range(&mut self, text: &str, start: usize, end: usize) -> &mut Self {
        let (start, end) = (floor_char(text, start), ceil_char(text, end));
        self.append(&text[start..end])
    }

    /// Appends `c`.
    pub fn append_char(&mut self, c: char) -> &mut Self {
        let mut b = [0u8; 4];
        self.append(c.encode_utf8(&mut b))
    }

    fn on_text_will_change(
        &mut self,
        replace_start: usize,
        replace_end: usize,
        new_length: usize,
        is_from_hardware_source: bool,
    ) {
        self.change_tracker.track_change(replace_start, replace_end, new_length, is_from_hardware_source);
        if let Some(c) = &mut self.offset_mapping_calculator {
            c.record_edit_operation(replace_start, replace_end, new_length);
        }
        // (Edits from code or a desktop keyboard aren't followed by an input method's selection: the selection is
        // kept at a sensible place by every edit.)
        self.selection = adjust_text_range(self.selection, replace_start, replace_end, new_length);
        if let Some(s) = &mut self.text_style_buffer {
            s.replace_text(replace_start, replace_end, new_length);
        }
    }

    /// The character at byte `index` (at the start of the character holding it).
    pub fn char_at(&self, index: usize) -> char {
        let at = floor_char(&self.buffer, index);
        self.buffer[at..].chars().next().unwrap_or_else(|| panic!("index {index} out of bounds for {}", self.length()))
    }

    /// The text (Compose's `asCharSequence`).
    pub fn as_char_sequence(&self) -> &str {
        &self.buffer
    }

    /// Back to how it was made: its original text and selection, its changes forgotten, its styles the original
    /// ones (Compose's `revertAllChanges`).
    pub fn revert_all_changes(&mut self) {
        let original = self.original_value.text().to_string();
        let len = self.length();
        self.replace(0, len, &original);
        self.set_selection(self.original_value.selection());
        self.change_tracker.clear_changes();
        match self.original_value.text_field_text_styles() {
            Some(styles) => {
                let original = styles.text_style_buffer.clone();
                match &mut self.text_style_buffer {
                    Some(s) => s.sync_to(&original),
                    None => {
                        let mut s = TextStyleBuffer::new();
                        s.sync_to(&original);
                        self.text_style_buffer = Some(s);
                    }
                }
            }
            None => {
                if let Some(s) = &mut self.text_style_buffer {
                    s.clear();
                }
            }
        }
    }

    /// The caret before the character at `index` (Compose's `placeCursorBeforeCharAt`): 0 to `length`. Panics
    /// outside that.
    pub fn place_cursor_before_char_at(&mut self, index: usize) {
        assert!(index <= self.length(), "Expected {index} to be in [0, {})", self.length() + 1);
        self.selection = TextRange::at(floor_char(&self.buffer, index));
    }

    /// The caret after the character at `index` (Compose's `placeCursorAfterCharAt`): 0 to `length` - 1. Panics
    /// outside that.
    pub fn place_cursor_after_char_at(&mut self, index: usize) {
        assert!(index < self.length(), "Expected {index} to be in [0, {})", self.length());
        let at = floor_char(&self.buffer, index);
        let next = at + self.buffer[at..].chars().next().map_or(0, char::len_utf8);
        self.selection = TextRange::at(next.min(self.length()));
    }

    /// The caret at the end (Compose's `placeCursorAtEnd`).
    pub fn place_cursor_at_end(&mut self) {
        self.place_cursor_before_char_at(self.length());
    }

    /// Selects all the text (Compose's `selectAll`).
    pub fn select_all(&mut self) {
        self.set_selection(TextRange::new(0, self.length()));
    }

    /// Inserts `text` at `index` (Compose's `insert`).
    pub fn insert(&mut self, index: usize, text: impl AsRef<str>) {
        self.replace(index, index, text);
    }

    /// Deletes `start..end` (Compose's `delete`).
    pub fn delete(&mut self, start: usize, end: usize) {
        self.replace(start, end, "");
    }

    /// Selects `start..end`, each clamped to the text (Compose's `setSelectionCoerced`).
    pub(crate) fn set_selection_coerced(&mut self, start: usize, end: usize) {
        let len = self.length();
        self.set_selection(TextRange::new(start.min(len), end.min(len)));
    }

    /// An immutable copy of what it holds, with `selection` and `composition` (Compose's
    /// `toTextFieldCharSequence`).
    pub fn to_text_field_char_sequence_with(
        &self,
        selection: TextRange,
        composition: Option<TextRange>,
        composing_annotations: Option<Vec<PlacedAnnotation>>,
    ) -> TextFieldCharSequence {
        TextFieldCharSequence::with_all(
            self.buffer.as_str(),
            selection,
            composition,
            None,
            composing_annotations,
            None,
            self.get_text_field_text_styles(),
        )
    }

    /// An immutable copy of what it holds.
    pub fn to_text_field_char_sequence(&self) -> TextFieldCharSequence {
        self.to_text_field_char_sequence_with(
            self.selection,
            self.composition,
            self.composing_annotations.clone().filter(|a| !a.is_empty()),
        )
    }

    /// Its styles, held immutably (Compose's `getTextFieldTextStyles`).
    pub(crate) fn get_text_field_text_styles(&self) -> Option<TextFieldTextStylesImpl> {
        self.text_style_buffer.as_ref().map(|s| TextFieldTextStylesImpl::new(s.to_immutable(), self.length()))
    }

    /// The edits recorded for an output transformation's offset mapping.
    pub(crate) fn take_offset_mapping_calculator(&mut self) -> Option<OffsetMappingCalculator> {
        self.offset_mapping_calculator.take()
    }

    // ---- styles

    fn require_valid_style_range(&self, range: TextRange) {
        let valid = TextRange::new(0, self.length());
        assert!(valid.contains(range), "Expected {range:?} to be in {valid:?}");
        assert!(!range.reversed(), "Expected {range:?} to not be reversed.");
    }

    fn style_buffer(&mut self) -> &mut TextStyleBuffer {
        self.text_style_buffer.get_or_insert_with(TextStyleBuffer::new)
    }

    fn add_annotation_to_buffer<S: TextFieldStyle>(
        &mut self,
        style: S,
        range: TextRange,
        expand_policy: ExpandPolicy,
    ) -> TrackedRange<S> {
        let range = range_on_chars(&self.buffer, range);
        // (Treated as the text replaced by itself styled.)
        self.change_tracker.track_change(range.start, range.end, range.end - range.start, false);
        let buffer = self.style_buffer();
        let id = buffer.add_style(
            style.into_annotation(),
            range.start,
            range.end,
            expand_policy.start_expands(),
            expand_policy.end_expands(),
        );
        TrackedRange::new(buffer.id, id)
    }

    /// Adds `style` (a span or paragraph style) to `start..end`, its range following the edits as
    /// [`ExpandPolicy::AtEnd`]'s (Compose's `addStyle(spanStyle, start, end)`). Styles apply in the order added.
    /// Panics if the range is outside the text or reversed.
    pub fn add_style<S: TextFieldStyle>(&mut self, style: S, start: usize, end: usize) {
        let range = TextRange::new(start, end);
        self.require_valid_style_range(range);
        self.add_annotation_to_buffer(style, range, ExpandPolicy::AtEnd);
    }

    /// Adds `style` to `range`, following the edits as `expand_policy` says; its handle (Compose's
    /// `addStyle(spanStyle, range, expandPolicy)`).
    ///
    /// Kotlin: `addStyle(style: S, range: TextRange, expandPolicy: ExpandPolicy)`
    pub fn add_style_tracked<S: TextFieldStyle>(
        &mut self,
        style: S,
        range: TextRange,
        expand_policy: ExpandPolicy,
    ) -> TrackedRange<S> {
        self.require_valid_style_range(range);
        self.add_annotation_to_buffer(style, range, expand_policy)
    }

    fn get_styles<S: TextFieldStyle + StyleKind>(&self, range: TextRange) -> Vec<TrackedRange<S>> {
        let (start, end) = (range.min().min(self.length()), range.max().min(self.length()));
        match &self.text_style_buffer {
            Some(b) => b.get_styles::<S>(start, end).into_iter().map(|id| TrackedRange::new(b.id, id)).collect(),
            None => Vec::new(),
        }
    }

    /// The span styles meeting `range` (clamped to the text), in the order added (Compose's `getSpanStyles`).
    pub fn get_span_styles(&self, range: TextRange) -> Vec<TrackedRange<SpanStyle>> {
        self.get_styles::<SpanStyle>(range)
    }

    /// The paragraph styles meeting `range`, in the order added (Compose's `getParagraphStyles`).
    pub fn get_paragraph_styles(&self, range: TextRange) -> Vec<TrackedRange<ParagraphStyle>> {
        self.get_styles::<ParagraphStyle>(range)
    }

    /// Removes the style `tracked_range` is: whether it was there (Compose's `removeStyle`).
    pub fn remove_style<S>(&mut self, tracked_range: TrackedRange<S>) -> bool {
        match &mut self.text_style_buffer {
            Some(b) if b.id == tracked_range.creator_id => b.remove_style(tracked_range.entry_id),
            _ => false,
        }
    }

    /// Whether `tracked_range` is still a style of this buffer (Compose's `TrackedRange.isValid`).
    pub fn is_valid<S>(&self, tracked_range: TrackedRange<S>) -> bool {
        self.text_style_buffer
            .as_ref()
            .is_some_and(|b| b.id == tracked_range.creator_id && b.entry(tracked_range.entry_id).is_some())
    }

    fn active_entry<S>(
        &self,
        tracked_range: TrackedRange<S>,
    ) -> Option<&super::internal::text_style_buffer::StyleEntry> {
        let b = self.text_style_buffer.as_ref()?;
        assert!(b.id == tracked_range.creator_id, "TrackedRange belongs to a different TextFieldBuffer.");
        b.entry(tracked_range.entry_id)
    }

    /// Where the style is now (Compose's `TrackedRange.textRange`; [`TextRange::Zero`] if it is no longer valid).
    pub fn text_range<S>(&self, tracked_range: TrackedRange<S>) -> TextRange {
        if !self.is_valid(tracked_range) {
            return TextRange::Zero;
        }
        self.active_entry(tracked_range).map_or(TextRange::Zero, |e| TextRange::new(e.start, e.end))
    }

    /// Moves the style to `value` (Compose's `textRange = …`). Panics if it is outside the text, reversed or
    /// collapsed.
    pub fn set_text_range<S>(&mut self, tracked_range: TrackedRange<S>, value: TextRange) {
        self.require_valid_style_range(value);
        assert!(!value.collapsed(), "TrackedRange's textRange cannot be collapsed, but was {value:?}");
        if self.is_valid(tracked_range) {
            let value = range_on_chars(&self.buffer, value);
            self.style_buffer().set_range(tracked_range.entry_id, value.start, value.end);
        }
    }

    /// The style's span style (Compose's `TrackedRange<SpanStyle>.spanStyle`; a default one if no longer valid).
    pub fn span_style(&self, tracked_range: TrackedRange<SpanStyle>) -> SpanStyle {
        self.style_of(tracked_range).unwrap_or_default()
    }

    /// Sets the style's span style.
    pub fn set_span_style(&mut self, tracked_range: TrackedRange<SpanStyle>, value: SpanStyle) {
        self.set_style_of(tracked_range, value);
    }

    /// The style's paragraph style (Compose's `TrackedRange<ParagraphStyle>.paragraphStyle`).
    pub fn paragraph_style(&self, tracked_range: TrackedRange<ParagraphStyle>) -> ParagraphStyle {
        self.style_of(tracked_range).unwrap_or_default()
    }

    /// Sets the style's paragraph style.
    pub fn set_paragraph_style(&mut self, tracked_range: TrackedRange<ParagraphStyle>, value: ParagraphStyle) {
        self.set_style_of(tracked_range, value);
    }

    fn style_of<S: TextFieldStyle>(&self, tracked_range: TrackedRange<S>) -> Option<S> {
        if !self.is_valid(tracked_range) {
            return None;
        }
        self.active_entry(tracked_range).and_then(|e| S::of(&e.item).cloned())
    }

    fn set_style_of<S: TextFieldStyle>(&mut self, tracked_range: TrackedRange<S>, value: S) {
        if self.is_valid(tracked_range) {
            self.style_buffer().set_item(tracked_range.entry_id, value.into_annotation());
        }
    }

    /// How the style takes in text inserted at its ends (Compose's `TrackedRange.expandPolicy`; InsideOnly if no
    /// longer valid).
    pub fn expand_policy<S>(&self, tracked_range: TrackedRange<S>) -> ExpandPolicy {
        if !self.is_valid(tracked_range) {
            return ExpandPolicy::InsideOnly;
        }
        self.active_entry(tracked_range)
            .map_or(ExpandPolicy::InsideOnly, |e| ExpandPolicy::new(e.start_expands, e.end_expands))
    }

    /// Sets how the style takes in text inserted at its ends.
    pub fn set_expand_policy<S>(&mut self, tracked_range: TrackedRange<S>, value: ExpandPolicy) {
        if self.is_valid(tracked_range) {
            self.style_buffer().set_expand_policy(tracked_range.entry_id, value.start_expands(), value.end_expands());
        }
    }
}

impl std::fmt::Display for TextFieldBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(&self.buffer)
    }
}

impl std::fmt::Debug for TextFieldBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "TextFieldBuffer({:?}, selection={:?})", self.buffer, self.selection)
    }
}

/// `original_range` after `replace_start..replace_end` was replaced by `inserted_text_length` bytes (Compose's
/// `adjustTextRange`): before it, unmoved; around it, grown or shrunk at its end (a caret moved along); inside
/// it, moved to its end; after it, moved by the difference; overlapping one end, cut there.
pub fn adjust_text_range(
    original_range: TextRange,
    replace_start: usize,
    replace_end: usize,
    inserted_text_length: usize,
) -> TextRange {
    let mut sel_start = original_range.min() as isize;
    let mut sel_end = original_range.max() as isize;
    let (rs, re, n) = (replace_start as isize, replace_end as isize, inserted_text_length as isize);
    if sel_end < rs {
        return original_range;
    }
    if sel_start <= rs && re <= sel_end {
        let diff = n - (re - rs);
        if sel_start == sel_end {
            sel_start += diff;
        }
        sel_end += diff;
    } else if sel_start > rs && sel_end < re {
        sel_start = rs + n;
        sel_end = rs + n;
    } else if sel_start >= re {
        let diff = n - (re - rs);
        sel_start += diff;
        sel_end += diff;
    } else if rs < sel_start {
        sel_start = rs + n;
        sel_end += n - (re - rs);
    } else {
        sel_end = rs;
    }
    TextRange::new(sel_start.max(0) as usize, sel_end.max(0) as usize)
}

/// Where `a` and `b` differ: the start and end of the differing part of each (their common prefix and suffix
/// left out, whole characters), or none if they are the same (Compose's `findCommonPrefixAndSuffix`).
pub fn find_common_prefix_and_suffix(a: &str, b: &str) -> Option<(usize, usize, usize, usize)> {
    let (mut a_start, mut a_end, mut b_start, mut b_end) = (0, a.len(), 0, b.len());
    if !a.is_empty() && !b.is_empty() {
        let (mut prefix_found, mut suffix_found) = (false, false);
        loop {
            if !prefix_found {
                let (ca, cb) = (a[a_start..].chars().next(), b[b_start..].chars().next());
                match (ca, cb) {
                    (Some(ca), Some(cb)) if ca == cb => {
                        a_start += ca.len_utf8();
                        b_start += cb.len_utf8();
                    }
                    _ => prefix_found = true,
                }
            }
            if !suffix_found {
                let (ca, cb) = (a[..a_end].chars().next_back(), b[..b_end].chars().next_back());
                match (ca, cb) {
                    (Some(ca), Some(cb)) if ca == cb => {
                        a_end -= ca.len_utf8();
                        b_end -= cb.len_utf8();
                    }
                    _ => suffix_found = true,
                }
            }
            if !(a_start < a_end && b_start < b_end && !(prefix_found && suffix_found)) {
                break;
            }
        }
    }
    if a_start >= a_end && b_start >= b_end {
        return None;
    }
    Some((a_start, a_end, b_start, b_end))
}
