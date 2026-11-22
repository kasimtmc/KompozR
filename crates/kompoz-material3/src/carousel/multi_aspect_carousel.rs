//! Multi-aspect carousels made of lazy lists and grids (Compose's `MultiAspectCarousel.kt`): items of their own
//! sizes masked and moved (parallax) as they enter and leave the viewport.

use super::strategy::lerp_range;
use kompoz_foundation::gestures::Orientation;
use kompoz_foundation::lazy::grid::{LazyGridItemInfo, LazyGridState};
use kompoz_foundation::lazy::{LazyListItemInfo, LazyListState};
use kompoz_foundation::prelude::*;
use kompoz_graphics::{BorderStroke, Constraints, Density, LayoutDirection, Offset, Outline, Path, Rect, Shape, Size};
use kompoz_runtime::{kompozable, kompozable_block, named, remember};
use kompoz_ui::draw::{ContentDrawScope, DrawModifierNode};
use kompoz_ui::layout::{GraphicsLayerScope, LayoutModifierNode, Measurable, MeasureResult, MeasureScope};
use kompoz_ui::{Modifier, ModifierNode, ModifierNodeElement, node_capabilities};
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

// ---------------------------------------------------------------------------------------------------------------
// MultiAspectCarouselScope

/// Composes `content` in a multi-aspect carousel scope, whose modifiers make a lazy list or grid a multi-aspect
/// carousel (Compose's composable `MultiAspectCarouselScope`). It adds no layout of its own: `content`'s layouts
/// have the parent they would have without it.
/// Kotlin: `MultiAspectCarouselScope(content: @Composable MultiAspectCarouselScope.() -> Unit)`
#[kompozable]
pub fn MultiAspectCarouselScope(content: impl Fn(&MultiAspectCarouselScope) + 'static) {
    let scope = remember(|| MultiAspectCarouselScope { _private: () });
    content(&scope);
}

/// What makes a lazy list or grid a multi-aspect carousel (Compose's interface `MultiAspectCarouselScope`, given by
/// [`MultiAspectCarouselScope()`](fn@MultiAspectCarouselScope) only — its private implementation: a value here;
/// Kotlin's member extensions `Modifier.maskClip(…)` are its methods taking the modifier first): for each item,
/// remember a [`MultiAspectCarouselItemDrawInfo`] and modify it with [`mask_clip`](Self::mask_clip) for the masking
/// and parallax as items enter and leave the scrolling container.
#[derive(Clone, PartialEq, Debug)]
pub struct MultiAspectCarouselScope {
    _private: (),
}

#[named]
impl MultiAspectCarouselScope {
    /// `modifier` clipped to `shape` and moved by the masks and parallax of `multi_aspect_item_draw_info` (Compose's
    /// `Modifier.maskClip(shape, multiAspectItemDrawInfo)`).
    ///
    /// Kotlin: `maskClip(shape: Shape, multiAspectItemDrawInfo: MultiAspectCarouselItemDrawInfo)`
    pub fn mask_clip(
        &self,
        modifier: Modifier,
        shape: Arc<dyn Shape>,
        multi_aspect_item_draw_info: MultiAspectCarouselItemDrawInfo,
    ) -> Modifier {
        modifier.then_element(MaskParallaxNodeElement { base_shape: shape, draw_info: multi_aspect_item_draw_info })
    }

    /// `modifier` with `border` drawn along `shape` within the masks of `multi_aspect_item_draw_info` (Compose's
    /// composable `Modifier.maskBorder`); before [`mask_clip`](Self::mask_clip), so the clip doesn't cut it.
    ///
    /// Compose remembers a mask shape reading the draw info as its outline is made (in the border's cache, which
    /// observes what it reads). KompozR's shapes are `Send + Sync` and the draw info reads a lazy layout's state
    /// (which isn't): the shape here is made of the draw info's values as composed, which subscribes the
    /// composition to them (it recomposes as they change; the shape remembered of the first `shape`, as Compose's).
    ///
    /// Kotlin: `maskBorder(border: BorderStroke, shape: Shape, multiAspectItemDrawInfo: MultiAspectCarouselItemDrawInfo)`
    #[track_caller]
    pub fn mask_border(
        &self,
        modifier: Modifier,
        border: BorderStroke,
        shape: Arc<dyn Shape>,
        multi_aspect_item_draw_info: MultiAspectCarouselItemDrawInfo,
    ) -> Modifier {
        kompozable_block!({
            let base_shape = remember(move || shape);
            modifier.border_stroke_with(border, MaskShape::of(base_shape, &multi_aspect_item_draw_info))
        })
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The mask and parallax node

/// Clips what it wraps to its mask shape in a layer and draws it moved by the draw info's parallax (Compose's
/// private `MaskParallaxNode`). As Compose's, an update gives the mask shape the new base shape and draw info but
/// leaves the node's own draw info (the parallax's) as created.
struct MaskParallaxNode {
    draw_info: MultiAspectCarouselItemDrawInfo,
    /// Compose's `maskShape` (its `baseShape` and `drawInfo`, which an update sets).
    mask_base_shape: Arc<dyn Shape>,
    mask_draw_info: MultiAspectCarouselItemDrawInfo,
}

impl ModifierNode for MaskParallaxNode {
    node_capabilities!(layout, draw);
}

impl LayoutModifierNode for MaskParallaxNode {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let placeable = measurable.measure(constraints);
        let (base_shape, draw_info) = (self.mask_base_shape.clone(), self.mask_draw_info.clone());
        let (width, height) = (placeable.width(), placeable.height());
        kompoz_runtime::named_args!(scope.layout(width, height, move |p| {
            let (base_shape, draw_info) = (base_shape.clone(), draw_info.clone());
            kompoz_runtime::named_args!(p.place_with_layer(
                &placeable,
                0,
                0,
                layer_block = move |layer: &mut GraphicsLayerScope| {
                    layer.clip = true;
                    // (Compose's mask shape reads the draw info as the layer makes its outline; here its values
                    // are read in the layer block, which runs again as they change: see `MaskShape`)
                    layer.shape = Arc::new(MaskShape::of(base_shape.clone(), &draw_info));
                }
            ));
        }))
    }
}

impl DrawModifierNode for MaskParallaxNode {
    fn draw(&mut self, scope: &mut ContentDrawScope) {
        let parallax = self.draw_info.parallax();
        if self.draw_info.is_horizontal() {
            scope.translate_with(parallax, 0.0, |s| s.draw_content());
        } else {
            scope.translate_with(0.0, parallax, |s| s.draw_content());
        }
    }
}

/// Compose's private `MaskParallaxNodeElement`: equal by its base shape and draw info.
struct MaskParallaxNodeElement {
    base_shape: Arc<dyn Shape>,
    draw_info: MultiAspectCarouselItemDrawInfo,
}

impl PartialEq for MaskParallaxNodeElement {
    fn eq(&self, other: &Self) -> bool {
        *self.base_shape == *other.base_shape && self.draw_info == other.draw_info
    }
}

impl ModifierNodeElement for MaskParallaxNodeElement {
    type Node = MaskParallaxNode;

    fn create(&self) -> MaskParallaxNode {
        MaskParallaxNode {
            draw_info: self.draw_info.clone(),
            mask_base_shape: self.base_shape.clone(),
            mask_draw_info: self.draw_info.clone(),
        }
    }

    fn update(&self, node: &mut MaskParallaxNode) {
        node.mask_base_shape = self.base_shape.clone();
        node.mask_draw_info = self.draw_info.clone();
    }

    fn name(&self) -> &'static str {
        "maskParallaxNodeElement"
    }
}

/// The values of a draw info a mask shape is made of.
#[derive(Clone, Copy, Debug, PartialEq)]
struct MaskValues {
    size: f32,
    is_horizontal: bool,
    mask_start: f32,
    mask_end: f32,
}

/// `base_shape` within an item's masks along the main axis (Compose's private `MaskShape`: the item's size across;
/// the whole size while the draw info's size is 0; an empty path: a rectangle of the size).
///
/// Compose's reads its draw info as each outline is made. KompozR's shapes are `Send + Sync` (held in states and
/// layers' properties) and a draw info reads a lazy layout's state, which isn't: the shape holds the draw info's
/// values as read where it is made (the layer block, run again as they change; the border's composition).
#[derive(Clone, Debug)]
struct MaskShape {
    base_shape: Arc<dyn Shape>,
    values: MaskValues,
}

impl PartialEq for MaskShape {
    fn eq(&self, other: &Self) -> bool {
        *self.base_shape == *other.base_shape && self.values == other.values
    }
}

impl MaskShape {
    fn of(base_shape: Arc<dyn Shape>, draw_info: &MultiAspectCarouselItemDrawInfo) -> MaskShape {
        MaskShape {
            base_shape,
            values: MaskValues {
                size: draw_info.size(),
                is_horizontal: draw_info.is_horizontal(),
                mask_start: draw_info.mask_start(),
                mask_end: draw_info.mask_end(),
            },
        }
    }
}

impl Shape for MaskShape {
    fn create_outline(&self, size: Size, layout_direction: LayoutDirection, density: &Density) -> Outline {
        let mut path = Path::new();
        let shape_size = size.to_rect();
        let v = self.values;
        let rect = if v.size == 0.0 {
            shape_size
        } else if v.is_horizontal {
            Rect::new(v.mask_start, shape_size.top, v.mask_end, shape_size.bottom)
        } else {
            Rect::new(shape_size.left, v.mask_start, shape_size.right, v.mask_end)
        };
        path.add_outline(&self.base_shape.create_outline(rect.size(), layout_direction, density));
        path.translate(Offset::new(rect.left, rect.top));
        path.close();

        if path.is_empty() { Outline::Rectangle(size.to_rect()) } else { Outline::Generic(path) }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// MultiAspectCarouselItemDrawInfo

/// What a multi-aspect carousel reads of its container (Compose's private `MultiAspectCarouselContainerState`).
trait MultiAspectCarouselContainerState {
    fn viewport_end_offset(&self) -> f32;
    fn viewport_start_offset(&self) -> f32;
    fn orientation(&self) -> Orientation;
}

/// What a multi-aspect carousel reads of an item (Compose's private `MultiAspectCarouselItemInfoState`).
trait MultiAspectCarouselItemInfoState {
    fn is_visible(&self) -> bool;
    fn cross_axis_size(&self) -> f32;
    fn main_axis_size(&self) -> f32;
    fn offset(&self) -> f32;
}

/// What a multi-aspect carousel item draws by, changing as it scrolls (Compose's sealed interface
/// `MultiAspectCarouselItemDrawInfo` and its implementation: a value here). Equal, as Compose's, when its index and
/// every value read now are. The values marked as changing often change at every scroll: read them where the item
/// draws or lays out, not in a composition.
#[derive(Clone)]
pub struct MultiAspectCarouselItemDrawInfo {
    index: i32,
    container_state: Rc<dyn MultiAspectCarouselContainerState>,
    item_state: Rc<dyn MultiAspectCarouselItemInfoState>,
}

/// A draw info for the item at `index` of a lazy row or column of `state` (Compose's
/// `MultiAspectCarouselItemDrawInfo(index, state: LazyListState)`): remember one per item and give it to
/// [`MultiAspectCarouselScope::mask_clip`] on the item's outermost container.
/// Kotlin: `MultiAspectCarouselItemDrawInfo(index: Int, state: LazyListState)`
pub fn MultiAspectCarouselItemDrawInfo(index: i32, state: LazyListState) -> MultiAspectCarouselItemDrawInfo {
    MultiAspectCarouselItemDrawInfo {
        index,
        container_state: Rc::new(ListContainerState(state.clone())),
        item_state: Rc::new(ListItemInfoState { state, index }),
    }
}

/// A draw info for the item at `index` of a lazy grid of `state` (Compose's `MultiAspectCarouselItemDrawInfo(index,
/// state: LazyGridState)`).
/// Kotlin: `MultiAspectCarouselItemDrawInfo(index: Int, state: LazyGridState)`
pub fn GridMultiAspectCarouselItemDrawInfo(index: i32, state: LazyGridState) -> MultiAspectCarouselItemDrawInfo {
    MultiAspectCarouselItemDrawInfo {
        index,
        container_state: Rc::new(GridContainerState(state.clone())),
        item_state: Rc::new(GridItemInfoState { state, index }),
    }
}

struct ListContainerState(LazyListState);

impl MultiAspectCarouselContainerState for ListContainerState {
    fn viewport_end_offset(&self) -> f32 {
        self.0.layout_info().viewport_end_offset() as f32
    }
    fn viewport_start_offset(&self) -> f32 {
        self.0.layout_info().viewport_start_offset() as f32
    }
    fn orientation(&self) -> Orientation {
        self.0.layout_info().orientation()
    }
}

struct ListItemInfoState {
    state: LazyListState,
    index: i32,
}

impl ListItemInfoState {
    fn get_info(&self) -> Option<LazyListItemInfo> {
        self.state.layout_info().visible_items_info().into_iter().find(|it| it.index() == self.index)
    }
}

impl MultiAspectCarouselItemInfoState for ListItemInfoState {
    fn is_visible(&self) -> bool {
        self.get_info().is_some()
    }
    fn cross_axis_size(&self) -> f32 {
        let info = self.state.layout_info();
        if info.orientation() == Orientation::Horizontal {
            info.viewport_size().height as f32
        } else {
            info.viewport_size().width as f32
        }
    }
    fn main_axis_size(&self) -> f32 {
        self.get_info().map_or(0.0, |i| i.size() as f32)
    }
    fn offset(&self) -> f32 {
        self.get_info().map_or(0.0, |i| i.offset() as f32)
    }
}

struct GridContainerState(LazyGridState);

impl MultiAspectCarouselContainerState for GridContainerState {
    fn viewport_end_offset(&self) -> f32 {
        self.0.layout_info().viewport_end_offset() as f32
    }
    fn viewport_start_offset(&self) -> f32 {
        self.0.layout_info().viewport_start_offset() as f32
    }
    fn orientation(&self) -> Orientation {
        self.0.layout_info().orientation()
    }
}

struct GridItemInfoState {
    state: LazyGridState,
    index: i32,
}

impl GridItemInfoState {
    fn get_info(&self) -> Option<LazyGridItemInfo> {
        self.state.layout_info().visible_items_info().into_iter().find(|it| it.index() == self.index)
    }

    fn is_horizontal(&self) -> bool {
        self.state.layout_info().orientation() == Orientation::Horizontal
    }
}

impl MultiAspectCarouselItemInfoState for GridItemInfoState {
    fn is_visible(&self) -> bool {
        self.get_info().is_some()
    }
    fn cross_axis_size(&self) -> f32 {
        self.get_info().map_or(0.0, |i| if self.is_horizontal() { i.size().height } else { i.size().width } as f32)
    }
    fn main_axis_size(&self) -> f32 {
        self.get_info().map_or(0.0, |i| if self.is_horizontal() { i.size().width } else { i.size().height } as f32)
    }
    fn offset(&self) -> f32 {
        self.get_info().map_or(0.0, |i| if self.is_horizontal() { i.offset().x } else { i.offset().y } as f32)
    }
}

impl MultiAspectCarouselItemDrawInfo {
    /// The item's index in its list (Compose's `index`).
    pub fn index(&self) -> i32 {
        self.index
    }

    /// Its size along the main axis now, its masks taken off (Compose's `size`; changes often).
    pub fn size(&self) -> f32 {
        self.mask_end() - self.mask_start()
    }

    /// The least it is ever masked to along the main axis (Compose's `minSize`).
    pub fn min_size(&self) -> f32 {
        get_min_size(&*self.item_state)
    }

    /// The most it is along the main axis: its whole size, unmasked (Compose's `maxSize`).
    pub fn max_size(&self) -> f32 {
        self.item_state.main_axis_size()
    }

    /// How far from its start it is masked (growing as it leaves the viewport's start, as if squeezed against the
    /// edge; Compose's `maskStart`; changes often).
    pub fn mask_start(&self) -> f32 {
        get_mask(&*self.container_state, &*self.item_state).0
    }

    /// Where along it its mask ends (shrinking as it leaves the viewport's end; Compose's `maskEnd`; changes often).
    pub fn mask_end(&self) -> f32 {
        get_mask(&*self.container_state, &*self.item_state).1
    }

    /// How far its content is moved along the main axis (against the scroll as it leaves the viewport, as if moving
    /// slower and compressed against the edge; Compose's `parallax`; changes often).
    pub fn parallax(&self) -> f32 {
        get_parallax(&*self.container_state, &*self.item_state)
    }

    /// Whether the main axis is horizontal (Compose's `isHorizontal`).
    pub fn is_horizontal(&self) -> bool {
        self.container_state.orientation() == Orientation::Horizontal
    }
}

impl PartialEq for MultiAspectCarouselItemDrawInfo {
    fn eq(&self, other: &Self) -> bool {
        if std::ptr::eq(self, other) {
            return true;
        }
        self.index == other.index
            && self.size() == other.size()
            && self.min_size() == other.min_size()
            && self.max_size() == other.max_size()
            && self.mask_start() == other.mask_start()
            && self.mask_end() == other.mask_end()
            && self.parallax() == other.parallax()
            && self.is_horizontal() == other.is_horizontal()
    }
}

impl fmt::Debug for MultiAspectCarouselItemDrawInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "MultiAspectCarouselItemDrawInfoImpl(size={},minSize={},maxSize={},maskStart={},maskEnd={},parallax={},\
             isHorizontal={},)",
            self.size(),
            self.min_size(),
            self.max_size(),
            self.mask_start(),
            self.mask_end(),
            self.parallax(),
            self.is_horizontal()
        )
    }
}

/// How far the item's content is moved: towards the end as it leaves the start (up to its size × the mask
/// intensity, when it is its size out), towards the start as it leaves the end.
fn get_parallax(
    state: &dyn MultiAspectCarouselContainerState,
    item_state: &dyn MultiAspectCarouselItemInfoState,
) -> f32 {
    if !item_state.is_visible() {
        return 0.0;
    }

    let main_axis_size = item_state.main_axis_size();
    let cross_axis_size = item_state.cross_axis_size();
    // The distance out of the viewport over which the item gets fully masked and moved: its size, so the effect
    // looks the same for items of very different sizes.
    let offscreen_threshold = main_axis_size;
    let offset = item_state.offset();

    // The mask intensity is of the item's aspect ratio.
    let mask_intensity = get_mask_intensity(main_axis_size, cross_axis_size);
    let parallax_distance = main_axis_size * mask_intensity;

    // Entering or leaving at the start.
    if offset < state.viewport_start_offset() {
        let offscreen_distance = state.viewport_start_offset() - offset;
        return lerp_range(0.0, parallax_distance, 0.0, offscreen_threshold, offscreen_distance);
    }

    // Entering or leaving at the end.
    if offset > state.viewport_end_offset() - main_axis_size {
        let offscreen_distance = -(state.viewport_end_offset() - offset - main_axis_size);
        return -lerp_range(0.0, parallax_distance, 0.0, offscreen_threshold, offscreen_distance);
    }

    0.0
}

/// The item's mask start and end: the start growing as it leaves the viewport's start (up to its size × (1 −
/// the mask intensity)), the end shrinking as it leaves the end (down to its size × the intensity).
fn get_mask(
    state: &dyn MultiAspectCarouselContainerState,
    item_state: &dyn MultiAspectCarouselItemInfoState,
) -> (f32, f32) {
    if !item_state.is_visible() {
        return (0.0, 0.0);
    }

    let main_axis_size = item_state.main_axis_size();
    let cross_axis_size = item_state.cross_axis_size();
    // (the threshold as `get_parallax`'s)
    let offscreen_threshold = main_axis_size;
    let offset = item_state.offset();

    let mask_intensity = get_mask_intensity(main_axis_size, cross_axis_size);

    // Entering or leaving at the start.
    if offset < state.viewport_start_offset() {
        let offscreen_distance = state.viewport_start_offset() - offset;
        let mask_left =
            lerp_range(0.0, main_axis_size * (1.0 - mask_intensity), 0.0, offscreen_threshold, offscreen_distance);
        return (mask_left, main_axis_size);
    }

    // Entering or leaving at the end.
    if offset > state.viewport_end_offset() - main_axis_size {
        let offscreen_distance = -(state.viewport_end_offset() - offset - main_axis_size);
        let mask_right =
            lerp_range(main_axis_size, main_axis_size * mask_intensity, 0.0, offscreen_threshold, offscreen_distance);
        return (0.0, mask_right);
    }

    (0.0, main_axis_size)
}

fn get_min_size(item_state: &dyn MultiAspectCarouselItemInfoState) -> f32 {
    if !item_state.is_visible() {
        return 0.0;
    }
    let mask_intensity = get_mask_intensity(item_state.main_axis_size(), item_state.cross_axis_size());
    item_state.main_axis_size() * (1.0 - mask_intensity)
}

/// How strongly an item is masked, by its aspect ratio: ½ over 16:9, ⅓ to ½ between 1 and 16:9, ¼ to ⅓ between
/// 9:16 and 1, else ¼.
fn get_mask_intensity(main_axis_size: f32, cross_axis_size: f32) -> f32 {
    let ar = main_axis_size / cross_axis_size;
    if ar > 16.0 / 9.0 {
        1.0 / 2.0
    } else if ar < 16.0 / 9.0 && ar > 1.0 {
        lerp_range(1.0 / 3.0, 1.0 / 2.0, 1.0, 16.0 / 9.0, ar)
    } else if ar < 1.0 && ar > 9.0 / 16.0 {
        lerp_range(1.0 / 4.0, 1.0 / 3.0, 9.0 / 16.0, 1.0, ar)
    } else {
        1.0 / 4.0
    }
}
