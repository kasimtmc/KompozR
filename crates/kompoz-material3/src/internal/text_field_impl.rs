//! What the text fields share, as Compose's internal TextFieldImpl.kt: the decoration box both kinds of field
//! compose around their inner field ([`CommonDecorationBox`]: the label moving between the text's place and its
//! minimized place, the placeholder fading in and out, the prefix and suffix shown, the slots in their colors and
//! styles), the input phase's transition the animations follow, the layout ids, paddings and sizes their layouts
//! read, and the modifiers they take.

use crate::content_color::LocalContentColor;
use crate::interactive_component_size::LocalMinimumInteractiveComponentSize;
use crate::internal::float_producer::FloatProducer;
use crate::internal::provide_content_color_text_style::ProvideContentColorTextStyle;
use crate::material_theme::MaterialTheme;
use crate::outlined_text_field::{OutlineCutoutModifiers, OutlinedTextFieldLayout};
use crate::text_field::TextFieldLayout;
use crate::text_field_defaults::{
    TextFieldColors, TextFieldLabel, TextFieldLabelPosition, TextFieldLabelScope, TextFieldSlot,
};
use crate::tokens::{MotionSchemeKeyTokens, SmallIconButtonTokens, TypeScaleTokens};
use kompoz_animation::{Segment, Transition, update_transition};
use kompoz_foundation::prelude::*;
use kompoz_graphics::{Color, ColorProducer, Dp, Shape, Size, lerp_dp};
use kompoz_runtime::{
    KompozitionLocalProvider, State, derived_state_of, kompozable, mutable_state_of, named_args, remember,
    remember_keyed,
};
use kompoz_text::input::VisualTransformation;
use kompoz_text::{AnnotatedString, TextStyle, lerp_text_style};
use kompoz_ui::layout::{Measurable, MeasureScope};
use kompoz_ui::platform::LocalDensity;
use kompoz_ui::{Constraints, HorizontalAlignment, Modifier};
use std::rc::Rc;
use std::sync::Arc;

/// Which field a decoration box decorates (Compose's internal `TextFieldType`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum TextFieldType {
    Filled,
    Outlined,
}

/// The decoration box of both kinds of field (Compose's internal `CommonDecorationBox`): `inner_text_field` with
/// the label (where `label_position` says, moving as the field is focused and filled), the placeholder (while
/// `visual_text` is empty, fading in and out), the icons, the prefix and suffix (shown while the label is
/// minimized), the supporting text, each in its color of `colors` for the field's state (`enabled`, `is_error`,
/// focused as `interaction_source` says) and in the theme's body styles, laid out by the filled or outlined
/// layout in `content_padding` on `container` (the outlined one's border cut where its minimized label is).
///
/// Inline (Compose's is restartable): the inner field a decorator is given is borrowed for the call.
#[kompozable(inline)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn CommonDecorationBox(
    type_: TextFieldType,
    visual_text: String,
    inner_text_field: &dyn Fn(),
    label_position: TextFieldLabelPosition,
    label: Option<TextFieldLabel>,
    placeholder: Option<TextFieldSlot>,
    leading_icon: Option<TextFieldSlot>,
    trailing_icon: Option<TextFieldSlot>,
    prefix: Option<TextFieldSlot>,
    suffix: Option<TextFieldSlot>,
    supporting_text: Option<TextFieldSlot>,
    single_line: bool,
    enabled: bool,
    is_error: bool,
    interaction_source: InteractionSource,
    content_padding: PaddingValues,
    colors: TextFieldColors,
    container: TextFieldSlot,
) {
    let is_focused = interaction_source.collect_is_focused_as_state().get();
    let input_state = if is_focused {
        InputPhase::Focused
    } else if visual_text.is_empty() {
        InputPhase::UnfocusedEmpty
    } else {
        InputPhase::UnfocusedNotEmpty
    };

    let typography = MaterialTheme::typography();
    let body_large = typography.body_large.clone();
    let body_small = typography.body_small.clone();
    let override_label_text_style_color = (body_large.color == Color::Unspecified
        && body_small.color != Color::Unspecified)
        || (body_large.color != Color::Unspecified && body_small.color == Color::Unspecified);

    let transition = update_transition(input_state, Some("TextFieldInputState".to_string()));

    let show_expanded_label = label.is_some() && label_position.show_expanded_label();
    let label_progress = if label.is_some() { Some(label_progress(&transition, show_expanded_label)) } else { None };
    let placeholder_alpha =
        if placeholder.is_some() { Some(placeholder_opacity(&transition, show_expanded_label)) } else { None };
    let affix_alpha =
        if prefix.is_some() || suffix.is_some() { Some(affix_opacity(&transition, show_expanded_label)) } else { None };

    let decorated_label: Option<TextFieldSlot> = label.map(|label| {
        let (label_progress, transition) = (label_progress.clone(), transition.clone());
        let (body_small, body_large) = (body_small.clone(), body_large.clone());
        Rc::new(move || {
            DecoratedLabel(
                label_progress.clone(),
                colors,
                enabled,
                is_error,
                is_focused,
                override_label_text_style_color,
                transition.clone(),
                body_small.clone(),
                body_large.clone(),
                label.clone(),
            )
        }) as TextFieldSlot
    });

    let placeholder_color = colors.placeholder_color(enabled, is_error, is_focused);
    // (Compose's `remember { derivedStateOf { … } }` reads the alpha of the first composition)
    let alpha = placeholder_alpha.clone();
    let show_placeholder =
        remember(move || derived_state_of(move || alpha.as_ref().map_or(0.0, |alpha| alpha.get()) > 0.0));
    let decorated_placeholder: Option<Rc<dyn Fn(Modifier)>> = match placeholder {
        Some(placeholder) if visual_text.is_empty() && show_placeholder.get() => {
            let style = body_large.clone();
            Some(Rc::new(move |modifier: Modifier| {
                let (style, placeholder) = (style.clone(), placeholder.clone());
                Box(
                    modifier = modifier,
                    content = move || Decoration(placeholder_color, style.clone(), placeholder.clone()),
                );
            }) as Rc<dyn Fn(Modifier)>)
        }
        _ => None,
    };

    let prefix_color = colors.prefix_color(enabled, is_error, is_focused);
    let alpha = affix_alpha.clone();
    let show_affix = remember(move || derived_state_of(move || alpha.as_ref().map_or(0.0, |alpha| alpha.get()) > 0.0));
    let decorated_prefix: Option<TextFieldSlot> = match prefix {
        Some(prefix) if show_affix.get() => {
            let style = body_large.clone();
            Some(Rc::new(move || Decoration(prefix_color, style.clone(), prefix.clone())) as TextFieldSlot)
        }
        _ => None,
    };

    let suffix_color = colors.suffix_color(enabled, is_error, is_focused);
    let decorated_suffix: Option<TextFieldSlot> = match suffix {
        Some(suffix) if show_affix.get() => {
            let style = body_large.clone();
            Some(Rc::new(move || Decoration(suffix_color, style.clone(), suffix.clone())) as TextFieldSlot)
        }
        _ => None,
    };

    let leading_icon_color = colors.leading_icon_color(enabled, is_error, is_focused);
    let decorated_leading: Option<TextFieldSlot> =
        leading_icon.map(|it| Rc::new(move || ContentColorDecoration(leading_icon_color, it.clone())) as TextFieldSlot);

    let trailing_icon_color = colors.trailing_icon_color(enabled, is_error, is_focused);
    let decorated_trailing: Option<TextFieldSlot> = trailing_icon
        .map(|it| Rc::new(move || ContentColorDecoration(trailing_icon_color, it.clone())) as TextFieldSlot);

    let supporting_text_color = colors.supporting_text_color(enabled, is_error, is_focused);
    let decorated_supporting: Option<TextFieldSlot> = supporting_text.map(|it| {
        let style = body_small.clone();
        Rc::new(move || Decoration(supporting_text_color, style.clone(), it.clone())) as TextFieldSlot
    });

    // (Compose's lambdas, kept by the states they read as its compiler keeps them)
    let progress = label_progress.clone();
    let label_progress_producer = remember_keyed(label_progress.clone(), move || {
        FloatProducer::new(move || progress.as_ref().map_or(1.0, |progress| progress.get()))
    });
    let alpha = placeholder_alpha.clone();
    let placeholder_alpha_producer = remember_keyed(placeholder_alpha, move || {
        FloatProducer::new(move || alpha.as_ref().map_or(0.0, |alpha| alpha.get()))
    });
    let alpha = affix_alpha.clone();
    let affix_alpha_producer = remember_keyed(affix_alpha, move || {
        FloatProducer::new(move || alpha.as_ref().map_or(0.0, |alpha| alpha.get()))
    });

    match type_ {
        TextFieldType::Filled => {
            let container_with_id = move || {
                let container = container.clone();
                Box(
                    modifier = Modifier.layout_id(ContainerId),
                    propagate_min_constraints = true,
                    content = move || container(),
                );
            };
            TextFieldLayout(
                Modifier,
                inner_text_field,
                decorated_label,
                decorated_placeholder,
                decorated_leading,
                decorated_trailing,
                decorated_prefix,
                decorated_suffix,
                single_line,
                label_position,
                label_progress_producer,
                placeholder_alpha_producer,
                affix_alpha_producer,
                &container_with_id,
                decorated_supporting,
                content_padding,
            );
        }
        TextFieldType::Outlined => {
            let cutout_size = remember(|| mutable_state_of(Size::Zero));
            let (size, alignment, padding) =
                (cutout_size.clone(), label_position.minimized_alignment(), content_padding.clone());
            let border_container_with_id = move || {
                let (size, container) = (size.clone(), container.clone());
                Box(
                    modifier = Modifier.layout_id(ContainerId).outline_cutout(
                        move || size.get(),
                        alignment.clone(),
                        padding.clone(),
                    ),
                    propagate_min_constraints = true,
                    content = move || container(),
                );
            };
            // (Compose's lambda, kept by what it reads as its compiler keeps it)
            let (position, producer) = (label_position.clone(), label_progress_producer.clone());
            let on_label_measured: Rc<dyn Fn(Size)> =
                remember_keyed((label_position.clone(), label_progress_producer.clone()), move || {
                    Rc::new(move |it: Size| {
                        if position.is_above() {
                            return;
                        }
                        let progress = producer.invoke();
                        let label_width = it.width * progress;
                        let label_height = it.height * progress;
                        let size = cutout_size.peek();
                        if size.width != label_width || size.height != label_height {
                            cutout_size.set(Size::new(label_width, label_height));
                        }
                    }) as Rc<dyn Fn(Size)>
                });
            OutlinedTextFieldLayout(
                Modifier,
                inner_text_field,
                decorated_placeholder,
                decorated_label,
                decorated_leading,
                decorated_trailing,
                decorated_prefix,
                decorated_suffix,
                single_line,
                label_position,
                label_progress_producer,
                placeholder_alpha_producer,
                affix_alpha_producer,
                on_label_measured,
                &border_container_with_id,
                decorated_supporting,
                content_padding,
            );
        }
    }
}

/// The decoration box of both kinds of value-based field (the bodies of Compose's `DecorationBox`es): `value`
/// through `visual_transformation` (remembered for them) is what tells empty; the label attached. The slots in
/// Compose's order: placeholder, leading and trailing icons, prefix, suffix, supporting text.
#[kompozable(inline)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn ValueDecorationBox(
    type_: TextFieldType,
    value: String,
    inner_text_field: &dyn Fn(),
    enabled: bool,
    single_line: bool,
    visual_transformation: VisualTransformation,
    interaction_source: InteractionSource,
    is_error: bool,
    label: Option<TextFieldSlot>,
    slots: [Option<TextFieldSlot>; 6],
    colors: TextFieldColors,
    content_padding: PaddingValues,
    container: TextFieldSlot,
) {
    let (text, transformation) = (value.clone(), visual_transformation.clone());
    let visual_text =
        remember_keyed((value, visual_transformation), move || transformation.filter(&AnnotatedString::new(text)))
            .text
            .text()
            .to_string();
    let [placeholder, leading_icon, trailing_icon, prefix, suffix, supporting_text] = slots;
    let label: Option<TextFieldLabel> =
        label.map(|it| Rc::new(move |_: &dyn TextFieldLabelScope| it()) as TextFieldLabel);
    CommonDecorationBox(
        type_,
        visual_text,
        inner_text_field,
        TextFieldLabelPosition::default(),
        label,
        placeholder,
        leading_icon,
        trailing_icon,
        prefix,
        suffix,
        supporting_text,
        single_line,
        enabled,
        is_error,
        interaction_source,
        content_padding,
        colors,
        container,
    );
}

/// The label in its color and style (Compose's private `DecoratedLabel`): the label color of the field's state
/// animated in, the style the body large one going to the body small one as the label minimizes — the style's own
/// color animated between the two's where only one of them has one (the other taking the label color).
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn DecoratedLabel(
    label_progress: Option<State<f32>>,
    colors: TextFieldColors,
    enabled: bool,
    is_error: bool,
    is_focused: bool,
    override_label_text_style_color: bool,
    transition: Transition<InputPhase>,
    body_small: TextStyle,
    body_large: TextStyle,
    content: TextFieldLabel,
) {
    // (Compose's `remember { object : TextFieldLabelScope { … } }` reads the progress of the first composition)
    let progress = label_progress.clone();
    let label_scope = remember(move || Rc::new(LabelScope(progress)));
    let label_color = colors.label_color(enabled, is_error, is_focused);
    let label_text_style_color = if override_label_text_style_color {
        Some(label_text_style_color(
            &transition,
            body_small.color.take_or_else(|| label_color),
            body_large.color.take_or_else(|| label_color),
        ))
    } else {
        None
    };
    let label_content_color = label_content_color(&transition, label_color);
    let progress = label_progress.as_ref().map_or(1.0, |progress| progress.get());
    let text_style = lerp_text_style(&body_large, &body_small, progress);
    let label_text_style = match &label_text_style_color {
        Some(color) => named_args!(text_style.copy(color = color.get())),
        None => text_style,
    };
    Decoration(label_content_color.get(), label_text_style, Rc::new(move || content(&*label_scope)));
}

/// The scope a label is composed in (Compose's `object : TextFieldLabelScope`): the progress read as asked, 1
/// without one.
struct LabelScope(Option<State<f32>>);

impl TextFieldLabelScope for LabelScope {
    fn label_minimized_progress(&self) -> f32 {
        self.0.as_ref().map_or(1.0, |progress| progress.get())
    }
}

impl TextFieldLabelPosition {
    /// Whether the label shows expanded at the text's place while the field is empty and unfocused (Compose's
    /// private `showExpandedLabel`): attached, not always minimized.
    pub(crate) fn show_expanded_label(&self) -> bool {
        matches!(self, TextFieldLabelPosition::Attached { always_minimize: false, .. })
    }

    /// Where the minimized label goes across (Compose's internal `minimizedAlignment`).
    pub(crate) fn minimized_alignment(&self) -> HorizontalAlignment {
        match self {
            TextFieldLabelPosition::Above { alignment } => alignment.clone(),
            TextFieldLabelPosition::Attached { minimized_alignment, .. } => minimized_alignment.clone(),
        }
    }

    /// Where the expanded label goes across (Compose's internal `expandedAlignment`).
    pub(crate) fn expanded_alignment(&self) -> HorizontalAlignment {
        match self {
            TextFieldLabelPosition::Above { alignment } => alignment.clone(),
            TextFieldLabelPosition::Attached { expanded_alignment, .. } => expanded_alignment.clone(),
        }
    }
}

/// A slot in a content color and a text style merged into the one around (Compose's private `Decoration(contentColor,
/// textStyle, content)`).
#[kompozable]
fn Decoration(content_color: Color, text_style: TextStyle, content: TextFieldSlot) {
    ProvideContentColorTextStyle(content_color, text_style, move || content());
}

/// A slot in a content color (Compose's private `Decoration(contentColor, content)`).
#[kompozable]
fn ContentColorDecoration(content_color: Color, content: TextFieldSlot) {
    KompozitionLocalProvider(&[LocalContentColor.provides(content_color)], move || content());
}

/// The modifiers the fields take (Compose's internal ones of TextFieldImpl.kt).
pub(crate) trait TextFieldImplModifiers {
    /// An error told to accessibility, `default_error_message`, while `is_error` (Compose's internal
    /// `Modifier.defaultErrorSemantics`).
    fn default_error_semantics(self, is_error: bool, default_error_message: String) -> Modifier;

    /// `color` in `shape` behind (Compose's internal `Modifier.textFieldBackground`).
    fn text_field_background(self, color: ColorProducer, shape: Arc<dyn Shape>) -> Modifier;

    /// At least `min_height` tall (unspecified: no least), within the constraints (Compose's internal
    /// `Modifier.textFieldLabelMinHeight`): read as it measures.
    fn text_field_label_min_height(self, min_height: impl Fn() -> Dp + 'static) -> Modifier;
}

impl TextFieldImplModifiers for Modifier {
    fn default_error_semantics(self, is_error: bool, default_error_message: String) -> Modifier {
        if is_error {
            named_args!(self.semantics(properties = move |c| c.error(default_error_message.clone())))
        } else {
            self
        }
    }

    fn text_field_background(self, color: ColorProducer, shape: Arc<dyn Shape>) -> Modifier {
        self.draw_with_cache(move |scope| {
            let outline = shape.create_outline(scope.size(), scope.layout_direction(), &scope.density());
            let color = color.clone();
            scope.on_draw_behind(move |d| d.draw_outline(&outline, color.get()))
        })
    }

    fn text_field_label_min_height(self, min_height: impl Fn() -> Dp + 'static) -> Modifier {
        self.layout(move |scope: &MeasureScope, measurable: &dyn Measurable, constraints: Constraints| {
            let min_height = min_height();
            let resolved_min_height =
                constraints.constrain_height(if min_height.is_specified() { scope.round_to_px(min_height) } else { 0 });
            let placeable = measurable.measure(named_args!(constraints.copy(min_height = resolved_min_height)));
            let (width, height) = (placeable.width(), placeable.height());
            scope.layout(width, height, move |p| p.place(&placeable, 0, 0))
        })
    }
}

/// How far the label is minimized in each phase (Compose's private `Transition<InputPhase>.labelProgress`): 1
/// focused or filled, 0 empty and unfocused where the label shows expanded; by the theme's fast spatial motion.
#[kompozable]
fn label_progress(transition: &Transition<InputPhase>, show_expanded_label: bool) -> State<f32> {
    let label_transition_spec = MotionSchemeKeyTokens::FastSpatial.value::<f32>();
    transition.animate_float(
        transition_spec = move |_: &Segment<InputPhase>| label_transition_spec.clone(),
        label = "LabelProgress".to_string(),
        target_value_by_state = move |it: &InputPhase| match it {
            InputPhase::Focused => 1.0,
            InputPhase::UnfocusedEmpty => {
                if show_expanded_label {
                    0.0
                } else {
                    1.0
                }
            }
            InputPhase::UnfocusedNotEmpty => 1.0,
        },
    )
}

/// The placeholder's opacity in each phase (Compose's private `Transition<InputPhase>.placeholderOpacity`): shown
/// focused, and empty and unfocused where the label doesn't show expanded; by the theme's slow effects motion as it
/// comes in focusing an empty field or emptying an unfocused one, its fast effects motion otherwise.
#[kompozable]
fn placeholder_opacity(transition: &Transition<InputPhase>, show_expanded_label: bool) -> State<f32> {
    let fast_opacity_transition_spec = MotionSchemeKeyTokens::FastEffects.value::<f32>();
    let slow_opacity_transition_spec = MotionSchemeKeyTokens::SlowEffects.value::<f32>();
    transition.animate_float(
        transition_spec = move |segment: &Segment<InputPhase>| {
            if segment.is_transitioning_to(InputPhase::Focused, InputPhase::UnfocusedEmpty) {
                fast_opacity_transition_spec.clone()
            } else if segment.is_transitioning_to(InputPhase::UnfocusedEmpty, InputPhase::Focused)
                || segment.is_transitioning_to(InputPhase::UnfocusedNotEmpty, InputPhase::UnfocusedEmpty)
            {
                slow_opacity_transition_spec.clone()
            } else {
                fast_opacity_transition_spec.clone()
            }
        },
        label = "PlaceholderOpacity".to_string(),
        target_value_by_state = move |it: &InputPhase| match it {
            InputPhase::Focused => 1.0,
            InputPhase::UnfocusedEmpty => {
                if show_expanded_label {
                    0.0
                } else {
                    1.0
                }
            }
            InputPhase::UnfocusedNotEmpty => 0.0,
        },
    )
}

/// The prefix's and suffix's opacity in each phase (Compose's private `Transition<InputPhase>.affixOpacity`):
/// shown but while empty and unfocused where the label shows expanded; by the theme's fast effects motion.
#[kompozable]
fn affix_opacity(transition: &Transition<InputPhase>, show_expanded_label: bool) -> State<f32> {
    let fast_opacity_transition_spec = MotionSchemeKeyTokens::FastEffects.value::<f32>();
    transition.animate_float(
        transition_spec = move |_: &Segment<InputPhase>| fast_opacity_transition_spec.clone(),
        label = "PrefixSuffixOpacity".to_string(),
        target_value_by_state = move |it: &InputPhase| match it {
            InputPhase::Focused => 1.0,
            InputPhase::UnfocusedEmpty => {
                if show_expanded_label {
                    0.0
                } else {
                    1.0
                }
            }
            InputPhase::UnfocusedNotEmpty => 1.0,
        },
    )
}

/// The label style's own color in each phase (Compose's private `Transition<InputPhase>.labelTextStyleColor`): the
/// focused one focused, the unfocused one otherwise; by the theme's fast effects motion.
#[kompozable]
fn label_text_style_color(
    transition: &Transition<InputPhase>,
    focused_label_text_style_color: Color,
    unfocused_label_text_style_color: Color,
) -> State<Color> {
    let color_transition_spec = MotionSchemeKeyTokens::FastEffects.value::<Color>();
    transition.animate_color(
        transition_spec = move |_: &Segment<InputPhase>| color_transition_spec.clone(),
        label = "LabelTextStyleColor".to_string(),
        target_value_by_state = move |it: &InputPhase| match it {
            InputPhase::Focused => focused_label_text_style_color,
            _ => unfocused_label_text_style_color,
        },
    )
}

/// The label's content color (Compose's private `Transition<InputPhase>.labelContentColor`): `label_color` in every
/// phase — a new one animated to as the transition runs again for it —, by the theme's fast effects motion.
#[kompozable]
fn label_content_color(transition: &Transition<InputPhase>, label_color: Color) -> State<Color> {
    let color_transition_spec = MotionSchemeKeyTokens::FastEffects.value::<Color>();
    transition.animate_color(
        transition_spec = move |_: &Segment<InputPhase>| color_transition_spec.clone(),
        label = "LabelContentColor".to_string(),
        target_value_by_state = move |_: &InputPhase| label_color,
    )
}

/// The phases of a field the decoration follows (Compose's private `InputPhase`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum InputPhase {
    /// Focused, empty or not.
    Focused,
    /// Unfocused and empty.
    UnfocusedEmpty,
    /// Unfocused, with text.
    UnfocusedNotEmpty,
}

/// The layout ids of a field's parts (Compose's internal `TextFieldId`, …).
pub(crate) const TextFieldId: &str = "TextField";
pub(crate) const PlaceholderId: &str = "Hint";
pub(crate) const LabelId: &str = "Label";
pub(crate) const LeadingId: &str = "Leading";
pub(crate) const TrailingId: &str = "Trailing";
pub(crate) const PrefixId: &str = "Prefix";
pub(crate) const SuffixId: &str = "Suffix";
pub(crate) const SupportingId: &str = "Supporting";
pub(crate) const ContainerId: &str = "Container";

/// What the icons' boxes leave of a field's side padding (Compose's internal `textFieldHorizontalIconPadding`): half
/// of the minimum interactive size's room around a small icon button's icon (none where that size is off).
#[kompozable]
pub(crate) fn text_field_horizontal_icon_padding() -> Dp {
    let interactive_size_or_nan = LocalMinimumInteractiveComponentSize.current();
    let interactive_size = if interactive_size_or_nan.is_specified() { interactive_size_or_nan } else { Dp(0.0) };
    Dp(((interactive_size - SmallIconButtonTokens::IconSize) / 2.0).0.max(0.0))
}

/// Half the minimized label's height (Compose's internal `minimizedLabelHalfHeight`): half the theme's body small
/// line height (the token's where it isn't in sp).
#[kompozable]
pub(crate) fn minimized_label_half_height() -> Dp {
    let composition_local_value = MaterialTheme::typography().body_small.line_height;
    let fallback_value = TypeScaleTokens::BodySmallLineHeight;
    let value = if composition_local_value.is_sp() { composition_local_value } else { fallback_value };
    LocalDensity.current().text_to_dp(value) / 2.0
}

/// Compose's `androidx.compose.ui.util.lerp(Int, Int, Float)`: rounded half up.
pub(crate) fn lerp_int(start: i32, stop: i32, fraction: f32) -> i32 {
    start + ((stop - start) as f64 * fraction as f64 + 0.5).floor() as i32
}

/// The labelled min height of the label's box as the label minimizes (Compose's `lerp(MinTextLineHeight,
/// MinFocusedLabelLineHeight, labelProgress())`).
pub(crate) fn label_min_height(progress: f32) -> Dp {
    lerp_dp(MinTextLineHeight, MinFocusedLabelLineHeight, progress)
}

pub(crate) const TextFieldPadding: Dp = Dp(16.0);
pub(crate) const AboveLabelHorizontalPadding: Dp = Dp(4.0);
pub(crate) const AboveLabelBottomPadding: Dp = Dp(4.0);
pub(crate) const SupportingTopPadding: Dp = Dp(4.0);
pub(crate) const PrefixSuffixTextPadding: Dp = Dp(2.0);
pub(crate) const MinTextLineHeight: Dp = Dp(24.0);
pub(crate) const MinFocusedLabelLineHeight: Dp = Dp(16.0);
pub(crate) const MinSupportingTextLineHeight: Dp = Dp(16.0);
