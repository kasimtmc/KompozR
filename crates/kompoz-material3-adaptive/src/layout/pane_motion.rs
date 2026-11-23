//! How panes move, as Compose's adaptive-layout PaneMotion.kt (1.2.0): [`PaneMotion`] (a pane's motion between two
//! scaffold values: entering, exiting, keeping shown, modal), [`PaneMotionData`] and the provider of all panes'
//! ([`PaneScaffoldMotionDataProvider`]), the default enter/exit transitions it gives
//! ([`calculate_default_enter_transition`](PaneScaffoldMotionDataProviderExt::calculate_default_enter_transition)),
//! [`PaneMotionDefaults`]' specs, and how the motions are decided (`calculate_pane_motion`).

use super::pane_adapted_value::PaneAdaptedValue;
use super::pane_scaffold_role::PaneScaffoldRole;
use super::three_pane_scaffold_value::{PaneScaffoldHorizontalOrder, PaneScaffoldValue};
use kompoz_animation::{
    AnimationSpec, AnimationVector, AnimationVector1D, AnimationVector2D, AnimationVector3D, AnimationVector4D,
    EnterTransition, ExitTransition, FiniteAnimationSpec, Spring, SpringSpec, TwoWayConverter, VectorizedAnimationSpec,
    VectorizedFiniteAnimationSpec, expand_horizontally, fade_in, fade_out, shrink_horizontally, slide_in_horizontally,
    slide_out_horizontally,
};
use kompoz_graphics::{IntOffset, IntRect, IntSize};
use kompoz_runtime::kt::KtToString;
use kompoz_runtime::{MutableState, mutable_state_of, named_args};
use kompoz_ui::Alignment;
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::fmt;
use std::sync::Arc;

// ---- PaneScaffoldMotionDataProvider ----

/// What a pane scaffold knows of its panes' motion (Compose's sealed interface `PaneScaffoldMotionDataProvider`):
/// the scaffold's size, and each pane's [`PaneMotionData`] by role or by its index left to right.
pub trait PaneScaffoldMotionDataProvider {
    /// The roles of its panes.
    type Role: PaneScaffoldRole + Copy + PartialEq;

    /// The scaffold's size.
    fn scaffold_size(&self) -> IntSize;

    /// The number of panes.
    fn count(&self) -> i32;

    /// The role of the pane at `index`, left to right.
    fn get_role_at(&self, index: i32) -> Self::Role;

    /// The motion data of the pane of `role` (Kotlin's operator `get(role)`).
    fn get(&self, role: Self::Role) -> &PaneMotionData;

    /// The motion data of the pane at `index`, left to right (Kotlin's operator `get(index)`).
    fn get_at(&self, index: i32) -> &PaneMotionData {
        self.get(self.get_role_at(index))
    }
}

/// The provider's panes one by one (Compose's inline `forEach` / `forEachReversed` extensions) and the motions it
/// gives (Compose's `calculateDefaultEnterTransition` / `calculateDefaultExitTransition` extensions).
pub trait PaneScaffoldMotionDataProviderExt: PaneScaffoldMotionDataProvider {
    /// `action` of each pane's role and data, left to right.
    fn for_each(&self, mut action: impl FnMut(Self::Role, &PaneMotionData)) {
        for i in 0..self.count() {
            action(self.get_role_at(i), self.get_at(i));
        }
    }

    /// `action` of each pane's role and data, right to left.
    fn for_each_reversed(&self, mut action: impl FnMut(Self::Role, &PaneMotionData)) {
        for i in (0..self.count()).rev() {
            action(self.get_role_at(i), self.get_at(i));
        }
    }

    /// The default enter transition of the pane of `role` by its motion: sliding in from the left or right
    /// (delayed or not) from beside the panes in its way, expanding (and sliding) between shown panes, fading in
    /// as a modal; none otherwise.
    fn calculate_default_enter_transition(&self, role: Self::Role) -> EnterTransition
    where
        Self: Sized,
    {
        let motion = self.get(role).motion();
        if motion == PaneMotion::EnterFromLeft {
            let offset = slide_in_from_left_offset(self);
            named_args!(slide_in_horizontally(
                animation_spec = PaneMotionDefaults::OffsetAnimationSpec(),
                initial_offset_x = move |_| offset
            ))
        } else if motion == PaneMotion::EnterFromLeftDelayed {
            let offset = slide_in_from_left_offset(self);
            named_args!(slide_in_horizontally(
                animation_spec = PaneMotionDefaults::DelayedOffsetAnimationSpec(),
                initial_offset_x = move |_| offset
            ))
        } else if motion == PaneMotion::EnterFromRight {
            let offset = slide_in_from_right_offset(self);
            named_args!(slide_in_horizontally(
                animation_spec = PaneMotionDefaults::OffsetAnimationSpec(),
                initial_offset_x = move |_| offset
            ))
        } else if motion == PaneMotion::EnterFromRightDelayed {
            let offset = slide_in_from_right_offset(self);
            named_args!(slide_in_horizontally(
                animation_spec = PaneMotionDefaults::DelayedOffsetAnimationSpec(),
                initial_offset_x = move |_| offset
            ))
        } else if motion == PaneMotion::EnterWithExpand {
            let offset = get_hidden_pane_current_left(self, role) - self.get(role).target_left();
            named_args!(expand_horizontally(
                animation_spec = PaneMotionDefaults::SizeAnimationSpec(),
                expand_from = Alignment::CenterHorizontally
            )) + named_args!(slide_in_horizontally(
                animation_spec = PaneMotionDefaults::OffsetAnimationSpec(),
                initial_offset_x = move |_| offset
            ))
        } else if motion == PaneMotion::EnterAsModal {
            named_args!(fade_in(animation_spec = PaneMotionDefaults::VisibilityAnimationSpec()))
        } else {
            EnterTransition::None
        }
    }

    /// The default exit transition of the pane of `role` by its motion: sliding out to the left or right past
    /// the panes in its way, shrinking (and sliding) between shown panes, fading out as a modal; none otherwise.
    fn calculate_default_exit_transition(&self, role: Self::Role) -> ExitTransition
    where
        Self: Sized,
    {
        let motion = self.get(role).motion();
        if motion == PaneMotion::ExitToLeft {
            let offset = slide_out_to_left_offset(self);
            named_args!(slide_out_horizontally(
                animation_spec = PaneMotionDefaults::OffsetAnimationSpec(),
                target_offset_x = move |_| offset
            ))
        } else if motion == PaneMotion::ExitToRight {
            let offset = slide_out_to_right_offset(self);
            named_args!(slide_out_horizontally(
                animation_spec = PaneMotionDefaults::OffsetAnimationSpec(),
                target_offset_x = move |_| offset
            ))
        } else if motion == PaneMotion::ExitWithShrink {
            let offset = get_hiding_pane_target_left(self, role) - self.get(role).current_left();
            named_args!(shrink_horizontally(
                animation_spec = PaneMotionDefaults::SizeAnimationSpec(),
                shrink_towards = Alignment::CenterHorizontally
            )) + named_args!(slide_out_horizontally(
                animation_spec = PaneMotionDefaults::OffsetAnimationSpec(),
                target_offset_x = move |_| offset
            ))
        } else if motion == PaneMotion::ExitAsModal {
            named_args!(fade_out(animation_spec = PaneMotionDefaults::VisibilityAnimationSpec()))
        } else {
            ExitTransition::None
        }
    }
}

impl<P: PaneScaffoldMotionDataProvider + ?Sized> PaneScaffoldMotionDataProviderExt for P {}

// ---- PaneMotionDefaults ----

/// The specs panes move with (Compose's `PaneMotionDefaults`): a spring of damping 0.8 and stiffness 380 for the
/// bounds, a tenth of its duration later for the delayed ones, their offsets and sizes, the visibility's.
pub struct PaneMotionDefaults;

const IntRectVisibilityThreshold: IntRect = IntRect { left: 1, top: 1, right: 1, bottom: 1 };

// (Compose's vals: made once per thread, as a spec holding another — an `Arc<dyn FiniteAnimationSpec<_>>` — isn't
// `Send + Sync`: specs animate any value)
impl PaneMotionDefaults {
    /// The bounds' spring (Compose's `AnimationSpec`): damping 0.8, stiffness 380, to within 1 px.
    pub fn AnimationSpec() -> Arc<dyn FiniteAnimationSpec<IntRect>> {
        thread_local! {
            static SPEC: Arc<dyn FiniteAnimationSpec<IntRect>> = Arc::new(SpringSpec::new(0.8, 380.0, Some(IntRectVisibilityThreshold)));
        }
        SPEC.with(Arc::clone)
    }

    /// The same spring started a tenth of its duration later (Compose's `DelayedAnimationSpec`).
    pub fn DelayedAnimationSpec() -> Arc<dyn FiniteAnimationSpec<IntRect>> {
        thread_local! {
            static SPEC: Arc<dyn FiniteAnimationSpec<IntRect>> = Arc::new(DelayedSpringSpec::new(0.8, 380.0, 0.1, Some(IntRectVisibilityThreshold)));
        }
        SPEC.with(Arc::clone)
    }

    /// The visibility's spring (Compose's `VisibilityAnimationSpec`): damping 0.8, stiffness 380.
    pub fn VisibilityAnimationSpec() -> Arc<dyn FiniteAnimationSpec<f32>> {
        thread_local! {
            static SPEC: Arc<dyn FiniteAnimationSpec<f32>> = Arc::new(SpringSpec::<f32>::new(0.8, 380.0, None));
        }
        SPEC.with(Arc::clone)
    }

    /// The bounds' spring of an offset (Compose's `OffsetAnimationSpec`).
    pub fn OffsetAnimationSpec() -> Arc<dyn FiniteAnimationSpec<IntOffset>> {
        thread_local! {
            static SPEC: Arc<dyn FiniteAnimationSpec<IntOffset>> = Arc::new(DerivedOffsetAnimationSpec { bounds_spec: PaneMotionDefaults::AnimationSpec() });
        }
        SPEC.with(Arc::clone)
    }

    /// The bounds' spring of a size (Compose's `SizeAnimationSpec`).
    pub fn SizeAnimationSpec() -> Arc<dyn FiniteAnimationSpec<IntSize>> {
        thread_local! {
            static SPEC: Arc<dyn FiniteAnimationSpec<IntSize>> = Arc::new(DerivedSizeAnimationSpec { bounds_spec: PaneMotionDefaults::AnimationSpec() });
        }
        SPEC.with(Arc::clone)
    }

    /// The delayed spring of an offset (Compose's `DelayedOffsetAnimationSpec`).
    pub fn DelayedOffsetAnimationSpec() -> Arc<dyn FiniteAnimationSpec<IntOffset>> {
        thread_local! {
            static SPEC: Arc<dyn FiniteAnimationSpec<IntOffset>> = Arc::new(DerivedOffsetAnimationSpec { bounds_spec: PaneMotionDefaults::DelayedAnimationSpec() });
        }
        SPEC.with(Arc::clone)
    }

    /// The delayed spring of a size (Compose's `DelayedSizeAnimationSpec`).
    pub fn DelayedSizeAnimationSpec() -> Arc<dyn FiniteAnimationSpec<IntSize>> {
        thread_local! {
            static SPEC: Arc<dyn FiniteAnimationSpec<IntSize>> = Arc::new(DerivedSizeAnimationSpec { bounds_spec: PaneMotionDefaults::DelayedAnimationSpec() });
        }
        SPEC.with(Arc::clone)
    }
}

// ---- PaneMotionData ----

/// What a pane scaffold knows of a pane's motion (Compose's `PaneMotionData`): its motion (observable), and where
/// it was (origin) and is going (target) in the scaffold during the motion.
pub struct PaneMotionData {
    motion: MutableState<PaneMotion>,
    origin_size: Cell<IntSize>,
    origin_position: Cell<IntOffset>,
    target_size: Cell<IntSize>,
    target_position: Cell<IntOffset>,
    pub(crate) z_index: Cell<f32>,
    pub(crate) is_origin_size_and_position_set: Cell<bool>,
    pub(crate) is_target_size_and_position_set: Cell<bool>,
}

impl PaneMotionData {
    /// Compose's internal constructor.
    pub(crate) fn new() -> PaneMotionData {
        PaneMotionData {
            motion: mutable_state_of(PaneMotion::NoMotion),
            origin_size: Cell::new(IntSize::Zero),
            origin_position: Cell::new(IntOffset::Zero),
            target_size: Cell::new(IntSize::Zero),
            target_position: Cell::new(IntOffset::Zero),
            z_index: Cell::new(0.0),
            is_origin_size_and_position_set: Cell::new(false),
            is_target_size_and_position_set: Cell::new(false),
        }
    }

    /// The pane's motion of the current change of the scaffold's value.
    pub fn motion(&self) -> PaneMotion {
        self.motion.get()
    }
    pub(crate) fn set_motion(&self, motion: PaneMotion) {
        self.motion.set(motion);
    }

    /// The pane's size before the motion.
    pub fn origin_size(&self) -> IntSize {
        self.origin_size.get()
    }
    pub(crate) fn set_origin_size(&self, size: IntSize) {
        self.origin_size.set(size);
    }

    /// The pane's position in the scaffold before the motion.
    pub fn origin_position(&self) -> IntOffset {
        self.origin_position.get()
    }
    pub(crate) fn set_origin_position(&self, position: IntOffset) {
        self.origin_position.set(position);
    }

    /// The pane's size after the motion.
    pub fn target_size(&self) -> IntSize {
        self.target_size.get()
    }
    pub(crate) fn set_target_size(&self, size: IntSize) {
        self.target_size.set(size);
    }

    /// The pane's position in the scaffold after the motion.
    pub fn target_position(&self) -> IntOffset {
        self.target_position.get()
    }
    pub(crate) fn set_target_position(&self, position: IntOffset) {
        self.target_position.set(position);
    }

    pub(crate) fn target_left(&self) -> i32 {
        self.target_position().x
    }
    pub(crate) fn target_right(&self) -> i32 {
        self.target_position().x + self.target_size().width
    }
    pub(crate) fn target_top(&self) -> i32 {
        self.target_position().y
    }
    pub(crate) fn target_bottom(&self) -> i32 {
        self.target_position().y + self.target_size().height
    }
    pub(crate) fn current_left(&self) -> i32 {
        self.origin_position().x
    }
    pub(crate) fn current_right(&self) -> i32 {
        self.origin_position().x + self.origin_size().width
    }
}

fn is_shown_or(motion: PaneMotion, other: PaneMotionType) -> bool {
    motion.type_() == PaneMotionType::Shown || motion.type_() == other
}

/// How far a pane entering from the left slides in (Compose's internal `slideInFromLeftOffset`): to the left edge
/// of the shown or entering pane after the last one sliding in from the left, or that one's right edge.
pub(crate) fn slide_in_from_left_offset<P: PaneScaffoldMotionDataProvider + ?Sized>(provider: &P) -> i32 {
    let mut previous_left: Option<i32> = None;
    for i in (0..provider.count()).rev() {
        let it = provider.get_at(i);
        let motion = it.motion();
        if motion == PaneMotion::EnterFromLeft || motion == PaneMotion::EnterFromLeftDelayed {
            return -(previous_left.unwrap_or(it.target_right()));
        }
        if is_shown_or(motion, PaneMotionType::Entering) {
            previous_left = Some(it.target_left());
        }
    }
    0
}

/// How far a pane entering from the right slides in (Compose's internal `slideInFromRightOffset`).
pub(crate) fn slide_in_from_right_offset<P: PaneScaffoldMotionDataProvider + ?Sized>(provider: &P) -> i32 {
    let mut previous_right: Option<i32> = None;
    for i in 0..provider.count() {
        let it = provider.get_at(i);
        let motion = it.motion();
        if motion == PaneMotion::EnterFromRight || motion == PaneMotion::EnterFromRightDelayed {
            return provider.scaffold_size().width - previous_right.unwrap_or(it.target_left());
        }
        if is_shown_or(motion, PaneMotionType::Entering) {
            previous_right = Some(it.target_right());
        }
    }
    0
}

/// How far a pane exiting to the left slides out (Compose's internal `slideOutToLeftOffset`).
pub(crate) fn slide_out_to_left_offset<P: PaneScaffoldMotionDataProvider + ?Sized>(provider: &P) -> i32 {
    let mut previous_left: Option<i32> = None;
    for i in (0..provider.count()).rev() {
        let it = provider.get_at(i);
        let motion = it.motion();
        if motion == PaneMotion::ExitToLeft {
            return -(previous_left.unwrap_or(it.current_right()));
        }
        if is_shown_or(motion, PaneMotionType::Exiting) {
            previous_left = Some(it.current_left());
        }
    }
    0
}

/// How far a pane exiting to the right slides out (Compose's internal `slideOutToRightOffset`).
pub(crate) fn slide_out_to_right_offset<P: PaneScaffoldMotionDataProvider + ?Sized>(provider: &P) -> i32 {
    let mut previous_right: Option<i32> = None;
    for i in 0..provider.count() {
        let it = provider.get_at(i);
        let motion = it.motion();
        if motion == PaneMotion::ExitToRight {
            return provider.scaffold_size().width - previous_right.unwrap_or(it.current_left());
        }
        if is_shown_or(motion, PaneMotionType::Exiting) {
            previous_right = Some(it.current_right());
        }
    }
    0
}

/// The right edge of the shown or exiting pane left of the hidden pane of `role` (Compose's internal
/// `getHiddenPaneCurrentLeft`).
pub(crate) fn get_hidden_pane_current_left<P: PaneScaffoldMotionDataProvider + ?Sized>(
    provider: &P,
    role: P::Role,
) -> i32 {
    let mut current_left = 0;
    for i in 0..provider.count() {
        let (pane_role, data) = (provider.get_role_at(i), provider.get_at(i));
        if pane_role == role {
            return current_left;
        }
        if is_shown_or(data.motion(), PaneMotionType::Exiting) {
            current_left = data.current_right();
        }
    }
    current_left
}

/// The right edge of the shown or entering pane left of the hiding pane of `role` (Compose's internal
/// `getHidingPaneTargetLeft`).
pub(crate) fn get_hiding_pane_target_left<P: PaneScaffoldMotionDataProvider + ?Sized>(
    provider: &P,
    role: P::Role,
) -> i32 {
    let mut target_left = 0;
    for i in 0..provider.count() {
        let (pane_role, data) = (provider.get_role_at(i), provider.get_at(i));
        if pane_role == role {
            return target_left;
        }
        if is_shown_or(data.motion(), PaneMotionType::Entering) {
            target_left = data.target_right();
        }
    }
    target_left
}

// ---- PaneMotion ----

/// The kind of a pane's motion (Compose's value class `PaneMotion.Type`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct PaneMotionType(i32);

impl PaneMotionType {
    /// Hidden before and after.
    pub const Hidden: PaneMotionType = PaneMotionType(0);
    /// Shown before, hidden after.
    pub const Exiting: PaneMotionType = PaneMotionType(1);
    /// Hidden before, shown after.
    pub const Entering: PaneMotionType = PaneMotionType(2);
    /// Shown before and after.
    pub const Shown: PaneMotionType = PaneMotionType(3);
    /// Exiting, levitated before or after.
    pub const ExitingModal: PaneMotionType = PaneMotionType(5);
    /// Entering, levitated before or after.
    pub const EnteringModal: PaneMotionType = PaneMotionType(6);

    /// Its value (Compose's `value`).
    pub fn value(self) -> i32 {
        self.0
    }

    /// The kind of motion from `previous_value` to `current_value` (Compose's internal `Type.calculate`).
    pub(crate) fn calculate(previous_value: &PaneAdaptedValue, current_value: &PaneAdaptedValue) -> PaneMotionType {
        let was_shown = *previous_value != PaneAdaptedValue::Hidden;
        let is_shown = *current_value != PaneAdaptedValue::Hidden;
        let levitated_pane = previous_value.is_levitated() || current_value.is_levitated();
        match (was_shown, is_shown) {
            (true, true) => Self::Shown,
            (false, false) => Self::Hidden,
            (true, false) => {
                if levitated_pane {
                    Self::ExitingModal
                } else {
                    Self::Exiting
                }
            }
            (false, true) => {
                if levitated_pane {
                    Self::EnteringModal
                } else {
                    Self::Entering
                }
            }
        }
    }
}

/// Kotlin's `toString`: `PaneMotion.Type[Shown]`.
impl fmt::Display for PaneMotionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match *self {
            Self::Hidden => "Hidden".to_string(),
            Self::Exiting => "Exiting".to_string(),
            Self::Entering => "Entering".to_string(),
            Self::Shown => "Shown".to_string(),
            Self::ExitingModal => "ExitingModal".to_string(),
            Self::EnteringModal => "EnteringModal".to_string(),
            other => format!("Unknown value={}", other.0),
        };
        write!(f, "PaneMotion.Type[{name}]")
    }
}

/// A pane's motion between two scaffold values (Compose's sealed interface `PaneMotion` and its predefined
/// motions; the custom motions Compose's sealed interface can't have either). Equal by identity of the predefined
/// one, as Compose's objects.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct PaneMotion {
    name: &'static str,
    type_: PaneMotionType,
}

impl PaneMotion {
    /// No motion.
    pub const NoMotion: PaneMotion = PaneMotion { name: "NoMotion", type_: PaneMotionType::Hidden };
    /// Shown before and after: its bounds animate.
    pub const AnimateBounds: PaneMotion = PaneMotion { name: "AnimateBounds", type_: PaneMotionType::Shown };
    /// Sliding in from the left.
    pub const EnterFromLeft: PaneMotion = PaneMotion { name: "EnterFromLeft", type_: PaneMotionType::Entering };
    /// Sliding in from the right.
    pub const EnterFromRight: PaneMotion = PaneMotion { name: "EnterFromRight", type_: PaneMotionType::Entering };
    /// Sliding in from the left after a delay.
    pub const EnterFromLeftDelayed: PaneMotion =
        PaneMotion { name: "EnterFromLeftDelayed", type_: PaneMotionType::Entering };
    /// Sliding in from the right after a delay.
    pub const EnterFromRightDelayed: PaneMotion =
        PaneMotion { name: "EnterFromRightDelayed", type_: PaneMotionType::Entering };
    /// Sliding out to the left.
    pub const ExitToLeft: PaneMotion = PaneMotion { name: "ExitToLeft", type_: PaneMotionType::Exiting };
    /// Sliding out to the right.
    pub const ExitToRight: PaneMotion = PaneMotion { name: "ExitToRight", type_: PaneMotionType::Exiting };
    /// Expanding in between shown panes.
    pub const EnterWithExpand: PaneMotion = PaneMotion { name: "EnterWithExpand", type_: PaneMotionType::Entering };
    /// Shrinking out between shown panes.
    pub const ExitWithShrink: PaneMotion = PaneMotion { name: "ExitWithShrink", type_: PaneMotionType::Exiting };
    /// Entering as a modal (fading in).
    pub const EnterAsModal: PaneMotion = PaneMotion { name: "EnterAsModal", type_: PaneMotionType::EnteringModal };
    /// Exiting as a modal (fading out).
    pub const ExitAsModal: PaneMotion = PaneMotion { name: "ExitAsModal", type_: PaneMotionType::ExitingModal };

    /// Its kind (Compose's `type`).
    pub fn type_(&self) -> PaneMotionType {
        self.type_
    }
}

/// Kotlin's `toString`: its name.
impl fmt::Display for PaneMotion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name)
    }
}

impl fmt::Debug for PaneMotion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name)
    }
}

impl KtToString for PaneMotion {
    fn kt_write(&self, out: &mut String) {
        out.push_str(self.name);
    }
}

/// The motions of the panes from `previous_scaffold_value` to `current_scaffold_value`, in `pane_order`'s order
/// (Compose's internal `calculatePaneMotion`): shown panes animate their bounds; exiting panes leave to the right
/// or left where nothing interferes (shrinking between shown panes); entering panes come from the right or left
/// where nothing blocks (delayed past exiting panes, expanding between shown ones); levitated panes come and go as
/// modals.
pub(crate) fn calculate_pane_motion<V, O>(
    previous_scaffold_value: &V,
    current_scaffold_value: &V,
    pane_order: &O,
) -> Vec<PaneMotion>
where
    V: PaneScaffoldValue,
    O: PaneScaffoldHorizontalOrder<Role = V::Role>,
{
    let num_of_panes = pane_order.size();
    let n = num_of_panes as usize;
    let mut pane_motion_types = vec![PaneMotionType::Hidden; n];
    let mut pane_motions = vec![PaneMotion::NoMotion; n];
    let mut first_shown_pane_index = num_of_panes;
    let mut first_entering_pane_index = num_of_panes;
    let mut last_shown_pane_index = -1;
    let mut last_entering_pane_index = -1;
    // First pass: each pane's entering/exiting status; shown panes animate their bounds.
    pane_order.for_each_indexed(&mut |i, role| {
        let t = PaneMotionType::calculate(&previous_scaffold_value.get(role), &current_scaffold_value.get(role));
        pane_motion_types[i as usize] = t;
        if t == PaneMotionType::Shown {
            first_shown_pane_index = first_shown_pane_index.min(i);
            last_shown_pane_index = last_shown_pane_index.max(i);
            pane_motions[i as usize] = PaneMotion::AnimateBounds;
        } else if t == PaneMotionType::Entering {
            first_entering_pane_index = first_entering_pane_index.min(i);
            last_entering_pane_index = last_entering_pane_index.max(i);
        }
    });
    // Second pass: the exiting motions.
    let mut has_panes_exit_to_right = false;
    let mut has_panes_exit_to_left = false;
    let mut first_pane_exit_to_right_index = num_of_panes;
    let mut last_pane_exit_to_left_index = -1;
    for i in 0..num_of_panes {
        let has_shown_panes_on_left = first_shown_pane_index < i;
        let has_entering_panes_on_left = first_entering_pane_index < i;
        let has_shown_panes_on_right = last_shown_pane_index > i;
        let has_entering_panes_on_right = last_entering_pane_index > i;
        if pane_motion_types[i as usize] == PaneMotionType::Exiting {
            pane_motions[i as usize] = if !has_shown_panes_on_right && !has_entering_panes_on_right {
                // No panes will interfere the motion on the right, exit to right.
                has_panes_exit_to_right = true;
                first_pane_exit_to_right_index = first_pane_exit_to_right_index.min(i);
                PaneMotion::ExitToRight
            } else if !has_shown_panes_on_left && !has_entering_panes_on_left {
                // No panes will interfere the motion on the left, exit to left.
                has_panes_exit_to_left = true;
                last_pane_exit_to_left_index = last_pane_exit_to_left_index.max(i);
                PaneMotion::ExitToLeft
            } else if !has_shown_panes_on_right {
                // Only showing panes can interfere the motion on the right, exit to right.
                has_panes_exit_to_right = true;
                first_pane_exit_to_right_index = first_pane_exit_to_right_index.min(i);
                PaneMotion::ExitToRight
            } else if !has_shown_panes_on_left {
                // Only showing panes can interfere the motion on the left, exit to left.
                has_panes_exit_to_left = true;
                last_pane_exit_to_left_index = last_pane_exit_to_left_index.max(i);
                PaneMotion::ExitToLeft
            } else {
                // Both sides has panes that keep being visible during transition, shrink to exit
                PaneMotion::ExitWithShrink
            };
        }
    }
    // Third pass: the entering motions.
    for i in 0..num_of_panes {
        let has_shown_panes_on_left = first_shown_pane_index < i;
        let has_shown_panes_on_right = last_shown_pane_index > i;
        let has_left_pane_exit_to_right = first_pane_exit_to_right_index < i;
        let has_right_pane_exit_to_left = last_pane_exit_to_left_index > i;
        let no_blocking_panes_on_right = !has_shown_panes_on_right && !has_right_pane_exit_to_left;
        let no_blocking_panes_on_left = !has_shown_panes_on_left && !has_left_pane_exit_to_right;
        if pane_motion_types[i as usize] == PaneMotionType::Entering {
            pane_motions[i as usize] = if no_blocking_panes_on_right && !has_panes_exit_to_right {
                PaneMotion::EnterFromRight
            } else if no_blocking_panes_on_left && !has_panes_exit_to_left {
                PaneMotion::EnterFromLeft
            } else if no_blocking_panes_on_right {
                PaneMotion::EnterFromRightDelayed
            } else if no_blocking_panes_on_left {
                PaneMotion::EnterFromLeftDelayed
            } else {
                PaneMotion::EnterWithExpand
            };
        }
    }
    // Fourth pass: the levitated panes' motions.
    for i in 0..n {
        if pane_motion_types[i] == PaneMotionType::EnteringModal {
            pane_motions[i] = PaneMotion::EnterAsModal;
        } else if pane_motion_types[i] == PaneMotionType::ExitingModal {
            pane_motions[i] = PaneMotion::ExitAsModal;
        }
    }
    pane_motions
}

// ---- The specs ----

/// An IntRect as a 4D vector and back (Compose's internal `IntRectToVector`).
pub(crate) fn int_rect_to_vector() -> TwoWayConverter<IntRect, AnimationVector4D> {
    TwoWayConverter::new(
        |r: IntRect| AnimationVector4D { v1: r.left as f32, v2: r.top as f32, v3: r.right as f32, v4: r.bottom as f32 },
        |v: AnimationVector4D| IntRect {
            left: fast_round_to_int(v.v1),
            top: fast_round_to_int(v.v2),
            right: fast_round_to_int(v.v3),
            bottom: fast_round_to_int(v.v4),
        },
    )
}

/// Kotlin's `fastRoundToInt` (Math.round).
fn fast_round_to_int(v: f32) -> i32 {
    (v + 0.5).floor() as i32
}

/// The trait impls of a finite spec type with an inherent `vectorize_into<V>`, equal as `eq_spec` says.
macro_rules! finite_spec_impls {
    ($ty:ty, $value:ty) => {
        impl AnimationSpec<$value> for $ty {
            fn vectorize_1d(
                &self,
                c: &TwoWayConverter<$value, AnimationVector1D>,
            ) -> Box<dyn VectorizedAnimationSpec<AnimationVector1D>> {
                self.vectorize_into_any(c)
            }
            fn vectorize_2d(
                &self,
                c: &TwoWayConverter<$value, AnimationVector2D>,
            ) -> Box<dyn VectorizedAnimationSpec<AnimationVector2D>> {
                self.vectorize_into_any(c)
            }
            fn vectorize_3d(
                &self,
                c: &TwoWayConverter<$value, AnimationVector3D>,
            ) -> Box<dyn VectorizedAnimationSpec<AnimationVector3D>> {
                self.vectorize_into_any(c)
            }
            fn vectorize_4d(
                &self,
                c: &TwoWayConverter<$value, AnimationVector4D>,
            ) -> Box<dyn VectorizedAnimationSpec<AnimationVector4D>> {
                self.vectorize_into_any(c)
            }
            fn as_any(&self) -> &dyn Any {
                self
            }
            fn equals(&self, other: &dyn Any) -> bool {
                other.downcast_ref::<Self>().is_some_and(|o| self.eq_spec(o))
            }
        }
        impl FiniteAnimationSpec<$value> for $ty {
            fn vectorize_finite_1d(
                &self,
                c: &TwoWayConverter<$value, AnimationVector1D>,
            ) -> Box<dyn VectorizedFiniteAnimationSpec<AnimationVector1D>> {
                self.vectorize_into(c)
            }
            fn vectorize_finite_2d(
                &self,
                c: &TwoWayConverter<$value, AnimationVector2D>,
            ) -> Box<dyn VectorizedFiniteAnimationSpec<AnimationVector2D>> {
                self.vectorize_into(c)
            }
            fn vectorize_finite_3d(
                &self,
                c: &TwoWayConverter<$value, AnimationVector3D>,
            ) -> Box<dyn VectorizedFiniteAnimationSpec<AnimationVector3D>> {
                self.vectorize_into(c)
            }
            fn vectorize_finite_4d(
                &self,
                c: &TwoWayConverter<$value, AnimationVector4D>,
            ) -> Box<dyn VectorizedFiniteAnimationSpec<AnimationVector4D>> {
                self.vectorize_into(c)
            }
        }
    };
}

/// A finite vectorized spec as a plain one (a `Box<dyn VectorizedFiniteAnimationSpec<V>>` is one).
struct AsVectorized<V: AnimationVector>(Box<dyn VectorizedFiniteAnimationSpec<V>>);

impl<V: AnimationVector> VectorizedAnimationSpec<V> for AsVectorized<V> {
    fn get_value_from_nanos(&self, play_time_nanos: i64, initial_value: V, target_value: V, initial_velocity: V) -> V {
        self.0.get_value_from_nanos(play_time_nanos, initial_value, target_value, initial_velocity)
    }
    fn get_velocity_from_nanos(
        &self,
        play_time_nanos: i64,
        initial_value: V,
        target_value: V,
        initial_velocity: V,
    ) -> V {
        self.0.get_velocity_from_nanos(play_time_nanos, initial_value, target_value, initial_velocity)
    }
    fn get_duration_nanos(&self, initial_value: V, target_value: V, initial_velocity: V) -> i64 {
        self.0.get_duration_nanos(initial_value, target_value, initial_velocity)
    }
}

/// A size's spec made of a bounds spec (Compose's internal `DerivedSizeAnimationSpec`): the size as the bounds
/// `(0, 0, width, height)`. Equal by its bounds spec.
pub(crate) struct DerivedSizeAnimationSpec {
    bounds_spec: Arc<dyn FiniteAnimationSpec<IntRect>>,
}

impl DerivedSizeAnimationSpec {
    fn vectorize_into<V: AnimationVector>(
        &self,
        converter: &TwoWayConverter<IntSize, V>,
    ) -> Box<dyn VectorizedFiniteAnimationSpec<V>> {
        let (to, from) = (converter.clone(), converter.clone());
        let bounds_converter: TwoWayConverter<IntRect, V> = TwoWayConverter(
            move |bounds: IntRect| to.convert_to_vector(bounds.size()),
            move |vector: V| {
                let size = from.convert_from_vector(vector);
                IntRect { left: 0, top: 0, right: size.width, bottom: size.height }
            },
        );
        V::vectorize_finite(&*self.bounds_spec, &bounds_converter)
    }

    fn vectorize_into_any<V: AnimationVector>(
        &self,
        converter: &TwoWayConverter<IntSize, V>,
    ) -> Box<dyn VectorizedAnimationSpec<V>> {
        Box::new(AsVectorized(self.vectorize_into(converter)))
    }

    fn eq_spec(&self, other: &Self) -> bool {
        *self.bounds_spec == *other.bounds_spec
    }
}

finite_spec_impls!(DerivedSizeAnimationSpec, IntSize);

/// An offset's spec made of a bounds spec (Compose's internal `DerivedOffsetAnimationSpec`): the offset as the
/// empty bounds at it. Equal by its bounds spec.
pub(crate) struct DerivedOffsetAnimationSpec {
    bounds_spec: Arc<dyn FiniteAnimationSpec<IntRect>>,
}

impl DerivedOffsetAnimationSpec {
    fn vectorize_into<V: AnimationVector>(
        &self,
        converter: &TwoWayConverter<IntOffset, V>,
    ) -> Box<dyn VectorizedFiniteAnimationSpec<V>> {
        let (to, from) = (converter.clone(), converter.clone());
        let bounds_converter: TwoWayConverter<IntRect, V> = TwoWayConverter(
            move |bounds: IntRect| to.convert_to_vector(bounds.top_left()),
            move |vector: V| {
                let o = from.convert_from_vector(vector);
                IntRect { left: o.x, top: o.y, right: o.x, bottom: o.y }
            },
        );
        V::vectorize_finite(&*self.bounds_spec, &bounds_converter)
    }

    fn vectorize_into_any<V: AnimationVector>(
        &self,
        converter: &TwoWayConverter<IntOffset, V>,
    ) -> Box<dyn VectorizedAnimationSpec<V>> {
        Box::new(AsVectorized(self.vectorize_into(converter)))
    }

    fn eq_spec(&self, other: &Self) -> bool {
        *self.bounds_spec == *other.bounds_spec
    }
}

finite_spec_impls!(DerivedOffsetAnimationSpec, IntOffset);

/// A spring started `delayed_ratio` of its duration later (Compose's internal `DelayedSpringSpec`). Equal only to
/// itself (Compose's class has no `equals`).
pub(crate) struct DelayedSpringSpec<T> {
    original_spring_spec: SpringSpec<T>,
    delayed_ratio: f32,
}

impl<T: Clone + 'static> DelayedSpringSpec<T> {
    pub(crate) fn new(
        damping_ratio: f32,
        stiffness: f32,
        delayed_ratio: f32,
        visibility_threshold: Option<T>,
    ) -> DelayedSpringSpec<T> {
        DelayedSpringSpec {
            original_spring_spec: SpringSpec::new(damping_ratio, stiffness, visibility_threshold),
            delayed_ratio,
        }
    }

    fn vectorize_into<V: AnimationVector>(
        &self,
        converter: &TwoWayConverter<T, V>,
    ) -> Box<dyn VectorizedFiniteAnimationSpec<V>>
    where
        SpringSpec<T>: FiniteAnimationSpec<T>,
    {
        Box::new(DelayedVectorizedSpringSpec {
            original_vectorized_spring_spec: V::vectorize_finite(&self.original_spring_spec, converter),
            delayed_ratio: self.delayed_ratio,
            cache: RefCell::new(None),
        })
    }

    fn vectorize_into_any<V: AnimationVector>(
        &self,
        converter: &TwoWayConverter<T, V>,
    ) -> Box<dyn VectorizedAnimationSpec<V>>
    where
        SpringSpec<T>: FiniteAnimationSpec<T>,
    {
        Box::new(AsVectorized(self.vectorize_into(converter)))
    }

    fn eq_spec(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

finite_spec_impls!(DelayedSpringSpec<IntRect>, IntRect);

/// What a delayed spring remembers of the last start, target and velocity it was asked of.
struct DelayCache<V> {
    initial_value: V,
    target_value: V,
    initial_velocity: V,
    original_duration_nanos: i64,
    delayed_time_nanos: i64,
}

/// Compose's private `DelayedVectorizedSpringSpec`: the start value (and start velocity) until the delay, then the
/// spring from there.
struct DelayedVectorizedSpringSpec<V: AnimationVector> {
    original_vectorized_spring_spec: Box<dyn VectorizedFiniteAnimationSpec<V>>,
    delayed_ratio: f32,
    cache: RefCell<Option<DelayCache<V>>>,
}

impl<V: AnimationVector> DelayedVectorizedSpringSpec<V> {
    /// (original duration, delay) for these values, worked out again when they change.
    fn update_delayed_time_nanos_if_needed(
        &self,
        initial_value: V,
        target_value: V,
        initial_velocity: V,
    ) -> (i64, i64) {
        let mut cache = self.cache.borrow_mut();
        let stale = cache.as_ref().is_none_or(|c| {
            c.initial_value != initial_value || c.target_value != target_value || c.initial_velocity != initial_velocity
        });
        if stale {
            let original_duration_nanos =
                self.original_vectorized_spring_spec.get_duration_nanos(initial_value, target_value, initial_velocity);
            *cache = Some(DelayCache {
                initial_value,
                target_value,
                initial_velocity,
                original_duration_nanos,
                delayed_time_nanos: (original_duration_nanos as f64 * self.delayed_ratio as f64) as i64,
            });
        }
        let c = cache.as_ref().expect("set above");
        (c.original_duration_nanos, c.delayed_time_nanos)
    }
}

impl<V: AnimationVector> VectorizedAnimationSpec<V> for DelayedVectorizedSpringSpec<V> {
    fn get_value_from_nanos(&self, play_time_nanos: i64, initial_value: V, target_value: V, initial_velocity: V) -> V {
        let (_, delay) = self.update_delayed_time_nanos_if_needed(initial_value, target_value, initial_velocity);
        if play_time_nanos <= delay {
            initial_value
        } else {
            self.original_vectorized_spring_spec.get_value_from_nanos(
                play_time_nanos - delay,
                initial_value,
                target_value,
                initial_velocity,
            )
        }
    }

    fn get_velocity_from_nanos(
        &self,
        play_time_nanos: i64,
        initial_value: V,
        target_value: V,
        initial_velocity: V,
    ) -> V {
        let (_, delay) = self.update_delayed_time_nanos_if_needed(initial_value, target_value, initial_velocity);
        if play_time_nanos <= delay {
            initial_velocity
        } else {
            self.original_vectorized_spring_spec.get_velocity_from_nanos(
                play_time_nanos - delay,
                initial_value,
                target_value,
                initial_velocity,
            )
        }
    }

    fn get_duration_nanos(&self, initial_value: V, target_value: V, initial_velocity: V) -> i64 {
        let (duration, delay) = self.update_delayed_time_nanos_if_needed(initial_value, target_value, initial_velocity);
        duration + delay
    }
}

impl<V: AnimationVector> VectorizedFiniteAnimationSpec<V> for DelayedVectorizedSpringSpec<V> {}

#[allow(dead_code)]
const _: () = {
    // (Spring's defaults are the delayed spring's defaults in Compose: no bounce, medium stiffness)
    let _ = Spring::DampingRatioNoBouncy;
};
