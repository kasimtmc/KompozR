//! What sizes a text field shared by both engines (Compose's TextFieldDelegate.kt's default text,
//! HeightInLinesModifier.kt and TextFieldSize.kt with the flags of 1.12 on): a field is at least as wide as ten
//! "H" and one line tall (`text_field_min_size`), and as tall as its min and max lines say
//! (`height_in_lines`); a state-based single-line field is as tall as its laid out line
//! (`height_for_single_line_field`).

use super::input::internal::TextLayoutState;
use kompoz_graphics::{Constraints, Density, Dp, IntSize, LayoutDirection};
use kompoz_runtime::State;
use kompoz_text::{
    FontFamilyResolver, FontStyle, FontSynthesis, FontWeight, Paragraph, ParagraphIntrinsics, TextOverflow, TextStyle,
    Typeface,
};
use kompoz_ui::layout::{LayoutModifierNode, Measurable};
use kompoz_ui::modifier::ObserverModifierNode;
use kompoz_ui::platform::LocalFontFamilyResolver;
use kompoz_ui::{
    MeasureResult, MeasureScope, Modifier, ModifierNode, ModifierNodeElement, NodeCx, NodeHandle, node_capabilities,
};
use std::rc::Rc;

/// The reference text of a field's least size (Compose's EmptyTextReplacement: DefaultWidthCharCount, ten, "H").
pub(crate) const EMPTY_TEXT_REPLACEMENT: &str = "HHHHHHHHHH";

/// `lines` lines of the reference text laid out in `style` (Compose's `paragraphForDefaultText`).
pub(crate) fn paragraph_for_default_text(
    style: &TextStyle,
    density: Density,
    font_family_resolver: &Rc<dyn FontFamilyResolver>,
    lines: usize,
    soft_wrap: bool,
) -> Paragraph {
    let text = vec![EMPTY_TEXT_REPLACEMENT; lines].join("\n");
    let intrinsics = ParagraphIntrinsics::new(&text, style, &[], &[], density, font_family_resolver, soft_wrap);
    Paragraph::new(Rc::new(intrinsics), Constraints::default(), lines, TextOverflow::Clip)
}

/// The size of `lines` lines of the reference text (Compose's `computeSizeForDefaultText`): what a field
/// reserves when it is empty.
pub(crate) fn compute_size_for_default_text(
    style: &TextStyle,
    density: Density,
    font_family_resolver: &Rc<dyn FontFamilyResolver>,
    lines: usize,
) -> IntSize {
    let paragraph = paragraph_for_default_text(style, density, font_family_resolver, lines, lines > 1);
    IntSize::new(paragraph.min_intrinsic_width().ceil() as i32, paragraph.height().ceil() as i32)
}

/// The face `style` resolves to, as a state (what a node observes to measure again when an async font loads).
fn resolve_font(style: &TextStyle, resolver: &Rc<dyn FontFamilyResolver>) -> State<Typeface> {
    resolver.resolve(
        style.font_family.as_ref(),
        style.font_weight.unwrap_or(FontWeight::Normal),
        style.font_style.unwrap_or(FontStyle::Normal),
        style.font_synthesis.unwrap_or(FontSynthesis::All),
    )
}

/// A height of `lines_limit` lines from three measured lines' heights, none (-1) for the default limit (Compose's
/// `computeHeightFromThreeLines`).
fn compute_height_from_three_lines(top: f32, middle: f32, bottom: f32, lines_limit: i32, default_limit: i32) -> i32 {
    if lines_limit == default_limit {
        -1
    } else {
        (top + middle * (lines_limit - 2).max(0) as f32 + bottom * (lines_limit - 1).min(1) as f32).round() as i32
    }
}

/// Checks a field's line limits (Compose's `validateMinMaxLines`).
pub(crate) fn validate_min_max_lines(min_lines: i32, max_lines: i32) {
    assert!(
        min_lines > 0 && max_lines > 0,
        "both minLines {min_lines} and maxLines {max_lines} must be greater than zero"
    );
    assert!(min_lines <= max_lines, "minLines {min_lines} must be less than or equal to maxLines {max_lines}");
}

/// The text field modifiers foundation's fields share.
pub(crate) trait TextFieldSizeModifiers {
    /// A wrapping field's min and max heights in lines of `text_style` (Compose's `heightInLines`): nothing for
    /// a field that doesn't wrap (a single line has its own height).
    fn height_in_lines(self, text_style: &TextStyle, min_lines: i32, max_lines: i32, soft_wrap: bool) -> Modifier;
    /// A field at least ten "H" wide and a line tall (Compose's `textFieldMinSize`).
    fn text_field_min_size(self, style: &TextStyle) -> Modifier;
    /// A state-based single-line field as tall as its laid out line (Compose's `heightForSingleLineField`, the
    /// min size optimization's layout): nothing for a multi-line one (its height 0).
    fn height_for_single_line_field(self, text_layout_state: &Rc<TextLayoutState>) -> Modifier;
}

impl TextFieldSizeModifiers for Modifier {
    fn height_in_lines(self, text_style: &TextStyle, min_lines: i32, max_lines: i32, soft_wrap: bool) -> Modifier {
        validate_min_max_lines(min_lines, max_lines);
        if min_lines == 1 && max_lines == i32::MAX {
            return self;
        }
        if !soft_wrap {
            return self;
        }
        self.then_element(HeightInLinesElement { text_style: text_style.clone(), min_lines, max_lines })
    }

    fn text_field_min_size(self, style: &TextStyle) -> Modifier {
        self.then_element(TextFieldSizeElement { style: style.clone() })
    }

    fn height_for_single_line_field(self, text_layout_state: &Rc<TextLayoutState>) -> Modifier {
        self.then_element(HeightForSingleLineFieldElement(text_layout_state.clone()))
    }
}

// ---------------------------------------------------------------------------------------------------------------
// heightInLines

#[derive(PartialEq)]
struct HeightInLinesElement {
    text_style: TextStyle,
    min_lines: i32,
    max_lines: i32,
}

impl ModifierNodeElement for HeightInLinesElement {
    type Node = HeightInLinesNode;
    fn create(&self) -> HeightInLinesNode {
        HeightInLinesNode {
            text_style: self.text_style.clone(),
            min_lines: self.min_lines,
            max_lines: self.max_lines,
            dirty: false,
            precomputed_min_lines_height: -1,
            precomputed_max_lines_height: -1,
            resolved_style: None,
            font_resolution_state: None,
            computed_density: None,
            handle: None,
        }
    }
    fn update(&self, node: &mut HeightInLinesNode) {
        node.update(&self.text_style, self.min_lines, self.max_lines);
    }
    fn name(&self) -> &'static str {
        "heightInLines"
    }
}

/// Compose's HeightInLinesNode: the heights of the min and max lines worked out from three lines of the style
/// (the height in lines optimization), again when the font loads, the density or the direction changes.
struct HeightInLinesNode {
    text_style: TextStyle,
    min_lines: i32,
    max_lines: i32,
    dirty: bool,
    precomputed_min_lines_height: i32,
    precomputed_max_lines_height: i32,
    resolved_style: Option<TextStyle>,
    font_resolution_state: Option<State<Typeface>>,
    /// The density the heights were worked out in (a new one works them out again: Compose's `onDensityChange`).
    computed_density: Option<Density>,
    handle: Option<NodeHandle>,
}

impl HeightInLinesNode {
    fn resolver(&self) -> Rc<dyn FontFamilyResolver> {
        self.handle
            .as_ref()
            .map_or_else(|| LocalFontFamilyResolver.default_value(), |h| h.current_value_of(&LocalFontFamilyResolver))
    }

    fn layout_direction(&self) -> LayoutDirection {
        self.handle.as_ref().map_or(LayoutDirection::Ltr, NodeHandle::layout_direction)
    }

    fn observe_font(&mut self) {
        if let (Some(h), Some(state)) = (&self.handle, &self.font_resolution_state) {
            let state = state.clone();
            h.observe_reads(|| {
                state.get();
            });
        }
    }

    fn compute_heights(&mut self, density: Density) {
        self.computed_density = Some(density);
        let Some(style) = self.resolved_style.clone() else { return };
        let three = paragraph_for_default_text(&style, density, &self.resolver(), 3, true);
        let (first, second, third) = (three.get_line_height(0), three.get_line_height(1), three.get_line_height(2));
        self.precomputed_min_lines_height = compute_height_from_three_lines(first, second, third, self.min_lines, 1);
        self.precomputed_max_lines_height =
            compute_height_from_three_lines(first, second, third, self.max_lines, i32::MAX);
    }

    fn update(&mut self, text_style: &TextStyle, min_lines: i32, max_lines: i32) {
        if self.text_style != *text_style || self.min_lines != min_lines || self.max_lines != max_lines {
            self.text_style = text_style.clone();
            self.min_lines = min_lines;
            self.max_lines = max_lines;
            self.resolved_style = Some(text_style.resolve_defaults(self.layout_direction()));
            self.dirty = true;
            if let Some(h) = &self.handle {
                h.invalidate_measurement();
            }
        }
    }
}

impl ModifierNode for HeightInLinesNode {
    node_capabilities!(layout, observer);

    fn should_auto_invalidate(&self) -> bool {
        false
    }

    fn on_attach(&mut self, cx: &NodeCx) {
        self.handle = Some(cx.handle());
        let resolved = self.text_style.resolve_defaults(cx.layout_direction());
        // The font resolved at once, not blocking the layout pass; its state observed.
        self.font_resolution_state = Some(resolve_font(&resolved, &cx.current_value_of(&LocalFontFamilyResolver)));
        self.resolved_style = Some(resolved);
        self.observe_font();
        self.dirty = true;
    }

    fn on_detach(&mut self, _cx: &NodeCx) {
        self.resolved_style = None;
        self.font_resolution_state = None;
        self.dirty = false;
    }

    fn on_layout_direction_change(&mut self, cx: &NodeCx) {
        self.resolved_style = Some(self.text_style.resolve_defaults(cx.layout_direction()));
        self.dirty = true;
        cx.invalidate_measurement();
    }
}

impl ObserverModifierNode for HeightInLinesNode {
    fn on_observed_reads_changed(&mut self) {
        self.observe_font();
        self.dirty = true;
        if let Some(h) = &self.handle {
            h.invalidate_measurement();
        }
    }
}

impl LayoutModifierNode for HeightInLinesNode {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let density = Density::new(scope.density, scope.font_scale);
        if self.dirty || self.computed_density != Some(density) {
            self.compute_heights(density);
            self.dirty = false;
        }
        let min_height = if self.precomputed_min_lines_height != -1 {
            self.precomputed_min_lines_height.clamp(constraints.min_height, constraints.max_height)
        } else {
            constraints.min_height
        };
        let max_height = if self.precomputed_max_lines_height != -1 {
            self.precomputed_max_lines_height.clamp(constraints.min_height, constraints.max_height)
        } else {
            constraints.max_height
        };
        let placeable =
            measurable.measure(Constraints::new(constraints.min_width, constraints.max_width, min_height, max_height));
        let (w, h) = (placeable.width(), placeable.height());
        MeasureResult::new(w, h, Vec::new(), move |p| p.place_relative(&placeable, 0, 0))
    }

    fn min_intrinsic_height(
        &mut self,
        scope: &kompoz_ui::layout::IntrinsicMeasureScope,
        measurable: &dyn kompoz_ui::layout::IntrinsicMeasurable,
        width: i32,
    ) -> i32 {
        self.get_or_precompute_min_max_height(scope.density());
        if self.precomputed_min_lines_height == self.precomputed_max_lines_height {
            self.precomputed_min_lines_height
        } else {
            measurable
                .min_intrinsic_height(width)
                .clamp(self.precomputed_min_lines_height, self.precomputed_max_lines_height)
        }
    }

    fn max_intrinsic_height(
        &mut self,
        scope: &kompoz_ui::layout::IntrinsicMeasureScope,
        measurable: &dyn kompoz_ui::layout::IntrinsicMeasurable,
        width: i32,
    ) -> i32 {
        self.get_or_precompute_min_max_height(scope.density());
        if self.precomputed_min_lines_height == self.precomputed_max_lines_height {
            self.precomputed_max_lines_height
        } else {
            measurable
                .max_intrinsic_height(width)
                .clamp(self.precomputed_min_lines_height, self.precomputed_max_lines_height)
        }
    }
}

impl HeightInLinesNode {
    /// Compose's `getOrPrecomputeMinMaxHeight`: a missing min is 0, a missing max unbounded.
    fn get_or_precompute_min_max_height(&mut self, density: Density) {
        if self.dirty || self.computed_density != Some(density) {
            self.compute_heights(density);
            self.dirty = false;
        }
        self.precomputed_min_lines_height = self.precomputed_min_lines_height.max(0);
        if self.precomputed_max_lines_height == -1 {
            self.precomputed_max_lines_height = Constraints::Infinity;
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// textFieldMinSize

#[derive(PartialEq)]
struct TextFieldSizeElement {
    style: TextStyle,
}

impl ModifierNodeElement for TextFieldSizeElement {
    type Node = TextFieldSizeNode;
    fn create(&self) -> TextFieldSizeNode {
        TextFieldSizeNode {
            style: self.style.clone(),
            resolved_style: None,
            font_resolution_state: None,
            min_size: None,
            dirty: true,
            handle: None,
        }
    }
    fn update(&self, node: &mut TextFieldSizeNode) {
        node.update(&self.style);
    }
    fn name(&self) -> &'static str {
        "textFieldMinSize"
    }
}

/// Compose's TextFieldSizeNode with its TextFieldSize: the reference text's size, measured again when the font
/// loads (its state read in measure), the density, the direction or the style changes.
struct TextFieldSizeNode {
    style: TextStyle,
    resolved_style: Option<TextStyle>,
    font_resolution_state: Option<State<Typeface>>,
    /// The size and the density, direction and face it was measured with.
    min_size: Option<(IntSize, Density, LayoutDirection, Typeface)>,
    dirty: bool,
    handle: Option<NodeHandle>,
}

impl TextFieldSizeNode {
    fn resolver(&self) -> Rc<dyn FontFamilyResolver> {
        self.handle
            .as_ref()
            .map_or_else(|| LocalFontFamilyResolver.default_value(), |h| h.current_value_of(&LocalFontFamilyResolver))
    }

    fn update_font_resolution_state(&mut self) {
        if let Some(style) = &self.resolved_style {
            self.font_resolution_state = Some(resolve_font(style, &self.resolver()));
        }
        self.dirty = true;
        if let Some(h) = &self.handle {
            h.invalidate_measurement();
        }
    }

    fn update(&mut self, style: &TextStyle) {
        self.style = style.clone();
        let direction = self.handle.as_ref().map_or(LayoutDirection::Ltr, NodeHandle::layout_direction);
        self.resolved_style = Some(style.resolve_defaults(direction));
        self.update_font_resolution_state();
    }

    /// The min size, measured again when the face, density or direction changed (Compose's
    /// `cachedMinSizeOrComputeMinSize`).
    fn cached_min_size_or_compute(
        &mut self,
        typeface: Typeface,
        density: Density,
        direction: LayoutDirection,
    ) -> IntSize {
        if let Some((size, d, l, t)) = &self.min_size
            && !self.dirty
            && *d == density
            && *l == direction
            && *t == typeface
        {
            return *size;
        }
        let style = self.resolved_style.clone().unwrap_or_else(|| self.style.resolve_defaults(direction));
        let size = compute_size_for_default_text(&style, density, &self.resolver(), 1);
        self.min_size = Some((size, density, direction, typeface));
        self.dirty = false;
        size
    }
}

impl ModifierNode for TextFieldSizeNode {
    node_capabilities!(layout);

    fn should_auto_invalidate(&self) -> bool {
        false
    }

    fn on_attach(&mut self, cx: &NodeCx) {
        self.handle = Some(cx.handle());
        self.resolved_style = Some(self.style.resolve_defaults(cx.layout_direction()));
        self.update_font_resolution_state();
    }

    fn on_detach(&mut self, _cx: &NodeCx) {
        self.font_resolution_state = None;
        self.min_size = None;
    }

    fn on_layout_direction_change(&mut self, cx: &NodeCx) {
        self.resolved_style = Some(self.style.resolve_defaults(cx.layout_direction()));
        self.dirty = true;
        cx.invalidate_measurement();
    }
}

impl LayoutModifierNode for TextFieldSizeNode {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        // The face read here: a font loading later measures it again.
        let typeface = match &self.font_resolution_state {
            Some(state) => state.get(),
            None => resolve_font(&self.style, &self.resolver()).get(),
        };
        let min_size = self.cached_min_size_or_compute(
            typeface,
            Density::new(scope.density, scope.font_scale),
            scope.layout_direction(),
        );
        let child = constraints.constrain(Constraints::new(
            min_size.width,
            Constraints::Infinity,
            min_size.height,
            Constraints::Infinity,
        ));
        let placeable = measurable.measure(child);
        let (w, h) = (placeable.width(), placeable.height());
        MeasureResult::new(w, h, Vec::new(), move |p| p.place_relative(&placeable, 0, 0))
    }
}

// ---------------------------------------------------------------------------------------------------------------
// heightForSingleLineField

struct HeightForSingleLineFieldElement(Rc<TextLayoutState>);

impl PartialEq for HeightForSingleLineFieldElement {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl ModifierNodeElement for HeightForSingleLineFieldElement {
    type Node = HeightForSingleLineFieldNode;
    fn create(&self) -> HeightForSingleLineFieldNode {
        HeightForSingleLineFieldNode(self.0.clone())
    }
    fn update(&self, node: &mut HeightForSingleLineFieldNode) {
        node.0 = self.0.clone();
    }
    fn name(&self) -> &'static str {
        "heightForSingleLineField"
    }
}

/// Compose's `heightForSingleLineField` layout (isBasicTextFieldMinSizeOptimizationEnabled): the height the text
/// layout node gave (read as a state), any width.
struct HeightForSingleLineFieldNode(Rc<TextLayoutState>);

impl ModifierNode for HeightForSingleLineFieldNode {
    node_capabilities!(layout);
}

impl LayoutModifierNode for HeightForSingleLineFieldNode {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let height: Dp = self.0.height_for_single_line_field();
        let height_px = Density::new(scope.density, scope.font_scale).round_to_px(height);
        let wrapped = constraints.constrain(Constraints::new(
            0,
            Constraints::Infinity,
            height_px,
            if height == Dp(0.0) { Constraints::Infinity } else { height_px },
        ));
        let placeable = measurable.measure(wrapped);
        let (w, h) = (placeable.width(), placeable.height());
        MeasureResult::new(w, h, Vec::new(), move |p| p.place_relative(&placeable, 0, 0))
    }
}
