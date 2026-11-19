//! How content comes in and goes (Compose's EnterExitTransition.kt): [`EnterTransition`]s ([`fade_in`],
//! [`slide_in`] and its horizontal and vertical forms, [`scale_in`], [`expand_in`] and its forms, [`unveil_in`]) and
//! [`ExitTransition`]s ([`fade_out`], [`slide_out`], [`scale_out`], [`shrink_out`], [`veil_out`]), combined with
//! `+`; the [`EnterExitState`] a content's own transition goes through; and the modifier that animates a content
//! along it (Compose's internal `createModifier` and EnterExitTransitionModifierNode: its size and offset in its
//! layout, laid out as it shrinks or grows — the lookahead size where there is one — its alpha, scale and transform
//! origin in a graphics layer, its veil drawn over it).

use crate::converter::{TwoWayConverter, VectorConverter, VisibilityThreshold};
use crate::deferred_enter_exit::{ModifierLocalSharedMutableTransformState, SharedMutableTransformState};
use crate::spec::{FiniteAnimationSpec, SpringSpec};
use crate::spring_simulation::Spring;
use crate::transition::{DeferredAnimation, Segment, Transition};
use crate::vector::{AnimationVector2D, AnimationVector4D};
use crate::{AnimationValue, ColorVectorConverter};
use kompoz_graphics::colorspace::{ColorSpace, ColorSpaces};
use kompoz_graphics::{Color, IntOffset, IntSize, LayoutDirection, Offset, Size};
use kompoz_runtime::{group, named, remember_keyed};
use kompoz_ui::draw::{ContentDrawScope, DrawModifierNode};
use kompoz_ui::layout::{
    GraphicsLayerScope, IntrinsicMeasurable, IntrinsicMeasureScope, LayerBlock, LayoutAwareModifierNode,
    LayoutCoordinates, LayoutModifierNode, Measurable, MeasureResult, MeasureScope, PlacementScope, TransformOrigin,
};
use kompoz_ui::modifier::{ModifierLocalMap, ModifierLocalModifierNode};
use kompoz_ui::{Alignment, Constraints, HorizontalAlignment, Modifier, ModifierNode, ModifierNodeElement, NodeCx};
use kompoz_ui::{NodeHandle, VerticalAlignment, node_capabilities};
use std::cell::{Cell, RefCell};
use std::fmt;
use std::ops::Add;
use std::rc::Rc;
use std::sync::Arc;

// ---- The states ---------------------------------------------------------------------------------------------------

/// Where a content is in its coming in and going (Compose's EnterExitState): before it came in (`PreEnter`), shown
/// (`Visible`), gone (`PostExit`). An `AnimatedVisibility`'s content goes PreEnter → Visible as it comes in and
/// Visible → PostExit as it goes; its [`AnimatedVisibilityScope`](crate::AnimatedVisibilityScope)'s transition is of
/// these states.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum EnterExitState {
    PreEnter,
    Visible,
    PostExit,
}

// ---- What the transitions are made of -------------------------------------------------------------------------

/// A fade (Compose's internal Fade): the alpha it is at when not visible.
#[derive(Clone)]
pub(crate) struct Fade {
    pub(crate) alpha: f32,
    pub(crate) animation_spec: Arc<dyn FiniteAnimationSpec<f32>>,
}

impl PartialEq for Fade {
    fn eq(&self, other: &Self) -> bool {
        self.alpha == other.alpha && *self.animation_spec == *other.animation_spec
    }
}

/// A slide (Compose's internal Slide): the offset it is at when not visible, of the content's full size.
#[derive(Clone)]
pub(crate) struct Slide {
    pub(crate) slide_offset: Rc<dyn Fn(IntSize) -> IntOffset>,
    pub(crate) animation_spec: Arc<dyn FiniteAnimationSpec<IntOffset>>,
}

impl PartialEq for Slide {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.slide_offset, &other.slide_offset) && *self.animation_spec == *other.animation_spec
    }
}

/// A change of size (Compose's internal ChangeSize): the size it is at when not visible, of the content's full size,
/// where the content is held in the size along the way, whether it is clipped to it.
#[derive(Clone)]
pub(crate) struct ChangeSize {
    pub(crate) alignment: Alignment,
    pub(crate) size: Rc<dyn Fn(IntSize) -> IntSize>,
    pub(crate) animation_spec: Arc<dyn FiniteAnimationSpec<IntSize>>,
    pub(crate) clip: bool,
}

impl PartialEq for ChangeSize {
    fn eq(&self, other: &Self) -> bool {
        self.alignment == other.alignment
            && Rc::ptr_eq(&self.size, &other.size)
            && *self.animation_spec == *other.animation_spec
            && self.clip == other.clip
    }
}

/// A scale (Compose's internal Scale): the scale it is at when not visible, about the transform origin.
#[derive(Clone)]
pub(crate) struct Scale {
    pub(crate) scale: f32,
    pub(crate) transform_origin: TransformOrigin,
    pub(crate) animation_spec: Arc<dyn FiniteAnimationSpec<f32>>,
}

impl PartialEq for Scale {
    fn eq(&self, other: &Self) -> bool {
        self.scale == other.scale
            && self.transform_origin == other.transform_origin
            && *self.animation_spec == *other.animation_spec
    }
}

/// A veil (Compose's internal Veil): the colors it goes between, whether it covers the content's parent.
#[derive(Clone)]
pub(crate) struct Veil {
    pub(crate) initial_color: Color,
    pub(crate) target_color: Color,
    pub(crate) animation_spec: Arc<dyn FiniteAnimationSpec<Color>>,
    pub(crate) match_parent_size: bool,
}

impl PartialEq for Veil {
    fn eq(&self, other: &Self) -> bool {
        self.initial_color == other.initial_color
            && self.target_color == other.target_color
            && *self.animation_spec == *other.animation_spec
            && self.match_parent_size == other.match_parent_size
    }
}

/// What a transition is made of (Compose's internal TransitionData): each part or none, whether an exit holds the
/// content until the transition is done. (Its `effectsMap`, of internal TransitionEffects that nothing in Compose
/// 1.12.1 gives or reads, isn't ported.)
#[derive(Clone, PartialEq)]
pub(crate) struct TransitionData {
    pub(crate) fade: Option<Fade>,
    pub(crate) slide: Option<Slide>,
    pub(crate) change_size: Option<ChangeSize>,
    pub(crate) scale: Option<Scale>,
    pub(crate) veil: Option<Veil>,
    pub(crate) hold: bool,
}

impl TransitionData {
    const EMPTY: TransitionData =
        TransitionData { fade: None, slide: None, change_size: None, scale: None, veil: None, hold: false };

    /// `self` with `other`'s parts over its own (Compose's `plus`).
    fn plus(&self, other: &TransitionData, hold: bool) -> TransitionData {
        TransitionData {
            fade: other.fade.clone().or_else(|| self.fade.clone()),
            slide: other.slide.clone().or_else(|| self.slide.clone()),
            change_size: other.change_size.clone().or_else(|| self.change_size.clone()),
            scale: other.scale.clone().or_else(|| self.scale.clone()),
            veil: other.veil.clone().or_else(|| self.veil.clone()),
            hold,
        }
    }

    fn describe(&self, f: &mut fmt::Formatter, separator: &str) -> fmt::Result {
        let opt = |present: bool, s: String| if present { s } else { "null".to_string() };
        write!(
            f,
            "Fade - {}{separator} Slide - {}{separator} Shrink - {}{separator} Scale - {}{separator} Veil - {}",
            opt(
                self.fade.is_some(),
                self.fade.as_ref().map(|x| format!("Fade(alpha={:?})", x.alpha)).unwrap_or_default()
            ),
            opt(self.slide.is_some(), "Slide".to_string()),
            opt(
                self.change_size.is_some(),
                self.change_size
                    .as_ref()
                    .map(|c| format!("ChangeSize(alignment={:?}, clip={})", c.alignment, c.clip))
                    .unwrap_or_default()
            ),
            opt(
                self.scale.is_some(),
                self.scale
                    .as_ref()
                    .map(|s| format!("Scale(scale={:?}, transformOrigin={:?})", s.scale, s.transform_origin))
                    .unwrap_or_default()
            ),
            opt(
                self.veil.is_some(),
                self.veil
                    .as_ref()
                    .map(|v| format!("Veil(initialColor={:?}, targetColor={:?})", v.initial_color, v.target_color))
                    .unwrap_or_default()
            ),
        )
    }
}

// ---- EnterTransition / ExitTransition -----------------------------------------------------------------------------

/// How content comes in (Compose's EnterTransition): any of a fade, a slide, an expansion, a scale and an unveiling,
/// combined with `+` (the right side's parts over the left's). [`EnterTransition::None`]: shown at once. Equal when
/// their parts are (lambdas: the same ones).
#[derive(Clone, PartialEq)]
pub struct EnterTransition {
    pub(crate) data: TransitionData,
}

/// How content goes (Compose's ExitTransition): any of a fade, a slide, a shrinking, a scale and a veiling, combined
/// with `+`. [`ExitTransition::None`]: gone at once; [`ExitTransition::KeepUntilTransitionsFinished`]: kept, as it
/// is, until the other content's transition is done (AnimatedContent).
#[derive(Clone, PartialEq)]
pub struct ExitTransition {
    pub(crate) data: TransitionData,
}

impl EnterTransition {
    /// No transition: shown at once (Compose's `EnterTransition.None`).
    pub const None: EnterTransition = EnterTransition { data: TransitionData::EMPTY };

    /// `self` with `enter`'s parts over its own (Compose's `plus`, Rust's `+`).
    pub fn plus(&self, enter: &EnterTransition) -> EnterTransition {
        EnterTransition { data: self.data.plus(&enter.data, false) }
    }

    /// The content transform of this enter and `exit` (Compose's infix `togetherWith`): for AnimatedContent, this
    /// for the content coming in, `exit` for the one leaving.
    pub fn together_with(&self, exit: ExitTransition) -> crate::ContentTransform {
        crate::ContentTransform::new(self.clone(), exit)
    }

    /// Compose's deprecated infix `with`: [`together_with`](Self::together_with).
    ///
    /// Kotlin: `with(exit: ExitTransition)`
    pub fn with(&self, exit: ExitTransition) -> crate::ContentTransform {
        crate::ContentTransform::new(self.clone(), exit)
    }
}

impl ExitTransition {
    /// No transition: gone at once (Compose's `ExitTransition.None`).
    pub const None: ExitTransition = ExitTransition { data: TransitionData::EMPTY };

    /// Kept as it is until the other transitions of an AnimatedContent are done (Compose's
    /// `ExitTransition.KeepUntilTransitionsFinished`, reached through AnimatedContentTransitionScope; it is in
    /// `ExitTransition` itself here).
    pub const KeepUntilTransitionsFinished: ExitTransition = ExitTransition {
        data: TransitionData { fade: None, slide: None, change_size: None, scale: None, veil: None, hold: true },
    };

    /// `self` with `exit`'s parts over its own (Compose's `plus`, Rust's `+`): held if either holds.
    pub fn plus(&self, exit: &ExitTransition) -> ExitTransition {
        ExitTransition { data: self.data.plus(&exit.data, exit.data.hold || self.data.hold) }
    }
}

impl Add for EnterTransition {
    type Output = EnterTransition;
    fn add(self, enter: EnterTransition) -> EnterTransition {
        self.plus(&enter)
    }
}

impl Add for ExitTransition {
    type Output = ExitTransition;
    fn add(self, exit: ExitTransition) -> ExitTransition {
        self.plus(&exit)
    }
}

impl fmt::Display for EnterTransition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if *self == EnterTransition::None {
            return f.write_str("EnterTransition.None");
        }
        f.write_str("EnterTransition: ")?;
        self.data.describe(f, ",")
    }
}

impl fmt::Debug for EnterTransition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl fmt::Display for ExitTransition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if *self == ExitTransition::None {
            return f.write_str("ExitTransition.None");
        }
        if *self == ExitTransition::KeepUntilTransitionsFinished {
            return f.write_str("ExitTransition.KeepUntilTransitionsFinished");
        }
        f.write_str("ExitTransition:  ")?;
        self.data.describe(f, ", ")?;
        write!(f, ",  KeepUntilTransitionsFinished - {}", self.data.hold)
    }
}

impl fmt::Debug for ExitTransition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

// ---- Default specs ------------------------------------------------------------------------------------------------

/// `spring(stiffness = Spring.StiffnessMediumLow)` (Compose's DefaultAlphaAndScaleSpring and the specs' defaults).
pub(crate) fn medium_low_spring<T: AnimationValue>() -> SpringSpec<T> {
    SpringSpec::new(Spring::DampingRatioNoBouncy, Spring::StiffnessMediumLow, None)
}

/// `spring(stiffness = Spring.StiffnessMediumLow, visibilityThreshold = T.VisibilityThreshold)`.
pub(crate) fn medium_low_threshold_spring<T: AnimationValue + VisibilityThreshold>() -> SpringSpec<T> {
    SpringSpec::new(Spring::DampingRatioNoBouncy, Spring::StiffnessMediumLow, Some(T::VisibilityThreshold))
}

fn default_float_spec() -> Arc<dyn FiniteAnimationSpec<f32>> {
    Arc::new(medium_low_spring::<f32>())
}

fn default_offset_spec() -> Arc<dyn FiniteAnimationSpec<IntOffset>> {
    Arc::new(medium_low_threshold_spring::<IntOffset>())
}

fn default_size_spec() -> Arc<dyn FiniteAnimationSpec<IntSize>> {
    Arc::new(medium_low_threshold_spring::<IntSize>())
}

fn default_color_spec() -> Arc<dyn FiniteAnimationSpec<Color>> {
    Arc::new(medium_low_spring::<Color>())
}

/// Black at half alpha (the veil's default color).
fn half_black() -> Color {
    let c = Color::Black;
    Color::new_in(c.red(), c.green(), c.blue(), 0.5, c.color_space())
}

/// `color` at alpha 0 (Compose's `copy(alpha = 0f)`).
fn transparent_of(color: Color) -> Color {
    Color::new_in(color.red(), color.green(), color.blue(), 0.0, color.color_space())
}

// ---- The transitions ----------------------------------------------------------------------------------------------

/// Fades in from `initial_alpha` (Compose's `fadeIn`) along `animation_spec`.
///
/// Kotlin: `fadeIn(animationSpec: FiniteAnimationSpec<Float> = spring(stiffness = Spring.StiffnessMediumLow), initialAlpha: Float = 0f)`
#[named]
pub fn fade_in(
    #[default(default_float_spec())] animation_spec: Arc<dyn FiniteAnimationSpec<f32>>,
    #[default(0.0)] initial_alpha: f32,
) -> EnterTransition {
    EnterTransition {
        data: TransitionData { fade: Some(Fade { alpha: initial_alpha, animation_spec }), ..TransitionData::EMPTY },
    }
}

/// Fades out to `target_alpha` (Compose's `fadeOut`).
///
/// Kotlin: `fadeOut(animationSpec: FiniteAnimationSpec<Float> = spring(stiffness = Spring.StiffnessMediumLow), targetAlpha: Float = 0f)`
#[named]
pub fn fade_out(
    #[default(default_float_spec())] animation_spec: Arc<dyn FiniteAnimationSpec<f32>>,
    #[default(0.0)] target_alpha: f32,
) -> ExitTransition {
    ExitTransition {
        data: TransitionData { fade: Some(Fade { alpha: target_alpha, animation_spec }), ..TransitionData::EMPTY },
    }
}

/// Slides in from the offset `initial_offset` gives of the content's full size (Compose's `slideIn`).
///
/// Kotlin: `slideIn(animationSpec: FiniteAnimationSpec<IntOffset> = spring(stiffness = Spring.StiffnessMediumLow, visibilityThreshold = IntOffset.VisibilityThreshold), initialOffset: (fullSize: IntSize) -> IntOffset)`
#[named]
pub fn slide_in(
    #[default(default_offset_spec())] animation_spec: Arc<dyn FiniteAnimationSpec<IntOffset>>,
    initial_offset: impl Fn(IntSize) -> IntOffset + 'static,
) -> EnterTransition {
    EnterTransition {
        data: TransitionData {
            slide: Some(Slide { slide_offset: Rc::new(initial_offset), animation_spec }),
            ..TransitionData::EMPTY
        },
    }
}

/// Slides out to the offset `target_offset` gives of the content's full size (Compose's `slideOut`).
///
/// Kotlin: `slideOut(animationSpec: FiniteAnimationSpec<IntOffset> = spring(stiffness = Spring.StiffnessMediumLow, visibilityThreshold = IntOffset.VisibilityThreshold), targetOffset: (fullSize: IntSize) -> IntOffset)`
#[named]
pub fn slide_out(
    #[default(default_offset_spec())] animation_spec: Arc<dyn FiniteAnimationSpec<IntOffset>>,
    target_offset: impl Fn(IntSize) -> IntOffset + 'static,
) -> ExitTransition {
    ExitTransition {
        data: TransitionData {
            slide: Some(Slide { slide_offset: Rc::new(target_offset), animation_spec }),
            ..TransitionData::EMPTY
        },
    }
}

/// Scales in from `initial_scale` about `transform_origin` (Compose's `scaleIn`): a graphics layer's scale, the
/// layout's size as it is.
///
/// Kotlin: `scaleIn(animationSpec: FiniteAnimationSpec<Float> = spring(stiffness = Spring.StiffnessMediumLow), initialScale: Float = 0f, transformOrigin: TransformOrigin = TransformOrigin.Center)`
#[named]
pub fn scale_in(
    #[default(default_float_spec())] animation_spec: Arc<dyn FiniteAnimationSpec<f32>>,
    #[default(0.0)] initial_scale: f32,
    #[default(TransformOrigin::Center)] transform_origin: TransformOrigin,
) -> EnterTransition {
    EnterTransition {
        data: TransitionData {
            scale: Some(Scale { scale: initial_scale, transform_origin, animation_spec }),
            ..TransitionData::EMPTY
        },
    }
}

/// Scales out to `target_scale` about `transform_origin` (Compose's `scaleOut`).
///
/// Kotlin: `scaleOut(animationSpec: FiniteAnimationSpec<Float> = spring(stiffness = Spring.StiffnessMediumLow), targetScale: Float = 0f, transformOrigin: TransformOrigin = TransformOrigin.Center)`
#[named]
pub fn scale_out(
    #[default(default_float_spec())] animation_spec: Arc<dyn FiniteAnimationSpec<f32>>,
    #[default(0.0)] target_scale: f32,
    #[default(TransformOrigin::Center)] transform_origin: TransformOrigin,
) -> ExitTransition {
    ExitTransition {
        data: TransitionData {
            scale: Some(Scale { scale: target_scale, transform_origin, animation_spec }),
            ..TransitionData::EMPTY
        },
    }
}

/// Unveils: a veil of `initial_color` drawn over it fades to transparent (Compose's experimental `unveilIn`);
/// with `match_parent_size`, the veil covers the content's parent.
///
/// Kotlin: `unveilIn(animationSpec: FiniteAnimationSpec<Color> = spring(stiffness = Spring.StiffnessMediumLow), initialColor: Color = Color.Black.copy(alpha = 0.5f), matchParentSize: Boolean = false)`
#[named]
pub fn unveil_in(
    #[default(default_color_spec())] animation_spec: Arc<dyn FiniteAnimationSpec<Color>>,
    #[default(half_black())] initial_color: Color,
    #[default(false)] match_parent_size: bool,
) -> EnterTransition {
    EnterTransition {
        data: TransitionData {
            veil: Some(Veil {
                initial_color,
                target_color: transparent_of(initial_color),
                animation_spec,
                match_parent_size,
            }),
            ..TransitionData::EMPTY
        },
    }
}

/// Veils: a veil drawn over it goes from transparent to `target_color` (Compose's experimental `veilOut`).
///
/// Kotlin: `veilOut(animationSpec: FiniteAnimationSpec<Color> = spring(stiffness = Spring.StiffnessMediumLow), targetColor: Color = Color.Black.copy(alpha = 0.5f), matchParentSize: Boolean = false)`
#[named]
pub fn veil_out(
    #[default(default_color_spec())] animation_spec: Arc<dyn FiniteAnimationSpec<Color>>,
    #[default(half_black())] target_color: Color,
    #[default(false)] match_parent_size: bool,
) -> ExitTransition {
    ExitTransition {
        data: TransitionData {
            veil: Some(Veil {
                initial_color: transparent_of(target_color),
                target_color,
                animation_spec,
                match_parent_size,
            }),
            ..TransitionData::EMPTY
        },
    }
}

/// Expands from the size `initial_size` gives of the full size (none by default) to the full size (Compose's
/// `expandIn`): its layout grows so, the content held at `expand_from` in it, clipped to it unless not `clip`.
///
/// Kotlin: `expandIn(animationSpec: FiniteAnimationSpec<IntSize> = spring(stiffness = Spring.StiffnessMediumLow, visibilityThreshold = IntSize.VisibilityThreshold), expandFrom: Alignment = Alignment.BottomEnd, clip: Boolean = true, initialSize: (fullSize: IntSize) -> IntSize = { IntSize(0, 0) })`
#[named]
pub fn expand_in(
    #[default(default_size_spec())] animation_spec: Arc<dyn FiniteAnimationSpec<IntSize>>,
    #[default(Alignment::BottomEnd)] expand_from: Alignment,
    #[default(true)] clip: bool,
    #[default(|_: IntSize| IntSize::new(0, 0))] initial_size: impl Fn(IntSize) -> IntSize + 'static,
) -> EnterTransition {
    expand_in_of(animation_spec, expand_from, clip, Rc::new(initial_size))
}

fn expand_in_of(
    animation_spec: Arc<dyn FiniteAnimationSpec<IntSize>>,
    expand_from: Alignment,
    clip: bool,
    initial_size: Rc<dyn Fn(IntSize) -> IntSize>,
) -> EnterTransition {
    EnterTransition {
        data: TransitionData {
            change_size: Some(ChangeSize { alignment: expand_from, size: initial_size, animation_spec, clip }),
            ..TransitionData::EMPTY
        },
    }
}

/// Shrinks from the full size to the size `target_size` gives of it (none by default; Compose's `shrinkOut`): its
/// layout shrinks so, the content held at `shrink_towards`, clipped unless not `clip`.
///
/// Kotlin: `shrinkOut(animationSpec: FiniteAnimationSpec<IntSize> = spring(stiffness = Spring.StiffnessMediumLow, visibilityThreshold = IntSize.VisibilityThreshold), shrinkTowards: Alignment = Alignment.BottomEnd, clip: Boolean = true, targetSize: (fullSize: IntSize) -> IntSize = { IntSize(0, 0) })`
#[named]
pub fn shrink_out(
    #[default(default_size_spec())] animation_spec: Arc<dyn FiniteAnimationSpec<IntSize>>,
    #[default(Alignment::BottomEnd)] shrink_towards: Alignment,
    #[default(true)] clip: bool,
    #[default(|_: IntSize| IntSize::new(0, 0))] target_size: impl Fn(IntSize) -> IntSize + 'static,
) -> ExitTransition {
    shrink_out_of(animation_spec, shrink_towards, clip, Rc::new(target_size))
}

fn shrink_out_of(
    animation_spec: Arc<dyn FiniteAnimationSpec<IntSize>>,
    shrink_towards: Alignment,
    clip: bool,
    target_size: Rc<dyn Fn(IntSize) -> IntSize>,
) -> ExitTransition {
    ExitTransition {
        data: TransitionData {
            change_size: Some(ChangeSize { alignment: shrink_towards, size: target_size, animation_spec, clip }),
            ..TransitionData::EMPTY
        },
    }
}

/// A horizontal alignment as a whole one (Compose's private `Alignment.Horizontal.toAlignment`).
fn horizontal_to_alignment(alignment: &HorizontalAlignment) -> Alignment {
    if *alignment == Alignment::Start {
        Alignment::CenterStart
    } else if *alignment == Alignment::End {
        Alignment::CenterEnd
    } else {
        Alignment::Center
    }
}

/// A vertical alignment as a whole one (Compose's private `Alignment.Vertical.toAlignment`).
fn vertical_to_alignment(alignment: &VerticalAlignment) -> Alignment {
    if *alignment == Alignment::Top {
        Alignment::TopCenter
    } else if *alignment == Alignment::Bottom {
        Alignment::BottomCenter
    } else {
        Alignment::Center
    }
}

/// Expands across from the width `initial_width` gives of the full width (Compose's `expandHorizontally`).
///
/// Kotlin: `expandHorizontally(animationSpec: FiniteAnimationSpec<IntSize> = spring(stiffness = Spring.StiffnessMediumLow, visibilityThreshold = IntSize.VisibilityThreshold), expandFrom: Alignment.Horizontal = Alignment.End, clip: Boolean = true, initialWidth: (fullWidth: Int) -> Int = { 0 })`
#[named]
pub fn expand_horizontally(
    #[default(default_size_spec())] animation_spec: Arc<dyn FiniteAnimationSpec<IntSize>>,
    #[default(Alignment::End)] expand_from: HorizontalAlignment,
    #[default(true)] clip: bool,
    #[default(|_: i32| 0)] initial_width: impl Fn(i32) -> i32 + 'static,
) -> EnterTransition {
    let size = move |it: IntSize| IntSize::new(initial_width(it.width), it.height);
    expand_in_of(animation_spec, horizontal_to_alignment(&expand_from), clip, Rc::new(size))
}

/// Expands down or up from the height `initial_height` gives of the full height (Compose's `expandVertically`).
///
/// Kotlin: `expandVertically(animationSpec: FiniteAnimationSpec<IntSize> = spring(stiffness = Spring.StiffnessMediumLow, visibilityThreshold = IntSize.VisibilityThreshold), expandFrom: Alignment.Vertical = Alignment.Bottom, clip: Boolean = true, initialHeight: (fullHeight: Int) -> Int = { 0 })`
#[named]
pub fn expand_vertically(
    #[default(default_size_spec())] animation_spec: Arc<dyn FiniteAnimationSpec<IntSize>>,
    #[default(Alignment::Bottom)] expand_from: VerticalAlignment,
    #[default(true)] clip: bool,
    #[default(|_: i32| 0)] initial_height: impl Fn(i32) -> i32 + 'static,
) -> EnterTransition {
    let size = move |it: IntSize| IntSize::new(it.width, initial_height(it.height));
    expand_in_of(animation_spec, vertical_to_alignment(&expand_from), clip, Rc::new(size))
}

/// Shrinks across to the width `target_width` gives of the full width (Compose's `shrinkHorizontally`).
///
/// Kotlin: `shrinkHorizontally(animationSpec: FiniteAnimationSpec<IntSize> = spring(stiffness = Spring.StiffnessMediumLow, visibilityThreshold = IntSize.VisibilityThreshold), shrinkTowards: Alignment.Horizontal = Alignment.End, clip: Boolean = true, targetWidth: (fullWidth: Int) -> Int = { 0 })`
#[named]
pub fn shrink_horizontally(
    #[default(default_size_spec())] animation_spec: Arc<dyn FiniteAnimationSpec<IntSize>>,
    #[default(Alignment::End)] shrink_towards: HorizontalAlignment,
    #[default(true)] clip: bool,
    #[default(|_: i32| 0)] target_width: impl Fn(i32) -> i32 + 'static,
) -> ExitTransition {
    let size = move |it: IntSize| IntSize::new(target_width(it.width), it.height);
    shrink_out_of(animation_spec, horizontal_to_alignment(&shrink_towards), clip, Rc::new(size))
}

/// Shrinks up or down to the height `target_height` gives of the full height (Compose's `shrinkVertically`).
///
/// Kotlin: `shrinkVertically(animationSpec: FiniteAnimationSpec<IntSize> = spring(stiffness = Spring.StiffnessMediumLow, visibilityThreshold = IntSize.VisibilityThreshold), shrinkTowards: Alignment.Vertical = Alignment.Bottom, clip: Boolean = true, targetHeight: (fullHeight: Int) -> Int = { 0 })`
#[named]
pub fn shrink_vertically(
    #[default(default_size_spec())] animation_spec: Arc<dyn FiniteAnimationSpec<IntSize>>,
    #[default(Alignment::Bottom)] shrink_towards: VerticalAlignment,
    #[default(true)] clip: bool,
    #[default(|_: i32| 0)] target_height: impl Fn(i32) -> i32 + 'static,
) -> ExitTransition {
    let size = move |it: IntSize| IntSize::new(it.width, target_height(it.height));
    shrink_out_of(animation_spec, vertical_to_alignment(&shrink_towards), clip, Rc::new(size))
}

/// Slides in across from the x offset `initial_offset_x` gives of the full width (half of it to the left by
/// default; Compose's `slideInHorizontally`).
///
/// Kotlin: `slideInHorizontally(animationSpec: FiniteAnimationSpec<IntOffset> = spring(stiffness = Spring.StiffnessMediumLow, visibilityThreshold = IntOffset.VisibilityThreshold), initialOffsetX: (fullWidth: Int) -> Int = { -it / 2 })`
#[named]
pub fn slide_in_horizontally(
    #[default(default_offset_spec())] animation_spec: Arc<dyn FiniteAnimationSpec<IntOffset>>,
    #[default(|it: i32| -it / 2)] initial_offset_x: impl Fn(i32) -> i32 + 'static,
) -> EnterTransition {
    slide_in_of(animation_spec, Rc::new(move |it: IntSize| IntOffset::new(initial_offset_x(it.width), 0)))
}

/// Slides in down or up from the y offset `initial_offset_y` gives of the full height (half of it up by default;
/// Compose's `slideInVertically`).
///
/// Kotlin: `slideInVertically(animationSpec: FiniteAnimationSpec<IntOffset> = spring(stiffness = Spring.StiffnessMediumLow, visibilityThreshold = IntOffset.VisibilityThreshold), initialOffsetY: (fullHeight: Int) -> Int = { -it / 2 })`
#[named]
pub fn slide_in_vertically(
    #[default(default_offset_spec())] animation_spec: Arc<dyn FiniteAnimationSpec<IntOffset>>,
    #[default(|it: i32| -it / 2)] initial_offset_y: impl Fn(i32) -> i32 + 'static,
) -> EnterTransition {
    slide_in_of(animation_spec, Rc::new(move |it: IntSize| IntOffset::new(0, initial_offset_y(it.height))))
}

/// Slides out across to the x offset `target_offset_x` gives of the full width (Compose's `slideOutHorizontally`).
///
/// Kotlin: `slideOutHorizontally(animationSpec: FiniteAnimationSpec<IntOffset> = spring(stiffness = Spring.StiffnessMediumLow, visibilityThreshold = IntOffset.VisibilityThreshold), targetOffsetX: (fullWidth: Int) -> Int = { -it / 2 })`
#[named]
pub fn slide_out_horizontally(
    #[default(default_offset_spec())] animation_spec: Arc<dyn FiniteAnimationSpec<IntOffset>>,
    #[default(|it: i32| -it / 2)] target_offset_x: impl Fn(i32) -> i32 + 'static,
) -> ExitTransition {
    slide_out_of(animation_spec, Rc::new(move |it: IntSize| IntOffset::new(target_offset_x(it.width), 0)))
}

/// Slides out down or up to the y offset `target_offset_y` gives of the full height (Compose's
/// `slideOutVertically`).
///
/// Kotlin: `slideOutVertically(animationSpec: FiniteAnimationSpec<IntOffset> = spring(stiffness = Spring.StiffnessMediumLow, visibilityThreshold = IntOffset.VisibilityThreshold), targetOffsetY: (fullHeight: Int) -> Int = { -it / 2 })`
#[named]
pub fn slide_out_vertically(
    #[default(default_offset_spec())] animation_spec: Arc<dyn FiniteAnimationSpec<IntOffset>>,
    #[default(|it: i32| -it / 2)] target_offset_y: impl Fn(i32) -> i32 + 'static,
) -> ExitTransition {
    slide_out_of(animation_spec, Rc::new(move |it: IntSize| IntOffset::new(0, target_offset_y(it.height))))
}

pub(crate) fn slide_in_of(
    animation_spec: Arc<dyn FiniteAnimationSpec<IntOffset>>,
    initial_offset: Rc<dyn Fn(IntSize) -> IntOffset>,
) -> EnterTransition {
    EnterTransition {
        data: TransitionData {
            slide: Some(Slide { slide_offset: initial_offset, animation_spec }),
            ..TransitionData::EMPTY
        },
    }
}

pub(crate) fn slide_out_of(
    animation_spec: Arc<dyn FiniteAnimationSpec<IntOffset>>,
    target_offset: Rc<dyn Fn(IntSize) -> IntOffset>,
) -> ExitTransition {
    ExitTransition {
        data: TransitionData {
            slide: Some(Slide { slide_offset: target_offset, animation_spec }),
            ..TransitionData::EMPTY
        },
    }
}

// ---- The modifier ---------------------------------------------------------------------------------------------------

/// Whether a content's transition is enabled (Compose's `isEnabled: () -> Boolean`): a shared element turns its
/// own off while it is in the overlay.
pub(crate) type IsEnabled = Rc<dyn Fn() -> bool>;

/// Enabled always (Compose's default `{ true }`).
pub(crate) fn always_enabled() -> IsEnabled {
    thread_local! {
        static ALWAYS: IsEnabled = Rc::new(|| true);
    }
    ALWAYS.with(|a| a.clone())
}

/// What the modifier of a content's enter/exit transition is made of (Compose's internal
/// `Transition<EnterExitState>.createModifier`): the deferred animations the transitions need (a slide, a size
/// change and its offset when the alignment changes on the way, a veil, the alpha, scale and transform origin of the
/// layer), set up as the content is laid out; the active enter and exit tracked (what an interrupted transition
/// still needs) unless the caller tracks them.
#[track_caller]
pub(crate) fn create_modifier(
    transition: &Transition<EnterExitState>,
    enter: EnterTransition,
    exit: ExitTransition,
    track_active_enter_exit: bool,
    is_enabled: IsEnabled,
    shared_mutable_transform_state: Option<SharedMutableTransformState>,
    label: &str,
) -> Modifier {
    let t = transition.clone();
    group(move || {
        let active_enter = if track_active_enter_exit { group(|| track_active_enter(&t, enter)) } else { enter };
        let active_exit = if track_active_enter_exit { group(|| track_active_exit(&t, exit)) } else { exit };
        let active_mutable_state = track_active_mutable_state(&t, shared_mutable_transform_state);
        let should_animate_veil = active_enter.data.veil.is_some()
            || active_exit.data.veil.is_some()
            || active_mutable_state.veil_requires_animation();
        let should_animate_slide = active_enter.data.slide.is_some()
            || active_exit.data.slide.is_some()
            || active_mutable_state.slide_requires_animation();
        let should_animate_size_change =
            active_enter.data.change_size.is_some() || active_exit.data.change_size.is_some();

        let slide_animation = if should_animate_slide {
            Some(deferred_animation(&t, IntOffset::VectorConverter, format!("{label} slide")))
        } else {
            None
        };
        let size_animation = if should_animate_size_change {
            Some(deferred_animation(&t, IntSize::VectorConverter, format!("{label} shrink/expand")))
        } else {
            None
        };
        let offset_animation = if should_animate_size_change {
            Some(deferred_animation(&t, IntOffset::VectorConverter, format!("{label} InterruptionHandlingOffset")))
        } else {
            None
        };
        let disable_clip = active_enter.data.change_size.as_ref().is_some_and(|c| !c.clip)
            || active_exit.data.change_size.as_ref().is_some_and(|c| !c.clip)
            || !should_animate_size_change;
        let color_space: &'static dyn ColorSpace = [
            active_enter.data.veil.as_ref().map(|v| v.initial_color),
            active_enter.data.veil.as_ref().map(|v| v.target_color),
            active_exit.data.veil.as_ref().map(|v| v.initial_color),
            active_exit.data.veil.as_ref().map(|v| v.target_color),
        ]
        .into_iter()
        .flatten()
        .next()
        .map_or(&ColorSpaces::Srgb as &'static dyn ColorSpace, |c| c.color_space());

        let veil_modifier = if should_animate_veil {
            let veil_animation = deferred_animation(&t, Color::VectorConverter(color_space), format!("{label} veil"));
            Modifier.then_element(VeilModifierElement {
                transition: t.clone(),
                veil_animation,
                enter: active_enter.clone(),
                exit: active_exit.clone(),
                mutable_transform_state: active_mutable_state.clone(),
            })
        } else {
            Modifier
        };
        let should_veil_match_parent_size = active_enter
            .data
            .veil
            .as_ref()
            .map(|v| v.match_parent_size)
            .or_else(|| active_exit.data.veil.as_ref().map(|v| v.match_parent_size))
            .or_else(|| active_mutable_state.mutable_data().map(|d| d.veil_match_parent_size()))
            .unwrap_or(false);
        let graphics_layer_block =
            create_graphics_layer_block(&t, &active_enter, &active_exit, &active_mutable_state, label);

        let enabled = is_enabled.clone();
        let (inner_veil, outer_veil) =
            if should_veil_match_parent_size { (Modifier, veil_modifier) } else { (veil_modifier, Modifier) };
        Modifier
            .then_element(SharedMutableTransformStateProviderElement { state: active_mutable_state.clone() })
            .then(outer_veil)
            .graphics_layer_block(move |layer| layer.clip = !disable_clip && enabled())
            .then_element(EnterExitTransitionElement {
                transition: t.clone(),
                size_animation,
                offset_animation,
                slide_animation,
                enter: active_enter,
                exit: active_exit,
                mutable_transform_state: active_mutable_state,
                is_enabled,
                graphics_layer_block,
            })
            .then(inner_veil)
    })
}

/// `transition.createDeferredAnimation(converter, label)`.
#[track_caller]
fn deferred_animation<T: AnimationValue, V: crate::AnimationVector>(
    transition: &Transition<EnterExitState>,
    converter: TwoWayConverter<T, V>,
    label: String,
) -> DeferredAnimation<EnterExitState, T, V> {
    kompoz_runtime::named_args!(transition.create_deferred_animation(converter, label = label))
}

/// Calls `effect` when a transition stops being in between (or a deferred phase ends without a new target), as
/// Compose's internal `Transition.DeferredTransitionCleanupEffect` does.
#[track_caller]
pub(crate) fn deferred_transition_cleanup_effect<S: AnimationValue>(transition: &Transition<S>, effect: impl Fn()) {
    let t = transition.clone();
    group(move || {
        let is_mutating = t.pending_target_state().is_some();
        let target = t.target_state();
        if t.current_state() == target && !is_mutating {
            effect();
        }
        let was_mutating = kompoz_runtime::remember(|| Rc::new(Cell::new(is_mutating)));
        let last_target = kompoz_runtime::remember(|| Rc::new(RefCell::new(None::<S>)));
        if last_target.borrow().as_ref() != Some(&target) {
            if !is_mutating && !was_mutating.get() {
                effect();
            }
            *last_target.borrow_mut() = Some(target);
        }
        was_mutating.set(is_mutating);
    })
}

/// The shared mutable transform state given, or one of the transition's (Compose's internal
/// `trackActiveMutableState`): told whether the transition defers, cleared once it stops being in between.
#[track_caller]
fn track_active_mutable_state(
    transition: &Transition<EnterExitState>,
    shared: Option<SharedMutableTransformState>,
) -> SharedMutableTransformState {
    let t = transition.clone();
    group(move || {
        let shared = match shared {
            Some(s) => s,
            None => group(|| remember_keyed(t.clone(), SharedMutableTransformState::new)),
        };
        shared.set_is_mutating(t.pending_target_state().is_some());
        let s = shared.clone();
        deferred_transition_cleanup_effect(&t, move || s.clear());
        shared
    })
}

/// The enter transition in effect (Compose's internal `trackActiveEnter`): what an interrupted transition was
/// coming in by is kept, the new one added over it; none once shown (unless seeking).
#[track_caller]
pub(crate) fn track_active_enter(transition: &Transition<EnterExitState>, enter: EnterTransition) -> EnterTransition {
    let t = transition.clone();
    group(move || {
        let initial = enter.clone();
        let active = remember_keyed(t.clone(), move || Rc::new(RefCell::new(initial)));
        let (current, target) = (t.current_state(), t.target_state());
        if current == target && current == EnterExitState::Visible {
            *active.borrow_mut() = if t.is_seeking() { enter } else { EnterTransition::None };
        } else if target != EnterExitState::PostExit {
            let sum = active.borrow().plus(&enter);
            *active.borrow_mut() = sum;
        }
        active.borrow().clone()
    })
}

/// The exit transition in effect (Compose's internal `trackActiveExit`): when it goes again after coming back, the
/// earlier exit's parts first animate back to rest, the new exit over them.
#[track_caller]
pub(crate) fn track_active_exit(transition: &Transition<EnterExitState>, exit: ExitTransition) -> ExitTransition {
    let t = transition.clone();
    group(move || {
        let initial = exit.clone();
        let active = remember_keyed(t.clone(), move || Rc::new(RefCell::new(initial)));
        let (current, target) = (t.current_state(), t.target_state());
        if current == target && current == EnterExitState::Visible {
            *active.borrow_mut() = if t.is_seeking() { exit } else { ExitTransition::None };
        } else if target != EnterExitState::Visible {
            let neutral = {
                let a = active.borrow();
                TransitionData {
                    fade: a.data.fade.as_ref().map(|f| Fade { alpha: 1.0, ..f.clone() }),
                    scale: a.data.scale.as_ref().map(|s| Scale { scale: 1.0, ..s.clone() }),
                    slide: a
                        .data
                        .slide
                        .as_ref()
                        .map(|s| Slide { slide_offset: Rc::new(|_| IntOffset::Zero), ..s.clone() }),
                    change_size: a
                        .data
                        .change_size
                        .as_ref()
                        .map(|c| ChangeSize { size: Rc::new(|it| it), ..c.clone() }),
                    veil: a.data.veil.as_ref().map(|v| Veil { target_color: v.initial_color, ..v.clone() }),
                    ..TransitionData::EMPTY
                }
            };
            *active.borrow_mut() = ExitTransition { data: neutral }.plus(&exit);
        }
        active.borrow().clone()
    })
}

/// What sets up the layer of a content's alpha, scale and transform origin (Compose's internal
/// GraphicsLayerBlockForEnterExit): its `init` sets their deferred animations up and gives the layer's block.
pub(crate) type GraphicsLayerBlockForEnterExit = Rc<dyn Fn() -> LayerBlock>;

/// The two endpoints' specs of a float animation of the layer (Compose's alpha and scale `transitionSpec`s).
fn layer_spec(
    segment: &Segment<EnterExitState>,
    enter: Option<Arc<dyn FiniteAnimationSpec<f32>>>,
    exit: Option<Arc<dyn FiniteAnimationSpec<f32>>>,
) -> Arc<dyn FiniteAnimationSpec<f32>> {
    if segment.is_transitioning_to(EnterExitState::PreEnter, EnterExitState::Visible) {
        enter.unwrap_or_else(default_float_spec)
    } else if segment.is_transitioning_to(EnterExitState::Visible, EnterExitState::PostExit) {
        exit.unwrap_or_else(default_float_spec)
    } else {
        default_float_spec()
    }
}

/// Compose's private TransformOriginVectorConverter.
fn transform_origin_vector_converter() -> TwoWayConverter<TransformOrigin, AnimationVector2D> {
    TwoWayConverter::new(
        |t: TransformOrigin| AnimationVector2D(t.pivot_fraction_x, t.pivot_fraction_y),
        |v: AnimationVector2D| TransformOrigin::new(v.v1, v.v2),
    )
}

/// Compose's private `createGraphicsLayerBlock`.
#[track_caller]
fn create_graphics_layer_block(
    transition: &Transition<EnterExitState>,
    enter: &EnterTransition,
    exit: &ExitTransition,
    mutable_transform_state: &SharedMutableTransformState,
    label: &str,
) -> GraphicsLayerBlockForEnterExit {
    let t = transition.clone();
    group(move || {
        let should_animate_alpha =
            enter.data.fade.is_some() || exit.data.fade.is_some() || mutable_transform_state.alpha_requires_animation();
        let should_animate_scale = enter.data.scale.is_some()
            || exit.data.scale.is_some()
            || mutable_transform_state.scale_requires_animation();
        let alpha_animation = if should_animate_alpha {
            Some(deferred_animation(&t, f32::VectorConverter, format!("{label} alpha")))
        } else {
            None
        };
        let scale_animation = if should_animate_scale {
            Some(deferred_animation(&t, f32::VectorConverter, format!("{label} scale")))
        } else {
            None
        };
        let transform_origin_animation = if should_animate_scale {
            Some(deferred_animation(
                &t,
                transform_origin_vector_converter(),
                "TransformOriginInterruptionHandling".to_string(),
            ))
        } else {
            None
        };
        let (enter, exit, mts) = (enter.clone(), exit.clone(), mutable_transform_state.clone());
        Rc::new(move || -> LayerBlock {
            let alpha = alpha_animation.as_ref().map(|a| {
                let (fe, fx) = (enter.data.fade.clone(), exit.data.fade.clone());
                let (te, tx, m) = (enter.clone(), exit.clone(), mts.clone());
                a.animate_with_forced_initial(
                    move |s| {
                        layer_spec(
                            s,
                            fe.as_ref().map(|f| f.animation_spec.clone()),
                            fx.as_ref().map(|f| f.animation_spec.clone()),
                        )
                    },
                    mts.alpha_handoff_value(),
                    None,
                    move |state| match state {
                        EnterExitState::Visible => 1.0,
                        EnterExitState::PreEnter => te.data.fade.as_ref().map_or(1.0, |f| f.alpha),
                        EnterExitState::PostExit => tx.data.fade.as_ref().map_or_else(|| m.last_alpha(), |f| f.alpha),
                    },
                )
            });
            let scale = scale_animation.as_ref().map(|a| {
                let (se, sx) = (enter.data.scale.clone(), exit.data.scale.clone());
                let (te, tx, m) = (enter.clone(), exit.clone(), mts.clone());
                a.animate_with_forced_initial(
                    move |s| {
                        layer_spec(
                            s,
                            se.as_ref().map(|x| x.animation_spec.clone()),
                            sx.as_ref().map(|x| x.animation_spec.clone()),
                        )
                    },
                    mts.scale_handoff_value(),
                    mts.scale_handoff_velocity(),
                    move |state| match state {
                        EnterExitState::Visible => 1.0,
                        EnterExitState::PreEnter => te.data.scale.as_ref().map_or(1.0, |s| s.scale),
                        EnterExitState::PostExit => tx.data.scale.as_ref().map_or_else(|| m.last_scale(), |s| s.scale),
                    },
                )
            });
            let enter_origin = enter.data.scale.as_ref().map(|s| s.transform_origin);
            let exit_origin = exit.data.scale.as_ref().map(|s| s.transform_origin);
            let origin_when_visible = if t.current_state() == EnterExitState::PreEnter {
                enter_origin.or(exit_origin)
            } else {
                exit_origin.or(enter_origin)
            };
            let transform_origin = transform_origin_animation.as_ref().map(|a| {
                let m = mts.clone();
                a.animate_with_forced_initial(
                    |_| Arc::new(crate::spring::<TransformOrigin>()) as Arc<dyn FiniteAnimationSpec<TransformOrigin>>,
                    mts.transform_origin_handoff_value(),
                    None,
                    move |state| {
                        match state {
                            EnterExitState::Visible => origin_when_visible,
                            EnterExitState::PreEnter => enter_origin.or(exit_origin),
                            EnterExitState::PostExit => exit_origin.or_else(|| Some(m.last_transform_origin())),
                        }
                        .unwrap_or(TransformOrigin::Center)
                    },
                )
            });
            let m = mts.clone();
            Rc::new(move |layer: &mut GraphicsLayerScope| {
                layer.alpha = m.combined_alpha(alpha.as_ref().map_or(1.0, |a| a.value()));
                let combined_scale = m.combined_scale(scale.as_ref().map_or(1.0, |s| s.value()));
                layer.scale_x = combined_scale;
                layer.scale_y = combined_scale;
                layer.transform_origin = m.combined_transform_origin(
                    transform_origin.as_ref().map_or(TransformOrigin::Center, |o| o.value()),
                );
            })
        })
    })
}

// ---- Nodes ----------------------------------------------------------------------------------------------------------

/// Provides a content's [`SharedMutableTransformState`] to what is inside (Compose's `modifierLocalProvider`).
struct SharedMutableTransformStateProviderElement {
    state: SharedMutableTransformState,
}

impl PartialEq for SharedMutableTransformStateProviderElement {
    fn eq(&self, other: &Self) -> bool {
        self.state == other.state
    }
}

impl ModifierNodeElement for SharedMutableTransformStateProviderElement {
    type Node = SharedMutableTransformStateProviderNode;
    fn create(&self) -> SharedMutableTransformStateProviderNode {
        SharedMutableTransformStateProviderNode {
            values: ModifierLocalMap::of(&ModifierLocalSharedMutableTransformState, Some(self.state.clone())),
        }
    }
    fn update(&self, node: &mut SharedMutableTransformStateProviderNode) {
        node.values = ModifierLocalMap::of(&ModifierLocalSharedMutableTransformState, Some(self.state.clone()));
    }
    fn name(&self) -> &'static str {
        "modifierLocalProvider"
    }
}

struct SharedMutableTransformStateProviderNode {
    values: ModifierLocalMap,
}

impl ModifierNode for SharedMutableTransformStateProviderNode {
    node_capabilities!(modifier_local);
}

impl ModifierLocalModifierNode for SharedMutableTransformStateProviderNode {
    fn provided_values(&self) -> &ModifierLocalMap {
        &self.values
    }
}

/// Compose's private EnterExitTransitionElement.
struct EnterExitTransitionElement {
    transition: Transition<EnterExitState>,
    size_animation: Option<DeferredAnimation<EnterExitState, IntSize, AnimationVector2D>>,
    offset_animation: Option<DeferredAnimation<EnterExitState, IntOffset, AnimationVector2D>>,
    slide_animation: Option<DeferredAnimation<EnterExitState, IntOffset, AnimationVector2D>>,
    enter: EnterTransition,
    exit: ExitTransition,
    mutable_transform_state: SharedMutableTransformState,
    is_enabled: IsEnabled,
    graphics_layer_block: GraphicsLayerBlockForEnterExit,
}

impl PartialEq for EnterExitTransitionElement {
    fn eq(&self, other: &Self) -> bool {
        self.transition == other.transition
            && self.size_animation == other.size_animation
            && self.offset_animation == other.offset_animation
            && self.slide_animation == other.slide_animation
            && self.enter == other.enter
            && self.exit == other.exit
            && self.mutable_transform_state == other.mutable_transform_state
            && Rc::ptr_eq(&self.is_enabled, &other.is_enabled)
            && Rc::ptr_eq(&self.graphics_layer_block, &other.graphics_layer_block)
    }
}

impl ModifierNodeElement for EnterExitTransitionElement {
    type Node = EnterExitTransitionModifierNode;
    fn create(&self) -> EnterExitTransitionModifierNode {
        EnterExitTransitionModifierNode {
            transition: self.transition.clone(),
            size_animation: self.size_animation.clone(),
            offset_animation: self.offset_animation.clone(),
            slide_animation: self.slide_animation.clone(),
            enter: self.enter.clone(),
            exit: self.exit.clone(),
            mutable_transform_state: self.mutable_transform_state.clone(),
            is_enabled: self.is_enabled.clone(),
            graphics_layer_block: self.graphics_layer_block.clone(),
            lookahead_size: None,
            lookahead_constraints: None,
            current_alignment: None,
        }
    }
    fn update(&self, node: &mut EnterExitTransitionModifierNode) {
        node.transition = self.transition.clone();
        node.size_animation = self.size_animation.clone();
        node.offset_animation = self.offset_animation.clone();
        node.slide_animation = self.slide_animation.clone();
        node.enter = self.enter.clone();
        node.exit = self.exit.clone();
        node.mutable_transform_state = self.mutable_transform_state.clone();
        node.is_enabled = self.is_enabled.clone();
        node.graphics_layer_block = self.graphics_layer_block.clone();
    }
    fn name(&self) -> &'static str {
        "enterExitTransition"
    }
}

/// Compose's private EnterExitTransitionModifierNode: in the lookahead pass it is the content's size (the target);
/// in the main pass it animates its size from and to the transitions' (toward the lookahead size when there is
/// one), the content held by the alignment in it and slid, through the layer the block sets up.
struct EnterExitTransitionModifierNode {
    transition: Transition<EnterExitState>,
    size_animation: Option<DeferredAnimation<EnterExitState, IntSize, AnimationVector2D>>,
    offset_animation: Option<DeferredAnimation<EnterExitState, IntOffset, AnimationVector2D>>,
    slide_animation: Option<DeferredAnimation<EnterExitState, IntOffset, AnimationVector2D>>,
    enter: EnterTransition,
    exit: ExitTransition,
    mutable_transform_state: SharedMutableTransformState,
    is_enabled: IsEnabled,
    graphics_layer_block: GraphicsLayerBlockForEnterExit,
    /// What it measured to in the lookahead pass (Compose's `lookaheadSize`, InvalidSize as none).
    lookahead_size: Option<IntSize>,
    /// The constraints of the lookahead pass (Compose's `lookaheadConstraints` with its `…Available`).
    lookahead_constraints: Option<Constraints>,
    /// The alignment the size change started with (Compose's `currentAlignment`).
    current_alignment: Option<Alignment>,
}

impl ModifierNode for EnterExitTransitionModifierNode {
    node_capabilities!(layout, layout_aware);

    fn on_attach(&mut self, _cx: &NodeCx) {
        self.lookahead_constraints = None;
        self.lookahead_size = None;
    }
}

impl LayoutAwareModifierNode for EnterExitTransitionModifierNode {
    fn on_placed(&mut self, coordinates: &LayoutCoordinates) {
        self.mutable_transform_state.set_parent_layout_coordinates(coordinates.clone());
    }
    fn observes_remeasure(&self) -> bool {
        false
    }
}

/// The alignment of a size change now (Compose's node's `alignment`).
fn change_size_alignment(
    segment: &Segment<EnterExitState>,
    enter: &EnterTransition,
    exit: &ExitTransition,
) -> Option<Alignment> {
    let (e, x) = (enter.data.change_size.as_ref(), exit.data.change_size.as_ref());
    if segment.is_transitioning_to(EnterExitState::PreEnter, EnterExitState::Visible) {
        e.or(x).map(|c| c.alignment.clone())
    } else {
        x.or(e).map(|c| c.alignment.clone())
    }
}

/// The size of a state (Compose's node's `sizeByState`).
fn size_by_state(state: EnterExitState, full_size: IntSize, enter: &EnterTransition, exit: &ExitTransition) -> IntSize {
    match state {
        EnterExitState::Visible => full_size,
        EnterExitState::PreEnter => enter.data.change_size.as_ref().map_or(full_size, |c| (c.size)(full_size)),
        EnterExitState::PostExit => exit.data.change_size.as_ref().map_or(full_size, |c| (c.size)(full_size)),
    }
}

impl LayoutModifierNode for EnterExitTransitionModifierNode {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let segment = self.transition.segment();
        let alignment = change_size_alignment(&segment, &self.enter, &self.exit);
        if self.transition.current_state() == self.transition.target_state() {
            self.current_alignment = None;
        } else if self.current_alignment.is_none() {
            self.current_alignment = Some(alignment.clone().unwrap_or(Alignment::TopStart));
        }
        if scope.is_looking_ahead() {
            let placeable = measurable.measure(constraints);
            let measured = IntSize::new(placeable.width(), placeable.height());
            self.lookahead_size = Some(measured);
            self.lookahead_constraints = Some(constraints);
            return scope.layout(measured.width, measured.height, move |p: &PlacementScope| p.place(&placeable, 0, 0));
        }
        if !(self.is_enabled)() {
            // (not enabled: no animation at all)
            let placeable = measurable.measure(constraints);
            let (w, h) = (placeable.width(), placeable.height());
            return scope.layout(w, h, move |p: &PlacementScope| p.place(&placeable, 0, 0));
        }
        let layer_block = (self.graphics_layer_block)();
        // (the parent's constraints: AnimatedContent measures what leaves with constraints of its own)
        let placeable = measurable.measure(constraints);
        let measured = IntSize::new(placeable.width(), placeable.height());
        let target = self.lookahead_size.unwrap_or(measured);
        let anim_size = self.size_animation.as_ref().map(|a| {
            let (enter, exit) = (self.enter.clone(), self.exit.clone());
            let (e, x) = (self.enter.clone(), self.exit.clone());
            a.animate(
                move |s| {
                    if s.is_transitioning_to(EnterExitState::PreEnter, EnterExitState::Visible) {
                        e.data.change_size.as_ref().map(|c| c.animation_spec.clone())
                    } else if s.is_transitioning_to(EnterExitState::Visible, EnterExitState::PostExit) {
                        x.data.change_size.as_ref().map(|c| c.animation_spec.clone())
                    } else {
                        None
                    }
                    .unwrap_or_else(default_size_spec)
                },
                move |state| size_by_state(*state, target, &enter, &exit),
            )
        });
        // (within the incoming constraints, so the content is positioned in the constrained rect by the alignment)
        let current_size = constraints.constrain_size(anim_size.as_ref().map_or(measured, |s| s.value()));
        let offset_delta = self.offset_animation.as_ref().map_or(IntOffset::Zero, |a| {
            let (exit, current, align) = (self.exit.clone(), self.current_alignment.clone(), alignment.clone());
            a.animate(
                |_| default_offset_spec(),
                move |state| target_offset_by_state(*state, target, current.as_ref(), align.as_ref(), &exit),
            )
            .value()
        });
        let anim_slide = self.slide_animation.as_ref().map(|a| {
            let (e, x) = (self.enter.clone(), self.exit.clone());
            let (enter, exit, m) = (self.enter.clone(), self.exit.clone(), self.mutable_transform_state.clone());
            a.animate_with_forced_initial(
                move |s| {
                    if s.is_transitioning_to(EnterExitState::PreEnter, EnterExitState::Visible) {
                        e.data.slide.as_ref().map_or_else(default_offset_spec, |s| s.animation_spec.clone())
                    } else if s.is_transitioning_to(EnterExitState::Visible, EnterExitState::PostExit) {
                        x.data.slide.as_ref().map_or_else(default_offset_spec, |s| s.animation_spec.clone())
                    } else {
                        default_offset_spec()
                    }
                },
                self.mutable_transform_state.slide_handoff_value(),
                self.mutable_transform_state.slide_handoff_velocity(),
                move |state| {
                    if *state == EnterExitState::PostExit && exit.data.slide.is_none() {
                        m.last_slide()
                    } else {
                        slide_target_value_by_state(*state, target, &enter, &exit)
                    }
                },
            )
        });
        let current_alignment = self.current_alignment.clone();
        let mts = self.mutable_transform_state.clone();
        scope.layout(current_size.width, current_size.height, move |p: &PlacementScope| {
            let combined_slide =
                mts.combined_slide(anim_slide.as_ref().map_or(IntOffset::Zero, |s| s.value()), measured);
            let aligned = current_alignment
                .as_ref()
                .map_or(IntOffset::Zero, |a| a.align(target, current_size, LayoutDirection::Ltr));
            let offset = aligned + combined_slide;
            p.place_with_layer_block(
                &placeable,
                IntOffset::new(offset.x + offset_delta.x, offset.y + offset_delta.y),
                layer_block.clone(),
                0.0,
            );
        })
    }

    // (Compose's LayoutModifierNodeWithPassThroughIntrinsics: what it wraps answers)
    fn min_intrinsic_width(&mut self, _: &IntrinsicMeasureScope, m: &dyn IntrinsicMeasurable, height: i32) -> i32 {
        m.min_intrinsic_width(height)
    }
    fn min_intrinsic_height(&mut self, _: &IntrinsicMeasureScope, m: &dyn IntrinsicMeasurable, width: i32) -> i32 {
        m.min_intrinsic_height(width)
    }
    fn max_intrinsic_width(&mut self, _: &IntrinsicMeasureScope, m: &dyn IntrinsicMeasurable, height: i32) -> i32 {
        m.max_intrinsic_width(height)
    }
    fn max_intrinsic_height(&mut self, _: &IntrinsicMeasureScope, m: &dyn IntrinsicMeasurable, width: i32) -> i32 {
        m.max_intrinsic_height(width)
    }
}

/// The offset smoothing over an alignment that changed on the way (an enter expanding from one side and an exit
/// shrinking towards another; Compose's node's `targetOffsetByState`).
fn target_offset_by_state(
    state: EnterExitState,
    full_size: IntSize,
    current_alignment: Option<&Alignment>,
    alignment: Option<&Alignment>,
    exit: &ExitTransition,
) -> IntOffset {
    let (Some(current), Some(alignment)) = (current_alignment, alignment) else { return IntOffset::Zero };
    if current == alignment {
        return IntOffset::Zero;
    }
    match state {
        EnterExitState::Visible | EnterExitState::PreEnter => IntOffset::Zero,
        EnterExitState::PostExit => exit.data.change_size.as_ref().map_or(IntOffset::Zero, |c| {
            let end_size = (c.size)(full_size);
            let target_offset = alignment.align(full_size, end_size, LayoutDirection::Ltr);
            let current_offset = current.align(full_size, end_size, LayoutDirection::Ltr);
            target_offset - current_offset
        }),
    }
}

/// The slide's offset of a state (Compose's node's `slideTargetValueByState`).
fn slide_target_value_by_state(
    state: EnterExitState,
    full_size: IntSize,
    enter: &EnterTransition,
    exit: &ExitTransition,
) -> IntOffset {
    match state {
        EnterExitState::Visible => IntOffset::Zero,
        EnterExitState::PreEnter => enter.data.slide.as_ref().map_or(IntOffset::Zero, |s| (s.slide_offset)(full_size)),
        EnterExitState::PostExit => exit.data.slide.as_ref().map_or(IntOffset::Zero, |s| (s.slide_offset)(full_size)),
    }
}

/// Compose's private VeilModifierElement.
struct VeilModifierElement {
    transition: Transition<EnterExitState>,
    veil_animation: DeferredAnimation<EnterExitState, Color, AnimationVector4D>,
    enter: EnterTransition,
    exit: ExitTransition,
    mutable_transform_state: SharedMutableTransformState,
}

impl PartialEq for VeilModifierElement {
    fn eq(&self, other: &Self) -> bool {
        self.transition == other.transition
            && self.veil_animation == other.veil_animation
            && self.enter == other.enter
            && self.exit == other.exit
            && self.mutable_transform_state == other.mutable_transform_state
    }
}

impl ModifierNodeElement for VeilModifierElement {
    type Node = VeilModifierNode;
    fn create(&self) -> VeilModifierNode {
        VeilModifierNode {
            veil_animation: self.veil_animation.clone(),
            enter: self.enter.clone(),
            exit: self.exit.clone(),
            mutable_transform_state: self.mutable_transform_state.clone(),
            handle: None,
        }
    }
    fn update(&self, node: &mut VeilModifierNode) {
        node.veil_animation = self.veil_animation.clone();
        node.enter = self.enter.clone();
        node.exit = self.exit.clone();
        node.mutable_transform_state = self.mutable_transform_state.clone();
    }
    fn name(&self) -> &'static str {
        "veil"
    }
}

/// Compose's private VeilModifierNode: the veil's color, animated as it draws, over the content (or its parent).
struct VeilModifierNode {
    veil_animation: DeferredAnimation<EnterExitState, Color, AnimationVector4D>,
    enter: EnterTransition,
    exit: ExitTransition,
    mutable_transform_state: SharedMutableTransformState,
    handle: Option<NodeHandle>,
}

impl ModifierNode for VeilModifierNode {
    node_capabilities!(draw);

    fn on_attach(&mut self, cx: &NodeCx) {
        self.handle = Some(cx.handle());
    }
}

impl DrawModifierNode for VeilModifierNode {
    fn draw(&mut self, scope: &mut ContentDrawScope) {
        scope.draw_content();
        let (e, x) = (self.enter.clone(), self.exit.clone());
        let (enter, exit, m) = (self.enter.clone(), self.exit.clone(), self.mutable_transform_state.clone());
        let veil_color = self.veil_animation.animate_with_forced_initial(
            move |s| {
                if s.is_transitioning_to(EnterExitState::PreEnter, EnterExitState::Visible) {
                    e.data.veil.as_ref().map_or_else(default_color_spec, |v| v.animation_spec.clone())
                } else if s.is_transitioning_to(EnterExitState::Visible, EnterExitState::PostExit) {
                    x.data.veil.as_ref().map_or_else(default_color_spec, |v| v.animation_spec.clone())
                } else {
                    default_color_spec()
                }
            },
            self.mutable_transform_state.veil_handoff_value(),
            None,
            move |state| match state {
                EnterExitState::Visible => enter
                    .data
                    .veil
                    .as_ref()
                    .map(|v| v.target_color)
                    .or_else(|| exit.data.veil.as_ref().map(|v| v.initial_color))
                    .unwrap_or(Color::Transparent),
                EnterExitState::PreEnter => enter.data.veil.as_ref().map_or(Color::Transparent, |v| v.initial_color),
                EnterExitState::PostExit => exit.data.veil.as_ref().map_or_else(|| m.last_veil(), |v| v.target_color),
            },
        );
        let combined = self.mutable_transform_state.combined_veil(veil_color.value());
        if combined.alpha() != 0.0 {
            let veil = self.enter.data.veil.as_ref().or(self.exit.data.veil.as_ref());
            if veil.is_some_and(|v| v.match_parent_size) {
                let coordinates = self.handle.as_ref().and_then(|h| h.coordinates());
                let parent_size = coordinates
                    .as_ref()
                    .and_then(|c| c.parent_layout_coordinates())
                    .map_or(Size::Zero, |p| Size::new(p.size().width as f32, p.size().height as f32));
                let offset_in_parent = coordinates.map_or(Offset::Zero, |c| c.position_in_parent());
                kompoz_runtime::named_args!(scope.draw_rect(
                    combined,
                    top_left = Offset::new(-offset_in_parent.x, -offset_in_parent.y),
                    size = parent_size
                ));
            } else {
                scope.draw_rect(combined);
            }
        }
    }
}
