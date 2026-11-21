//! A shape whose corners animate, as Compose's internal AnimatedShape.kt: a component's shape (a button's, a toggle
//! button's, a menu item's) going to another one corner by corner along a spec, each corner kept within half the
//! height; its optical center ([`ShapeWithHorizontalCenterOptically`]) moving with them.
//!
//! KompozR's shapes are `Send + Sync` (they are held in states and theme values), its `Animatable`s are the UI
//! thread's. So each state's corners are in two parts: what the shape reads — the shape animated from, the size and
//! density, and each corner's value as a snapshot state (Compose's `Animatable.value`, itself a state) — shared
//! with the shape ([`AnimatedCorners`], `Send + Sync`), and each corner's `Animatable`, which the state keeps on the
//! UI thread and whose value it writes into that corner's state on each frame of its animations. A corner is made
//! when the shape first reads it, as Compose's `Animatable` is, and only the corners made animate; its
//! `Animatable` keeps its velocity across animations, and a new animation interrupts the running one, as
//! Compose's.

use crate::horizontal_center_optically::{CenterOpticallyCoefficient, ShapeWithHorizontalCenterOptically};
use kompoz_animation::{Animatable, AnimationSpec, AnimationVector1D, FiniteAnimationSpec};
use kompoz_coroutines::{Channel, LaunchedEffect, coroutine_scope};
use kompoz_graphics::{
    CornerBasedShape, CornerSize, Density, LayoutDirection, Outline, RoundedCornerShape, Shape, Size,
};
use kompoz_runtime::{MutableState, SideEffect, kompozable, mutable_state_of, named_args, remember, remember_keyed};
use kompoz_ui::platform::LocalDensity;
use std::cell::RefCell;
use std::fmt;
use std::future::Future;
use std::rc::Rc;
use std::sync::{Arc, Mutex, OnceLock};

/// The corner shapes a state animates from and to: their four corners, and the shape of rounded corners of these
/// sizes the animated shape draws.
pub(crate) trait AnimatedCornerShape: Clone + Send + Sync + 'static {
    fn top_start(&self) -> &CornerSize;
    fn top_end(&self) -> &CornerSize;
    fn bottom_start(&self) -> &CornerSize;
    fn bottom_end(&self) -> &CornerSize;
    /// The shape drawn with corners of these px.
    fn with_corners(&self, top_start: f32, top_end: f32, bottom_start: f32, bottom_end: f32) -> RoundedCornerShape;
}

impl AnimatedCornerShape for RoundedCornerShape {
    fn top_start(&self) -> &CornerSize {
        &self.top_start
    }
    fn top_end(&self) -> &CornerSize {
        &self.top_end
    }
    fn bottom_start(&self) -> &CornerSize {
        &self.bottom_start
    }
    fn bottom_end(&self) -> &CornerSize {
        &self.bottom_end
    }
    /// (Compose's `state.shape.copy(…)` of the corners)
    fn with_corners(&self, top_start: f32, top_end: f32, bottom_start: f32, bottom_end: f32) -> RoundedCornerShape {
        named_args!(self.copy(
            top_start = CornerSize::px(top_start),
            top_end = CornerSize::px(top_end),
            bottom_start = CornerSize::px(bottom_start),
            bottom_end = CornerSize::px(bottom_end),
        ))
    }
}

impl AnimatedCornerShape for CornerBasedShape {
    fn top_start(&self) -> &CornerSize {
        CornerBasedShape::top_start(self)
    }
    fn top_end(&self) -> &CornerSize {
        CornerBasedShape::top_end(self)
    }
    fn bottom_start(&self) -> &CornerSize {
        CornerBasedShape::bottom_start(self)
    }
    fn bottom_end(&self) -> &CornerSize {
        CornerBasedShape::bottom_end(self)
    }
    /// (Compose's `RoundedCornerShape(topStart, topEnd, bottomStart, bottomEnd)` of the px)
    fn with_corners(&self, top_start: f32, top_end: f32, bottom_start: f32, bottom_end: f32) -> RoundedCornerShape {
        RoundedCornerShape::new(
            CornerSize::px(top_start),
            CornerSize::px(top_end),
            CornerSize::px(bottom_end),
            CornerSize::px(bottom_start),
        )
    }
}

/// The corners in their order (Compose's four `Animatable` fields).
#[derive(Clone, Copy)]
enum Corner {
    TopStart = 0,
    TopEnd = 1,
    BottomStart = 2,
    BottomEnd = 3,
}

const CORNERS: [Corner; 4] = [Corner::TopStart, Corner::TopEnd, Corner::BottomStart, Corner::BottomEnd];

impl Corner {
    fn of<S: AnimatedCornerShape>(self, shape: &S) -> &CornerSize {
        match self {
            Corner::TopStart => shape.top_start(),
            Corner::TopEnd => shape.top_end(),
            Corner::BottomStart => shape.bottom_start(),
            Corner::BottomEnd => shape.bottom_end(),
        }
    }
}

/// What the animated shape reads of its state (`Send + Sync`): the shape animated from, the size and density its
/// corners are worked out in, and each corner's value once it is made.
pub(crate) struct AnimatedCorners<S> {
    shape: S,
    size: Mutex<Size>,
    density: Mutex<Density>,
    values: [OnceLock<MutableState<f32>>; 4],
}

impl<S: AnimatedCornerShape> AnimatedCorners<S> {
    fn size(&self) -> Size {
        *self.size.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn density(&self) -> Density {
        *self.density.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The corner's value (a state read), made of the shape's corner in `size` and `density` the first time
    /// (Compose's `topStart(size, density)` and the others).
    fn value(&self, corner: Corner, size: Size, density: &Density) -> f32 {
        self.values[corner as usize].get_or_init(|| mutable_state_of(corner.of(&self.shape).to_px(size, density))).get()
    }
}

/// The state of a shape whose corners animate (Compose's internal `AnimatedShapeState` and
/// `AnimatedCornerBasedShapeState`, of a [`RoundedCornerShape`] and a [`CornerBasedShape`]): from `shape`, along
/// `spec`.
pub(crate) struct AnimatedShapeStateOf<S> {
    pub(crate) spec: Arc<dyn FiniteAnimationSpec<f32>>,
    corners: Arc<AnimatedCorners<S>>,
    /// Each corner's `Animatable` (Compose's fields), made as it first animates, of its value then.
    animatables: RefCell<[Option<Animatable<f32, AnimationVector1D>>; 4]>,
}

/// Compose's internal `AnimatedShapeState`.
pub(crate) type AnimatedShapeState = AnimatedShapeStateOf<RoundedCornerShape>;

/// Compose's internal `AnimatedCornerBasedShapeState`.
pub(crate) type AnimatedCornerBasedShapeState = AnimatedShapeStateOf<CornerBasedShape>;

impl<S: AnimatedCornerShape> AnimatedShapeStateOf<S> {
    pub(crate) fn new(shape: S, spec: Arc<dyn FiniteAnimationSpec<f32>>) -> AnimatedShapeStateOf<S> {
        AnimatedShapeStateOf {
            spec,
            corners: Arc::new(AnimatedCorners {
                shape,
                size: Mutex::new(Size::Zero),
                density: Mutex::new(Density::new(0.0, 0.0)),
                values: Default::default(),
            }),
            animatables: RefCell::new(Default::default()),
        }
    }

    /// The shape it animates from (Compose's `shape`).
    pub(crate) fn shape(&self) -> &S {
        &self.corners.shape
    }

    /// The size its corners are worked out in (Compose's `size`).
    pub(crate) fn size(&self) -> Size {
        self.corners.size()
    }

    pub(crate) fn set_size(&self, size: Size) {
        *self.corners.size.lock().unwrap_or_else(|e| e.into_inner()) = size;
    }

    /// The density its corners are worked out in (Compose's `density`).
    pub(crate) fn density(&self) -> Density {
        self.corners.density()
    }

    pub(crate) fn set_density(&self, density: Density) {
        *self.corners.density.lock().unwrap_or_else(|e| e.into_inner()) = density;
    }
}

#[kompoz_runtime::named]
impl<S: AnimatedCornerShape> AnimatedShapeStateOf<S> {
    /// The top start corner, px (Compose's `topStart(size = this.size, density = this.density)`).
    pub(crate) fn top_start(
        &self,
        #[default(self.size())] size: Size,
        #[default(self.density())] density: Density,
    ) -> f32 {
        self.corners.value(Corner::TopStart, size, &density)
    }

    /// The top end corner, px (Compose's `topEnd`).
    pub(crate) fn top_end(
        &self,
        #[default(self.size())] size: Size,
        #[default(self.density())] density: Density,
    ) -> f32 {
        self.corners.value(Corner::TopEnd, size, &density)
    }

    /// The bottom start corner, px (Compose's `bottomStart`).
    pub(crate) fn bottom_start(
        &self,
        #[default(self.size())] size: Size,
        #[default(self.density())] density: Density,
    ) -> f32 {
        self.corners.value(Corner::BottomStart, size, &density)
    }

    /// The bottom end corner, px (Compose's `bottomEnd`).
    pub(crate) fn bottom_end(
        &self,
        #[default(self.size())] size: Size,
        #[default(self.density())] density: Density,
    ) -> f32 {
        self.corners.value(Corner::BottomEnd, size, &density)
    }
}

impl<S: AnimatedCornerShape> AnimatedShapeStateOf<S> {
    /// Animates each corner made to `shape`'s in the size and density it has (Compose's suspending
    /// `animateToShape`): done when all are.
    pub(crate) fn animate_to_shape<T: AnimatedCornerShape>(&self, shape: T) -> impl Future<Output = ()> + 'static {
        let (size, density) = (self.size(), self.density());
        let spec: Arc<dyn AnimationSpec<f32>> = self.spec.clone();
        let mut animations = Vec::new();
        for corner in CORNERS {
            let Some(value) = self.corners.values[corner as usize].get().cloned() else { continue };
            let animatable = self.animatables.borrow_mut()[corner as usize]
                .get_or_insert_with(|| kompoz_animation::Animatable(value.peek()))
                .clone();
            let target = corner.of(&shape).to_px(size, &density);
            animations.push((animatable, value, target));
        }
        coroutine_scope(move |scope| async move {
            for (animatable, value, target) in animations {
                let spec = spec.clone();
                scope.launch(async move {
                    let block: Box<dyn for<'a> FnMut(&'a Animatable<f32, AnimationVector1D>)> =
                        Box::new(move |a| value.set(a.peek()));
                    named_args!(animatable.animate_to(target, animation_spec = spec, block = Some(block))).await;
                });
            }
        })
    }
}

/// A key comparing a state by its instance (Compose's `remember(…, state)`).
struct InstanceKey<T>(Rc<T>);

impl<T> PartialEq for InstanceKey<T> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// A spec compared as Compose's specs are (`remember(animationSpec)`): by what it is.
struct SpecKey(Arc<dyn FiniteAnimationSpec<f32>>);

impl PartialEq for SpecKey {
    fn eq(&self, other: &Self) -> bool {
        *self.0 == *other.0
    }
}

/// The animated shape of a state (Compose's private `object : ShapeWithHorizontalCenterOptically` of
/// `rememberAnimatedShape(state)`): its corners the state's, each kept within half its height (`clamped_range`, a
/// state of the outline's last height); equal only to itself.
pub(crate) struct AnimatedShape<S> {
    corners: Arc<AnimatedCorners<S>>,
    clamped_range: MutableState<(f32, f32)>,
}

impl<S: AnimatedCornerShape> AnimatedShape<S> {
    /// The state's corner within the clamped range (Kotlin's `coerceIn(clampedRange)`).
    fn clamped(&self, corner: Corner) -> f32 {
        let (start, end) = self.clamped_range.get();
        let value = self.corners.value(corner, self.corners.size(), &self.corners.density());
        if value < start {
            start
        } else if value > end {
            end
        } else {
            value
        }
    }
}

impl<S> PartialEq for AnimatedShape<S> {
    fn eq(&self, other: &Self) -> bool {
        self.clamped_range == other.clamped_range
    }
}

impl<S> fmt::Debug for AnimatedShape<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "AnimatedShape({:p})", Arc::as_ptr(&self.corners))
    }
}

impl<S: AnimatedCornerShape> ShapeWithHorizontalCenterOptically for AnimatedShape<S> {
    fn offset(&self) -> f32 {
        let top_start = self.clamped(Corner::TopStart);
        let top_end = self.clamped(Corner::TopEnd);
        let bottom_start = self.clamped(Corner::BottomStart);
        let bottom_end = self.clamped(Corner::BottomEnd);
        let avg_start = (top_start + bottom_start) / 2.0;
        let avg_end = (top_end + bottom_end) / 2.0;
        CenterOpticallyCoefficient * (avg_start - avg_end)
    }
}

impl<S: AnimatedCornerShape> Shape for AnimatedShape<S> {
    fn create_outline(&self, size: Size, layout_direction: LayoutDirection, density: &Density) -> Outline {
        *self.corners.size.lock().unwrap_or_else(|e| e.into_inner()) = size;

        self.clamped_range.set((0.0, size.height / 2.0));
        self.corners
            .shape
            .with_corners(
                self.clamped(Corner::TopStart),
                self.clamped(Corner::TopEnd),
                self.clamped(Corner::BottomStart),
                self.clamped(Corner::BottomEnd),
            )
            .create_outline(size, layout_direction, density)
    }
}

/// The shape of `state` (Compose's private `rememberAnimatedShape(state)`): the state given the density here, the
/// shape remembered while the density and the state stay the same.
#[kompozable]
fn remember_animated_shape_of_state<S: AnimatedCornerShape>(state: Rc<AnimatedShapeStateOf<S>>) -> Arc<dyn Shape> {
    let density = LocalDensity.current();
    state.set_density(density);

    remember_keyed((density, InstanceKey(state.clone())), move || {
        Arc::new(AnimatedShape { corners: state.corners.clone(), clamped_range: mutable_state_of((0.0, 1.0)) })
            as Arc<dyn Shape>
    })
}

/// A shape whose corners animate to `current_shape`'s along `animation_spec` (Compose's internal
/// `rememberAnimatedShape(currentShape: RoundedCornerShape, animationSpec)` and `rememberAnimatedShape(currentShape:
/// CornerBasedShape, animationSpec)`): from the first shape given (a new spec starts again from the shape then),
/// each new one sent to a conflated channel the state animates to, the latest first.
#[kompozable]
pub(crate) fn remember_animated_shape<S: AnimatedCornerShape>(
    current_shape: S,
    animation_spec: Arc<dyn FiniteAnimationSpec<f32>>,
) -> Arc<dyn Shape> {
    let state = remember_keyed(SpecKey(animation_spec.clone()), || {
        Rc::new(AnimatedShapeStateOf::new(current_shape.clone(), animation_spec))
    });

    let channel = remember(|| Rc::new(Channel::<S>::conflated()));

    let (sent, shape) = (channel.clone(), current_shape);
    SideEffect(move || {
        let _ = sent.try_send(shape);
    });
    let (s, c) = (state.clone(), channel.clone());
    LaunchedEffect(
        (InstanceKey(state.clone()), InstanceKey(channel)),
        coroutine_scope(move |scope| async move {
            while let Some(target) = c.receive().await {
                let new_target = c.try_receive().unwrap_or(target);
                scope.launch(s.animate_to_shape(new_target));
            }
        }),
    );

    remember_animated_shape_of_state(state)
}

/// `shape` as a [`CornerBasedShape`] if it is one (Kotlin's `shape is CornerBasedShape` and the cast after it): a
/// rounded or cut corner shape (absolute or not), or a [`CornerBasedShape`] itself — what the components morph
/// with [`remember_animated_shape`] (their shapes' `isCornerBasedShape`).
pub(crate) fn as_corner_based_shape(shape: &Arc<dyn Shape>) -> Option<CornerBasedShape> {
    let any = shape.shape_as_any();
    if let Some(s) = any.downcast_ref::<CornerBasedShape>() {
        return Some(s.clone());
    }
    if let Some(s) = any.downcast_ref::<RoundedCornerShape>() {
        return Some(s.clone().into());
    }
    if let Some(s) = any.downcast_ref::<kompoz_graphics::CutCornerShape>() {
        return Some(s.clone().into());
    }
    if let Some(s) = any.downcast_ref::<kompoz_graphics::AbsoluteRoundedCornerShape>() {
        return Some(s.clone().into());
    }
    any.downcast_ref::<kompoz_graphics::AbsoluteCutCornerShape>().map(|s| s.clone().into())
}
