//! Window insets as rulers (Compose's WindowInsetsRulers): the window's root provides, for each kind of system UI,
//! the rectangle it leaves free now (`current`) and would leave free were it shown (`maximum`), where it moves from
//! and to while it moves, and each display cutout's bounds; a layout reads them while placing, in its own
//! coordinates, and is placed again (nothing measured or composed) as they change.

use super::{LayoutModifierNode, PlacementScope, RectRulers, RulerScope, RulersBlock};
use crate::modifier::{Modifier, ModifierNode, ModifierNodeElement};
use crate::platform::{Insets, InsetsKind, SystemInsetsHolder};
use crate::unit::Constraints;
use crate::{Measurable, MeasureResult, MeasureScope, node_capabilities};
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

enum RulersData {
    /// A kind the platform reports: its rulers, and where it moves from and to.
    Kind {
        name: &'static str,
        kind: InsetsKind,
        current: RectRulers,
        maximum: RectRulers,
        source: RectRulers,
        target: RectRulers,
    },
    /// The innermost of others (Compose's InnermostInsetsRulers).
    Innermost { name: Option<String>, rulers: Vec<WindowInsetsRulers>, current: RectRulers, maximum: RectRulers },
}

/// The rulers of a kind of window insets (Compose's WindowInsetsRulers): `current` and `maximum` rectangles, and
/// how it moves ([`get_animation`](WindowInsetsRulers::get_animation)). Known by identity: the kinds are the same
/// rulers wherever they are asked for (on a thread).
#[derive(Clone)]
pub struct WindowInsetsRulers(Rc<RulersData>);

impl PartialEq for WindowInsetsRulers {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl fmt::Debug for WindowInsetsRulers {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &*self.0 {
            RulersData::Kind { name, .. } => write!(f, "{name}"),
            RulersData::Innermost { name: Some(name), .. } => write!(f, "{name}"),
            RulersData::Innermost { rulers, .. } => write!(f, "innermostOf({rulers:?})"),
        }
    }
}

struct Kinds {
    caption_bar: WindowInsetsRulers,
    display_cutout: WindowInsetsRulers,
    ime: WindowInsetsRulers,
    mandatory_system_gestures: WindowInsetsRulers,
    navigation_bars: WindowInsetsRulers,
    status_bars: WindowInsetsRulers,
    system_bars: WindowInsetsRulers,
    system_gestures: WindowInsetsRulers,
    tappable_element: WindowInsetsRulers,
    waterfall: WindowInsetsRulers,
    safe_drawing: WindowInsetsRulers,
    safe_gestures: WindowInsetsRulers,
    safe_content: WindowInsetsRulers,
    /// What no one provides (Compose's NeverProvidedRectRulers).
    never: RectRulers,
}

fn kind(name: &'static str, kind: InsetsKind) -> WindowInsetsRulers {
    WindowInsetsRulers(Rc::new(RulersData::Kind {
        name,
        kind,
        current: RectRulers::named(name),
        maximum: RectRulers::named(&format!("{name} maximum")),
        source: RectRulers::named(&format!("{name} source")),
        target: RectRulers::named(&format!("{name} target")),
    }))
}

fn innermost(name: Option<String>, rulers: &[WindowInsetsRulers]) -> WindowInsetsRulers {
    let current = RectRulers::innermost_of(&rulers.iter().map(|r| r.current()).collect::<Vec<_>>());
    let maximum = RectRulers::innermost_of(&rulers.iter().map(|r| r.maximum()).collect::<Vec<_>>());
    WindowInsetsRulers(Rc::new(RulersData::Innermost { name, rulers: rulers.to_vec(), current, maximum }))
}

thread_local! {
    static KINDS: Kinds = {
        let caption_bar = kind("caption bar", InsetsKind::CaptionBar);
        let display_cutout = kind("display cutout", InsetsKind::DisplayCutout);
        let ime = kind("ime", InsetsKind::Ime);
        let mandatory_system_gestures = kind("mandatory system gestures", InsetsKind::MandatorySystemGestures);
        let navigation_bars = kind("navigation bars", InsetsKind::NavigationBars);
        let status_bars = kind("status bars", InsetsKind::StatusBars);
        let system_gestures = kind("system gestures", InsetsKind::SystemGestures);
        let tappable_element = kind("tappable element", InsetsKind::TappableElement);
        let waterfall = kind("waterfall", InsetsKind::Waterfall);
        let system_bars =
            innermost(Some("system bars".into()), &[status_bars.clone(), navigation_bars.clone(), caption_bar.clone()]);
        let safe_drawing =
            innermost(Some("safe drawing".into()), &[system_bars.clone(), ime.clone(), display_cutout.clone()]);
        let safe_gestures = innermost(
            Some("safe gestures".into()),
            &[mandatory_system_gestures.clone(), system_gestures.clone(), tappable_element.clone(), waterfall.clone()],
        );
        let safe_content = innermost(Some("safe content".into()), &[safe_drawing.clone(), safe_gestures.clone()]);
        Kinds {
            caption_bar,
            display_cutout,
            ime,
            mandatory_system_gestures,
            navigation_bars,
            status_bars,
            system_bars,
            system_gestures,
            tappable_element,
            waterfall,
            safe_drawing,
            safe_gestures,
            safe_content,
            never: RectRulers::new(),
        }
    };
}

impl WindowInsetsRulers {
    /// A freeform window's title bar (Compose's `CaptionBar`).
    pub fn caption_bar() -> WindowInsetsRulers {
        KINDS.with(|k| k.caption_bar.clone())
    }
    /// The display cutout (Compose's `DisplayCutout`); each cutout's bounds: `PlacementScope::get_display_cutout_bounds`.
    pub fn display_cutout() -> WindowInsetsRulers {
        KINDS.with(|k| k.display_cutout.clone())
    }
    /// The software keyboard (Compose's `Ime`).
    pub fn ime() -> WindowInsetsRulers {
        KINDS.with(|k| k.ime.clone())
    }
    /// The edges where the system's gestures always win (Compose's `MandatorySystemGestures`).
    pub fn mandatory_system_gestures() -> WindowInsetsRulers {
        KINDS.with(|k| k.mandatory_system_gestures.clone())
    }
    /// The navigation bars (Compose's `NavigationBars`).
    pub fn navigation_bars() -> WindowInsetsRulers {
        KINDS.with(|k| k.navigation_bars.clone())
    }
    /// The status bars (Compose's `StatusBars`).
    pub fn status_bars() -> WindowInsetsRulers {
        KINDS.with(|k| k.status_bars.clone())
    }
    /// The status bars, the navigation bars and the caption bar (Compose's `SystemBars`).
    pub fn system_bars() -> WindowInsetsRulers {
        KINDS.with(|k| k.system_bars.clone())
    }
    /// The edges where swipes go to the system (Compose's `SystemGestures`).
    pub fn system_gestures() -> WindowInsetsRulers {
        KINDS.with(|k| k.system_gestures.clone())
    }
    /// What a tap reaches the system through (Compose's `TappableElement`).
    pub fn tappable_element() -> WindowInsetsRulers {
        KINDS.with(|k| k.tappable_element.clone())
    }
    /// A waterfall display's curved edges (Compose's `Waterfall`).
    pub fn waterfall() -> WindowInsetsRulers {
        KINDS.with(|k| k.waterfall.clone())
    }
    /// The system bars, the keyboard and the display cutout (Compose's `SafeDrawing`).
    pub fn safe_drawing() -> WindowInsetsRulers {
        KINDS.with(|k| k.safe_drawing.clone())
    }
    /// The gesture edges, the tappable elements and the waterfall (Compose's `SafeGestures`).
    pub fn safe_gestures() -> WindowInsetsRulers {
        KINDS.with(|k| k.safe_gestures.clone())
    }
    /// `safe_drawing` and `safe_gestures` (Compose's `SafeContent`).
    pub fn safe_content() -> WindowInsetsRulers {
        KINDS.with(|k| k.safe_content.clone())
    }

    /// The innermost of `rulers`: the innermost of their `current` and of their `maximum` rectangles (Compose's
    /// `innermostOf`).
    pub fn innermost_of(rulers: &[WindowInsetsRulers]) -> WindowInsetsRulers {
        innermost(None, rulers)
    }

    /// What the kind leaves free of the window now (Compose's `current`): the root provides every kind's, the
    /// window's edges where it covers nothing.
    pub fn current(&self) -> RectRulers {
        match &*self.0 {
            RulersData::Kind { current, .. } | RulersData::Innermost { current, .. } => current.clone(),
        }
    }

    /// What it would leave free shown, whether it is or not (Compose's `maximum`; the keyboard provides none).
    pub fn maximum(&self) -> RectRulers {
        match &*self.0 {
            RulersData::Kind { maximum, .. } | RulersData::Innermost { maximum, .. } => maximum.clone(),
        }
    }

    /// How it moves, as of where `scope` places (Compose's `getAnimation`): what is read of it (whether it is shown,
    /// moving) places the reader again when it changes.
    pub fn get_animation(&self, scope: &PlacementScope) -> WindowInsetsAnimation {
        match &*self.0 {
            RulersData::Kind { kind, source, target, .. } => {
                let Some(provider) = find_provider(scope) else { return WindowInsetsAnimation::none() };
                let holder = &provider.holder;
                match holder.animation(*kind) {
                    Some(a) => WindowInsetsAnimation {
                        source: source.clone(),
                        target: target.clone(),
                        is_visible: holder.is_visible(*kind),
                        is_animating: true,
                        fraction: a.fraction,
                        duration_millis: a.duration_millis,
                        alpha: a.alpha,
                    },
                    None => WindowInsetsAnimation {
                        source: source.clone(),
                        target: target.clone(),
                        is_visible: holder.is_visible(*kind),
                        ..WindowInsetsAnimation::none()
                    },
                }
            }
            RulersData::Innermost { rulers, .. } => {
                let animations: Vec<WindowInsetsAnimation> = rulers.iter().map(|r| r.get_animation(scope)).collect();
                WindowInsetsAnimation {
                    is_visible: animations.iter().any(|a| a.is_visible),
                    is_animating: animations.iter().any(|a| a.is_animating),
                    ..WindowInsetsAnimation::none()
                }
            }
        }
    }
}

/// How a kind of window insets moves (Compose's WindowInsetsAnimation): the rulers of where it moves from and to
/// (provided only while it moves), whether it is shown, whether it moves, how far along, how long it lasts and its
/// alpha.
#[derive(Clone, Debug, PartialEq)]
pub struct WindowInsetsAnimation {
    pub source: RectRulers,
    pub target: RectRulers,
    pub is_visible: bool,
    pub is_animating: bool,
    pub fraction: f32,
    pub duration_millis: i64,
    pub alpha: f32,
}

impl WindowInsetsAnimation {
    /// Nothing moving, shown, its rulers never provided (Compose's NoWindowInsetsAnimation).
    fn none() -> WindowInsetsAnimation {
        let never = KINDS.with(|k| k.never.clone());
        WindowInsetsAnimation {
            source: never.clone(),
            target: never,
            is_visible: true,
            is_animating: false,
            fraction: 0.0,
            duration_millis: 0,
            alpha: 1.0,
        }
    }
}

impl PlacementScope {
    /// One rectangle of rulers for each display cutout, its bounds (Compose's `getDisplayCutoutBounds`): none where
    /// the window's root provides none.
    pub fn get_display_cutout_bounds(&self) -> Vec<RectRulers> {
        let Some(provider) = find_provider(self) else { return Vec::new() };
        let count = provider.holder.cutout_rects().len();
        provider.cutout_rulers(count)
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The root's provider

/// What the window's root provides the rulers from: the host's insets, and the cutouts' rulers so far.
pub(crate) struct InsetsRulersProvider {
    holder: SystemInsetsHolder,
    cutouts: RefCell<Vec<RectRulers>>,
    block: RefCell<Option<RulersBlock>>,
}

impl InsetsRulersProvider {
    /// The first `count` cutouts' rulers (made as needed, kept).
    fn cutout_rulers(&self, count: usize) -> Vec<RectRulers> {
        let mut cutouts = self.cutouts.borrow_mut();
        while cutouts.len() < count {
            let name = format!("display cutout rect {}", cutouts.len() + 1);
            cutouts.push(RectRulers::named(&name));
        }
        cutouts[..count].to_vec()
    }

    /// The rulers' values, from the host's insets and the root's size (Compose's `provideWindowInsetsRulers`): the
    /// states read, so they are worked out again as the platform reports others.
    fn provide(&self, scope: &RulerScope) {
        let size = scope.coordinates().size();
        let (width, height) = (size.width, size.height);
        let rect = |rulers: &RectRulers, insets: Insets| {
            scope.provides(&rulers.left, insets.left as f32);
            scope.provides(&rulers.top, insets.top as f32);
            scope.provides(&rulers.right, (width - insets.right) as f32);
            scope.provides(&rulers.bottom, (height - insets.bottom) as f32);
        };
        let h = &self.holder;
        for kind in InsetsKind::All {
            let rulers = KINDS.with(|k| match kind {
                InsetsKind::StatusBars => k.status_bars.clone(),
                InsetsKind::NavigationBars => k.navigation_bars.clone(),
                InsetsKind::CaptionBar => k.caption_bar.clone(),
                InsetsKind::Ime => k.ime.clone(),
                InsetsKind::DisplayCutout => k.display_cutout.clone(),
                InsetsKind::SystemGestures => k.system_gestures.clone(),
                InsetsKind::MandatorySystemGestures => k.mandatory_system_gestures.clone(),
                InsetsKind::TappableElement => k.tappable_element.clone(),
                InsetsKind::Waterfall => k.waterfall.clone(),
            });
            let RulersData::Kind { current, maximum, source, target, .. } = &*rulers.0 else { continue };
            let now = h.insets(kind);
            rect(current, now);
            if let Some(animation) = h.animation(kind) {
                rect(source, animation.source);
                rect(target, animation.target);
            }
            // (the waterfall's maximum is what it covers; the keyboard has none)
            let most = if kind == InsetsKind::Waterfall { Some(now) } else { h.ignoring_visibility(kind) };
            if let Some(most) = most {
                rect(maximum, most);
            }
        }
        let rects = h.cutout_rects();
        for (rulers, r) in self.cutout_rulers(rects.len()).iter().zip(&rects) {
            scope.provides(&rulers.left, r.left as f32);
            scope.provides(&rulers.top, r.top as f32);
            scope.provides(&rulers.right, r.right as f32);
            scope.provides(&rulers.bottom, r.bottom as f32);
        }
    }
}

/// The provider at the root of the window `scope` places in (Compose's `findDisplayCutouts` and
/// `findInsetsAnimationProperties` walk): none outside a window's tree.
fn find_provider(scope: &PlacementScope) -> Option<Rc<InsetsRulersProvider>> {
    let coordinator = scope.coordinator.as_ref()?;
    let root = coordinator.find_root().layout_node()?;
    let mut found = None;
    for cell in root.chain_nodes() {
        let node = cell.node.borrow();
        let node: &dyn std::any::Any = &*node;
        if let Some(n) = node.downcast_ref::<WindowInsetsRulersNode>() {
            found = Some(n.provider.clone());
            break;
        }
    }
    found
}

/// The window root's modifier providing the window insets rulers from `holder` (Compose's AndroidComposeView
/// insets ruler provider).
pub(crate) fn window_insets_rulers(holder: SystemInsetsHolder) -> Modifier {
    Modifier.then_element(WindowInsetsRulersElement { holder })
}

#[derive(PartialEq)]
struct WindowInsetsRulersElement {
    holder: SystemInsetsHolder,
}

impl ModifierNodeElement for WindowInsetsRulersElement {
    type Node = WindowInsetsRulersNode;
    fn create(&self) -> WindowInsetsRulersNode {
        WindowInsetsRulersNode {
            provider: Rc::new(InsetsRulersProvider {
                holder: self.holder.clone(),
                cutouts: RefCell::new(Vec::new()),
                block: RefCell::new(None),
            }),
        }
    }
    fn update(&self, node: &mut WindowInsetsRulersNode) {
        if node.provider.holder != self.holder {
            *node = self.create();
        }
    }
    fn name(&self) -> &'static str {
        "windowInsetsRulers"
    }
}

/// Measures what it wraps as it is and provides the window insets rulers (one block, kept: worked out again only
/// when what it read changes or the root moves or is resized).
pub(crate) struct WindowInsetsRulersNode {
    provider: Rc<InsetsRulersProvider>,
}

impl ModifierNode for WindowInsetsRulersNode {
    node_capabilities!(layout);
}

impl LayoutModifierNode for WindowInsetsRulersNode {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let placeable = measurable.measure(constraints);
        let kept = self.provider.block.borrow().clone();
        let block = kept.unwrap_or_else(|| {
            let weak = Rc::downgrade(&self.provider);
            let block: RulersBlock = Rc::new(move |scope: &RulerScope| {
                if let Some(provider) = weak.upgrade() {
                    provider.provide(scope);
                }
            });
            *self.provider.block.borrow_mut() = Some(block.clone());
            block
        });
        let mut result = scope.layout(placeable.width(), placeable.height(), move |p| p.place(&placeable, 0, 0));
        result.rulers = Some(block);
        result
    }
}
