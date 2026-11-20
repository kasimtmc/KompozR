//! Material's ripple, as Compose's material3 Ripple.kt: the indication a component shows of its presses, hovers,
//! focuses and drags ([`ripple`], the ripple node of kompoz-material-ripple in the content color around it at
//! Material's alphas, [`RippleDefaults::RippleAlpha`]), and how a subtree's ripples look or whether it has any
//! ([`LocalRippleConfiguration`], [`RippleConfiguration`]). `MaterialTheme` provides the bounded one as
//! `LocalIndication`.

use crate::content_color::LocalContentColor;
use crate::tokens::StateTokens;
use kompoz_foundation::indication::{Indication, IndicationNode, IndicationNodeFactory};
use kompoz_foundation::interaction::InteractionSource;
use kompoz_graphics::{Color, ColorProducer, Dp};
use kompoz_material_ripple::{RippleAlpha, create_ripple_modifier_node};
use kompoz_runtime::{KompozitionLocal, named};
use kompoz_ui::modifier::{DelegatedNode, ObserverModifierNode};
use kompoz_ui::{ModifierNode, NodeCx, NodeHandle, node_capabilities};
use std::fmt;
use std::rc::Rc;

/// A ripple (Compose's material3 `ripple(bounded, radius, color)`): `bounded` ones spread from the press inside the
/// component, unbounded ones from its middle past it; to `radius` (unspecified: to the corners, 10 dp further
/// when bounded); in `color` (unspecified: [`LocalRippleConfiguration`]'s, else the content color around,
/// [`LocalContentColor`]) at [`LocalRippleConfiguration`]'s alphas (else [`RippleDefaults::RippleAlpha`]); none
/// where [`LocalRippleConfiguration`] gives none. What `Modifier.clickable` and the components take as their
/// `indication`.
#[named]
pub fn ripple(
    #[default(true)] bounded: bool,
    #[default(Dp::Unspecified)] radius: Dp,
    #[default(Color::Unspecified)] color: Color,
) -> Rc<dyn Indication> {
    Rc::new(RippleNodeFactory { bounded, radius, color_producer: None, color })
}

/// A ripple whose color `color` gives as it draws (Compose's material3 `ripple(color: ColorProducer, bounded,
/// radius)`): an animated color redraws it without composing it again; unspecified: as [`ripple`]'s.
///
/// Kotlin: `ripple(color: ColorProducer, bounded: Boolean = true, radius: Dp = Dp.Unspecified)`
#[named]
pub fn ripple_color_producer(
    color: ColorProducer,
    #[default(true)] bounded: bool,
    #[default(Dp::Unspecified)] radius: Dp,
) -> Rc<dyn Indication> {
    Rc::new(RippleNodeFactory { bounded, radius, color_producer: Some(color), color: Color::Unspecified })
}

/// Material's ripple alphas (Compose's material3 RippleDefaults).
pub struct RippleDefaults;

impl RippleDefaults {
    /// The state layers' opacities (Material's state tokens): pressed 0.1, focused 0.1, dragged 0.16, hovered 0.08.
    pub const RippleAlpha: RippleAlpha = RippleAlpha::new(
        StateTokens::DraggedStateLayerOpacity,
        StateTokens::FocusStateLayerOpacity,
        StateTokens::HoverStateLayerOpacity,
        StateTokens::PressedStateLayerOpacity,
    );
}

/// How the ripples below look (Compose's material3 LocalRippleConfiguration): Material's (the content color, its
/// alphas) by default, another color or alphas where provided; none (`None` provided): no ripples there (a
/// component that shows its interactions otherwise).
pub static LocalRippleConfiguration: KompozitionLocal<Option<RippleConfiguration>> =
    KompozitionLocal::new(|| Some(RippleConfiguration::new(Color::Unspecified, None)));

/// A ripple's look (Compose's material3 RippleConfiguration): its `color` (unspecified: the content color's) and
/// alphas (none: Material's, [`RippleDefaults::RippleAlpha`]).
#[derive(Clone, Copy, PartialEq)]
pub struct RippleConfiguration {
    pub color: Color,
    pub ripple_alpha: Option<RippleAlpha>,
}

impl RippleConfiguration {
    pub const fn new(color: Color, ripple_alpha: Option<RippleAlpha>) -> RippleConfiguration {
        RippleConfiguration { color, ripple_alpha }
    }
}

/// Compose's `RippleConfiguration(color = Color.Unspecified, rippleAlpha = null)`.
#[named(type)]
pub fn RippleConfiguration(
    #[default(Color::Unspecified)] color: Color,
    #[default(None)] ripple_alpha: Option<RippleAlpha>,
) -> RippleConfiguration {
    RippleConfiguration::new(color, ripple_alpha)
}

impl Default for RippleConfiguration {
    fn default() -> RippleConfiguration {
        RippleConfiguration::new(Color::Unspecified, None)
    }
}

/// Compose's toString.
impl fmt::Display for RippleConfiguration {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.ripple_alpha {
            Some(a) => write!(f, "RippleConfiguration(color={:?}, rippleAlpha={a})", self.color),
            None => write!(f, "RippleConfiguration(color={:?}, rippleAlpha=null)", self.color),
        }
    }
}

impl fmt::Debug for RippleConfiguration {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// The ripple's factory (Compose's RippleNodeFactory): equal to another of the same bounds, radius and color (its
/// producer the same one).
struct RippleNodeFactory {
    bounded: bool,
    radius: Dp,
    color_producer: Option<ColorProducer>,
    color: Color,
}

impl PartialEq for RippleNodeFactory {
    fn eq(&self, other: &Self) -> bool {
        // (as Kotlin's value classes compare: `Dp.Unspecified` and `Color.Unspecified` equal to themselves)
        self.bounded == other.bounded
            && self.radius.0.to_bits() == other.radius.0.to_bits()
            && self.color_producer == other.color_producer
            && self.color == other.color
    }
}

impl Indication for RippleNodeFactory {
    fn as_node_factory(&self) -> Option<&dyn IndicationNodeFactory> {
        Some(self)
    }
}

impl IndicationNodeFactory for RippleNodeFactory {
    fn create(&self, interaction_source: InteractionSource) -> Box<dyn ModifierNode> {
        let color = self.color;
        let color_producer = self.color_producer.clone().unwrap_or_else(|| ColorProducer::new(move || color));
        Box::new(DelegatingThemeAwareRippleNode {
            interaction_source,
            bounded: self.bounded,
            radius: self.radius,
            color: color_producer,
            ripple_node: None,
            handle: None,
        })
    }
}

/// The node of a ripple (Compose's DelegatingThemeAwareRippleNode): it delegates to a ripple node while
/// [`LocalRippleConfiguration`] gives a configuration, its color and alphas read from it and from the content
/// color as it draws.
struct DelegatingThemeAwareRippleNode {
    interaction_source: InteractionSource,
    bounded: bool,
    radius: Dp,
    color: ColorProducer,
    ripple_node: Option<DelegatedNode<IndicationNode>>,
    handle: Option<NodeHandle>,
}

impl DelegatingThemeAwareRippleNode {
    fn update_configuration(&mut self) {
        let Some(cx) = self.handle.as_ref().and_then(NodeHandle::cx) else { return };
        let configuration = cx.observe_reads(|| cx.current_value_of(&LocalRippleConfiguration));
        match configuration {
            None => self.remove_ripple(&cx),
            Some(_) if self.ripple_node.is_none() => self.attach_new_ripple(&cx),
            Some(_) => {}
        }
    }

    fn attach_new_ripple(&mut self, cx: &NodeCx) {
        let handle = cx.handle();
        let color = self.color.clone();
        let read = handle.clone();
        let calculate_color = ColorProducer::new(move || {
            let user_defined_color = color.get();
            if user_defined_color.is_specified() {
                return user_defined_color;
            }
            let Some(cx) = read.cx() else { return Color::Unspecified };
            match cx.current_value_of(&LocalRippleConfiguration) {
                Some(c) if c.color.is_specified() => c.color,
                _ => cx.current_value_of(&LocalContentColor),
            }
        });
        let calculate_ripple_alpha = move || {
            handle
                .cx()
                .and_then(|cx| cx.current_value_of(&LocalRippleConfiguration))
                .and_then(|c| c.ripple_alpha)
                .unwrap_or(RippleDefaults::RippleAlpha)
        };
        let node = create_ripple_modifier_node(
            self.interaction_source.clone(),
            self.bounded,
            self.radius,
            calculate_color,
            calculate_ripple_alpha,
        );
        self.ripple_node = Some(cx.delegate(IndicationNode::new(node)));
    }

    fn remove_ripple(&mut self, cx: &NodeCx) {
        if let Some(node) = self.ripple_node.take() {
            cx.undelegate(&node);
        }
    }
}

impl ModifierNode for DelegatingThemeAwareRippleNode {
    node_capabilities!(observer);

    fn on_attach(&mut self, cx: &NodeCx) {
        self.handle = Some(cx.handle());
        self.update_configuration();
    }
}

impl ObserverModifierNode for DelegatingThemeAwareRippleNode {
    fn on_observed_reads_changed(&mut self) {
        self.update_configuration();
    }
}
