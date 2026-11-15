//! `Modifier.magnifier`, as Compose's Magnifier: the content around a point shown zoomed in a rounded loupe above
//! it. Compose hands the loupe to the platform (Android's Magnifier widget, a window of its own); here the node
//! draws it, after its content, through a layer of what it recorded of it (C++ Kompoz drew the scene again from the
//! host): what its layout draws is what it magnifies.

// (Compose's parameter lists: the methods of a `#[named]` trait carry no attributes of their own)
#![allow(clippy::too_many_arguments)]

use kompoz_graphics::{Color, Density, Dp, DpSize, IntSize, Offset, Rect, Size};
use kompoz_ui::draw::{ContentDrawScope, DrawModifierNode, GraphicsLayer};
use kompoz_ui::graphics_context::{CornerRadii, LayerTransform, Shadow};
use kompoz_ui::layout::{GlobalPositionAwareModifierNode, LayoutCoordinates};
use kompoz_ui::modifier::ObserverModifierNode;
use kompoz_ui::platform::LocalPlatform;
use kompoz_ui::semantics::{SemanticsConfiguration, SemanticsModifierNode, SemanticsPropertyKey};
use kompoz_ui::{Modifier, ModifierNode, ModifierNodeElement, NodeCx, NodeHandle, node_capabilities};
use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::Rc;

/// The zoom of a magnifier that says none (Android's default).
pub const DefaultMagnifierZoom: f32 = 1.25;
/// The size of a magnifier that says none (Android's default: 100 by 48 dp).
pub const DefaultMagnifierSize: DpSize = DpSize { width: Dp(100.0), height: Dp(48.0) };
/// The corner radius of a magnifier that says none (Android's text magnifier's).
pub const DefaultMagnifierCornerRadius: Dp = Dp(24.0);
/// The elevation of a magnifier that says none (Android's default).
pub const DefaultMagnifierElevation: Dp = Dp(4.0);
/// How far above the source a magnifier that says nowhere is (Android's default vertical offset).
pub const DefaultMagnifierVerticalOffset: Dp = Dp(42.0);

/// Where a magnified point is in the root, as accessibility and tests read it (Compose's
/// `MagnifierPositionInRoot`): unspecified while nothing is shown.
#[derive(Clone)]
pub struct MagnifierPosition(pub Rc<dyn Fn() -> Offset>);

impl PartialEq for MagnifierPosition {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl fmt::Debug for MagnifierPosition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "MagnifierPosition({:?})", (self.0)())
    }
}

/// The semantics property of a magnifier's source center in the root (Compose's internal
/// `MagnifierPositionInRoot`).
pub const MagnifierPositionInRoot: SemanticsPropertyKey<MagnifierPosition> =
    SemanticsPropertyKey::new("MagnifierPositionInRoot");

type CenterFn = Rc<dyn Fn(&Density) -> Offset>;
type SizeChanged = Rc<dyn Fn(DpSize)>;

/// The magnifier modifier of foundation.
#[kompoz_runtime::named(trait MagnifierModifiers)]
impl Modifier {
    /// Shows the content around `source_center` (px in the layout; unspecified: nothing shown) zoomed `zoom`
    /// times in a loupe of `size`, rounded by `corner_radius`, raised by `elevation`, centered at
    /// `magnifier_center` (px in the layout; none or unspecified: 42 dp above the source), kept in the window when
    /// `clip` (Compose's `Modifier.magnifier`); `on_size_changed` is told its size when it is first shown and as
    /// it changes. Unspecified values take the defaults (`DefaultMagnifier…`). The centers are read when worked
    /// out, observed: a state they read moves it.
    fn magnifier(
        self,
        source_center: impl Fn(&Density) -> Offset + 'static,
        #[default(None)] magnifier_center: Option<CenterFn>,
        #[default(None)] on_size_changed: Option<SizeChanged>,
        #[default(f32::NAN)] zoom: f32,
        #[default(DpSize::Unspecified)] size: DpSize,
        #[default(Dp::Unspecified)] corner_radius: Dp,
        #[default(Dp::Unspecified)] elevation: Dp,
        #[default(true)] clip: bool,
    ) -> Modifier {
        self.magnifier_with(
            source_center,
            magnifier_center,
            on_size_changed,
            zoom,
            size,
            corner_radius,
            elevation,
            clip,
        )
    }

    /// [`magnifier`](MagnifierModifiers::magnifier) with every parameter, for plain code.
    fn magnifier_with(
        self,
        source_center: impl Fn(&Density) -> Offset + 'static,
        magnifier_center: Option<CenterFn>,
        on_size_changed: Option<SizeChanged>,
        zoom: f32,
        size: DpSize,
        corner_radius: Dp,
        elevation: Dp,
        clip: bool,
    ) -> Modifier {
        self.then_element(MagnifierElement {
            settings: Settings {
                source_center: Rc::new(source_center),
                magnifier_center,
                on_size_changed,
                zoom,
                size,
                corner_radius,
                elevation,
                clip,
            },
        })
    }
}

/// What a magnifier is asked for.
#[derive(Clone)]
struct Settings {
    source_center: CenterFn,
    magnifier_center: Option<CenterFn>,
    on_size_changed: Option<SizeChanged>,
    zoom: f32,
    size: DpSize,
    corner_radius: Dp,
    elevation: Dp,
    clip: bool,
}

impl PartialEq for Settings {
    fn eq(&self, o: &Self) -> bool {
        let same_fn = |a: &Option<CenterFn>, b: &Option<CenterFn>| match (a, b) {
            (Some(a), Some(b)) => Rc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        let same_size_fn = match (&self.on_size_changed, &o.on_size_changed) {
            (Some(a), Some(b)) => Rc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        Rc::ptr_eq(&self.source_center, &o.source_center)
            && same_fn(&self.magnifier_center, &o.magnifier_center)
            && same_size_fn
            && (self.zoom == o.zoom || (self.zoom.is_nan() && o.zoom.is_nan()))
            && same_dp_size(self.size, o.size)
            && same_dp(self.corner_radius, o.corner_radius)
            && same_dp(self.elevation, o.elevation)
            && self.clip == o.clip
    }
}

/// Equal, unspecified ones too (Compose's `equalsIncludingNaN`).
fn same_dp(a: Dp, b: Dp) -> bool {
    a == b || (a.is_unspecified() && b.is_unspecified())
}

fn same_dp_size(a: DpSize, b: DpSize) -> bool {
    same_dp(a.width, b.width) && same_dp(a.height, b.height)
}

/// `Modifier.magnifier`'s element (Compose's MagnifierElement).
#[derive(PartialEq)]
struct MagnifierElement {
    settings: Settings,
}

impl ModifierNodeElement for MagnifierElement {
    type Node = MagnifierNode;
    fn create(&self) -> MagnifierNode {
        MagnifierNode {
            state: Rc::new(MagnifierState {
                settings: RefCell::new(self.settings.clone()),
                layout_coordinates: RefCell::new(None),
                source_center_in_root: Cell::new(Offset::Unspecified),
                shown: Cell::new(None),
                previous_size: Cell::new(None),
                handle: RefCell::new(None),
            }),
            layer: None,
        }
    }
    fn update(&self, node: &mut MagnifierNode) {
        *node.state.settings.borrow_mut() = self.settings.clone();
        node.state.update_magnifier();
    }
    fn name(&self) -> &'static str {
        "magnifier"
    }
}

/// Where the loupe is drawn, in the layout: the source and the loupe's center, px.
#[derive(Clone, Copy, PartialEq, Debug)]
struct Shown {
    source: Offset,
    center: Offset,
    zoom: f32,
    size: Size,
    corner_radius: f32,
    elevation: f32,
}

/// What a magnifier node and its semantics action share.
struct MagnifierState {
    settings: RefCell<Settings>,
    layout_coordinates: RefCell<Option<LayoutCoordinates>>,
    source_center_in_root: Cell<Offset>,
    shown: Cell<Option<Shown>>,
    previous_size: Cell<Option<IntSize>>,
    handle: RefCell<Option<NodeHandle>>,
}

impl MagnifierState {
    fn handle(&self) -> Option<NodeHandle> {
        self.handle.borrow().clone()
    }

    /// Works out where it is shown, what that reads observed (Compose's `onObservedReadsChanged`).
    fn update_magnifier(&self) {
        let Some(handle) = self.handle() else { return };
        let shown = handle.observe_reads(|| self.work_out(&handle.density()));
        if self.shown.replace(shown) != shown {
            handle.invalidate_draw();
        }
        if let Some(s) = shown {
            self.update_size_if_necessary(&handle.density(), s.size);
        }
    }

    /// Compose's `updateMagnifier`: shown where the source and the layout are known, hidden otherwise.
    fn work_out(&self, density: &Density) -> Option<Shown> {
        let settings = self.settings.borrow().clone();
        let source = (settings.source_center)(density);
        let coordinates = self.layout_coordinates.borrow().clone().filter(|c| c.is_attached());
        let (Some(coordinates), true) = (coordinates, source.is_specified()) else {
            self.source_center_in_root.set(Offset::Unspecified);
            return None;
        };
        let anchor = coordinates.position_in_root();
        self.source_center_in_root.set(anchor + source);
        let zoom = if settings.zoom.is_nan() { DefaultMagnifierZoom } else { settings.zoom.max(0.01) };
        let size = density.size_to_px(if settings.size.is_specified() { settings.size } else { DefaultMagnifierSize });
        let corner_radius = density.to_px(settings.corner_radius.take_or_else(|| DefaultMagnifierCornerRadius));
        let elevation = density.to_px(settings.elevation.take_or_else(|| DefaultMagnifierElevation));
        let mut center = settings
            .magnifier_center
            .as_ref()
            .map(|f| f(density))
            .filter(|c| c.is_specified())
            .unwrap_or_else(|| source - Offset::new(0.0, density.to_px(DefaultMagnifierVerticalOffset)));
        if settings.clip {
            // Kept in the window (Android's clipping): the loupe moved in where it would go past an edge.
            let window = coordinates.find_root_coordinates().size().to_size();
            let in_root = anchor + center;
            let (half_w, half_h) = (size.width / 2.0, size.height / 2.0);
            let x = in_root.x.clamp(half_w, (window.width - half_w).max(half_w));
            let y = in_root.y.clamp(half_h, (window.height - half_h).max(half_h));
            center = Offset::new(x, y) - anchor;
        }
        Some(Shown { source, center, zoom, size, corner_radius, elevation })
    }

    /// Tells its size when it changes (Compose's `updateSizeIfNecessary`).
    fn update_size_if_necessary(&self, density: &Density, size: Size) {
        let int_size = IntSize::new(size.width.round() as i32, size.height.round() as i32);
        if self.previous_size.get() == Some(int_size) {
            return;
        }
        self.previous_size.set(Some(int_size));
        let callback = self.settings.borrow().on_size_changed.clone();
        if let Some(f) = callback {
            f(density.size_to_dp(int_size.to_size()));
        }
    }
}

/// The node of `Modifier.magnifier` (Compose's MagnifierNode): it follows where its layout is, works out where the
/// loupe goes as what it reads changes, and draws it over its content.
pub struct MagnifierNode {
    state: Rc<MagnifierState>,
    /// What its content draws, recorded each frame (drawn, then drawn again zoomed).
    layer: Option<GraphicsLayer>,
}

impl ModifierNode for MagnifierNode {
    node_capabilities!(draw, global_position_aware, semantics, observer);

    fn should_auto_invalidate(&self) -> bool {
        false
    }

    fn on_attach(&mut self, cx: &NodeCx) {
        *self.state.handle.borrow_mut() = Some(cx.handle());
        self.layer = Some(GraphicsLayer::new(Some(cx.current_value_of(&LocalPlatform))));
        self.state.update_magnifier();
    }

    fn on_detach(&mut self, _cx: &NodeCx) {
        self.state.shown.set(None);
        if let Some(layer) = self.layer.take() {
            layer.release();
        }
    }
}

impl ObserverModifierNode for MagnifierNode {
    fn on_observed_reads_changed(&mut self) {
        let state = self.state.clone();
        kompoz_ui::input::when_node_released(move || state.update_magnifier());
    }
}

impl GlobalPositionAwareModifierNode for MagnifierNode {
    /// Where its layout is (Compose's `onGloballyPositioned`): the loupe worked out again.
    fn on_globally_positioned(&mut self, coordinates: &LayoutCoordinates) {
        *self.state.layout_coordinates.borrow_mut() = Some(coordinates.clone());
        let state = self.state.clone();
        kompoz_ui::input::when_node_released(move || state.update_magnifier());
    }
}

impl SemanticsModifierNode for MagnifierNode {
    fn apply_semantics(&mut self, config: &mut SemanticsConfiguration) {
        let state = self.state.clone();
        config.set(MagnifierPositionInRoot, MagnifierPosition(Rc::new(move || state.source_center_in_root.get())));
    }
}

impl DrawModifierNode for MagnifierNode {
    fn draw(&mut self, scope: &mut ContentDrawScope) {
        let (Some(shown), Some(layer)) = (self.state.shown.get(), self.layer.as_ref()) else {
            scope.draw_content();
            return;
        };
        let size = scope.size();
        let int_size = IntSize::new(size.width.round() as i32, size.height.round() as i32);
        scope.record_with(layer, int_size, |s| s.draw_content());
        scope.draw_layer(layer);
        let Some(picture) = layer.picture() else { return };
        let origin = scope.origin();
        let loupe = centered(origin + shown.center, shown.size);
        let context = scope.context();
        if shown.elevation > 0.0 {
            let shadow = Shadow::new(
                Color::new(0.0, 0.0, 0.0, 0.3),
                Offset::new(0.0, shown.elevation / 2.0),
                shown.elevation * 2.0,
                0.0,
            );
            context.draw_shadow(loupe, CornerRadii::all(shown.corner_radius), &shadow, false);
        }
        // The source's surroundings, scaled about their middle and moved to the loupe.
        let around =
            centered(origin + shown.source, Size::new(loupe.width() / shown.zoom, loupe.height() / shown.zoom));
        let transform = LayerTransform {
            scale_x: shown.zoom,
            scale_y: shown.zoom,
            translation_x: shown.center.x - shown.source.x,
            translation_y: shown.center.y - shown.source.y,
            ..LayerTransform::Identity
        };
        context.push_layer(around, CornerRadii::all(shown.corner_radius / shown.zoom), 1.0, transform);
        context.push_translation(origin);
        picture.replay(context);
        context.pop_translation();
        context.pop_layer();
    }
}

/// A rect of `size` centered at `center`.
fn centered(center: Offset, size: Size) -> Rect {
    Rect::from_offset_size(center - Offset::new(size.width / 2.0, size.height / 2.0), size)
}
