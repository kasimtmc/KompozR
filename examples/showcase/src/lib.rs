//! KompozR's showcase: the C++ Kompoz showcase (~/Projects/kompoz/examples/showcase) ported — its shell (main.cpp:
//! the drawer, the navigation suite, the app bar, the snackbars) and its Material screen (material/*: 27 sections,
//! one under another), in Kotlin's syntax: `.kt` files read by `kompoz_file!`, in the C++ showcase's folders and
//! files (showcase.hpp + common.cpp → `common.kt`, main.cpp → `main.kt`, material/*.cpp → `material/*.kt`).
//!
//! What Kotlin writes as classes is here in Rust until stage 14 brings Kotlin's classes: the enum classes
//! (`ColorSource`, `Spot`, `RowReveal`), the data class `Destination`, the two indications the ripple section
//! provides (Kotlin's `object … : IndicationNodeFactory`), and `onAndroid` (what the C++ checks with `__ANDROID__`).
//! The other screens (home, list, components, places) come with stage 16 and the NavHost with stage 13: until
//! then the shell's tabs are a state, and the screens other than Material say so.

#![allow(non_snake_case, non_upper_case_globals)]

use kompoz::foundation::interaction::{
    DragInteraction, FocusInteraction, HoverInteraction, Interaction, InteractionSource, PressInteraction,
};
pub mod shots;

use kompoz::foundation::indication::{Indication, IndicationNodeFactory};
use kompoz::graphics::Color;
use kompoz::graphics::vector::ImageVector;
use kompoz::material3::LocalContentColor;
use kompoz::runtime::{KotlinProperties, Saveable, SavedValue, named};
use kompoz::ui::draw::{ContentDrawScope, DrawModifierNode};
use kompoz::ui::{ModifierNode, NodeCx, NodeHandle, node_capabilities};
use std::cell::Cell;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

// Each `.kt` file in a module of its own, as Kotlin scopes a file's private declarations to it; what they declare
// otherwise is the crate's, seen from every file (Kotlin's package).
mod common {
    use super::*;
    use kompoz::prelude::*;
    kompoz::kompoz_file!("src/common.kt");
}
mod main {
    use super::*;
    use kompoz::prelude::*;
    use kompoz::ui::input::NestedScrollConnection;
    use kompoz::ui::strings::{LocalStrings, Strings};
    kompoz::kompoz_file!("src/main.kt");
}
mod material {
    use super::*;
    use kompoz::prelude::*;
    kompoz::kompoz_file!("src/material/material.kt");
}
mod sections {
    use super::*;
    use kompoz::prelude::*;
    use kompoz::ui::focus::FocusDirection;
    use kompoz::ui::focus::FocusRequester;
    use kompoz::ui::platform::FrameRateCategory;
    use kompoz::ui::semantics::CustomAccessibilityAction;
    use kompoz::ui::semantics::LiveRegionMode;
    use kompoz::ui::window::PopupPositionProvider;
    kompoz::kompoz_file!("src/material/sections.kt");
}
mod text_fields {
    use super::*;
    use kompoz::foundation::text::selection::TextSelectionColors;
    use kompoz::prelude::*;
    use kompoz::text::intl::Locale;
    use kompoz::ui::focus::FocusRequester;
    kompoz::kompoz_file!("src/material/text_fields.kt");
}
mod controls {
    use super::*;
    use kompoz::prelude::*;
    use kompoz::ui::semantics::ToggleableState;
    kompoz::kompoz_file!("src/material/controls.kt");
}
mod containers {
    use super::*;
    use kompoz::prelude::*;
    kompoz::kompoz_file!("src/material/containers.kt");
}
mod navigation {
    use super::*;
    use kompoz::prelude::*;
    use kompoz::ui::back::BackHandler;
    use kompoz::ui::input::NestedScrollConnection;
    kompoz::kompoz_file!("src/material/navigation.kt");
}
pub use common::*;
pub use containers::*;
pub use controls::*;
pub use main::*;
pub use material::*;
pub use navigation::*;
pub use sections::*;
pub use text_fields::*;

/// Where the app runs, for what differs (a phone has no hardware keyboard): the C++ showcase's `kOnAndroid`.
pub const on_android: bool = cfg!(target_os = "android");

/// Where the theme's colors come from: the app's own scheme, the system's accent color, or the wallpaper. The
/// library offers all three; the app (here its user) picks. (Kotlin: `enum class ColorSource { App, System,
/// Wallpaper }`, saved as Android saves an enum: by its name.)
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ColorSource {
    App,
    System,
    Wallpaper,
}

impl Saveable for ColorSource {
    fn save(&self) -> SavedValue {
        SavedValue::String(format!("{self:?}"))
    }
    fn restore(saved: &SavedValue) -> Option<ColorSource> {
        match saved.as_str()? {
            "App" => Some(ColorSource::App),
            "System" => Some(ColorSource::System),
            "Wallpaper" => Some(ColorSource::Wallpaper),
            _ => None,
        }
    }
}

/// The handle's three places (Kotlin: `enum class Spot { Start, Middle, End }`; its `name` is its Debug).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Spot {
    Start,
    Middle,
    End,
}

/// A row swiped open to its actions, or not (Kotlin: `enum class RowReveal { Closed, Open }`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum RowReveal {
    Closed,
    Open,
}

impl fmt::Display for Spot {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl fmt::Display for RowReveal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

/// A place to go: its label and icon (the navigation bar's and rail's items, the FAB menu's actions). Kotlin:
/// `data class Destination(val label: String, val icon: ImageVector)`.
#[derive(Clone, PartialEq, Debug, KotlinProperties)]
pub struct Destination {
    pub label: String,
    pub icon: ImageVector,
}

/// `Destination(label, icon)`.
#[named(type)]
pub fn Destination(label: impl Into<String>, icon: ImageVector) -> Destination {
    Destination { label: label.into(), icon }
}

/// The name of what a user did (the interaction section's log): what Kotlin tells apart with `when (i) { is
/// PressInteraction.Press -> "Press" … }` (`is` comes with stage 14); "" for an interaction of another kind.
pub fn interaction_name(i: Arc<dyn Interaction>) -> String {
    let i = &*i;
    let name = if i.is::<PressInteraction::Press>() {
        "Press"
    } else if i.is::<PressInteraction::Release>() {
        "Release"
    } else if i.is::<PressInteraction::Cancel>() {
        "Cancel"
    } else if i.is::<HoverInteraction::Enter>() {
        "HoverEnter"
    } else if i.is::<HoverInteraction::Exit>() {
        "HoverExit"
    } else if i.is::<FocusInteraction::Focus>() {
        "Focus"
    } else if i.is::<FocusInteraction::Unfocus>() {
        "Unfocus"
    } else if i.is::<DragInteraction::Start>() {
        "DragStart"
    } else if i.is::<DragInteraction::Stop>() {
        "DragStop"
    } else if i.is::<DragInteraction::Cancel>() {
        "DragCancel"
    } else {
        ""
    };
    name.to_string()
}

/// The ripple section's flat state layer: instead of the ripple, the content color laid over the component, at
/// Material's state-layer alphas (0.16 pressed, 0.10 focused, 0.08 hovered). Kotlin: an `object FlatStateLayer :
/// IndicationNodeFactory` whose node draws the overlay from the interactions it collects.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FlatStateLayerIndication;

thread_local! {
    /// The flat state layer, as Kotlin names its object.
    pub static FlatStateLayer: Rc<dyn Indication> = Rc::new(FlatStateLayerIndication);
    /// No indication at all for a subtree (`LocalIndication provides NoIndication`): Kotlin's `object NoIndication :
    /// IndicationNodeFactory` whose node draws only the content.
    pub static NoIndication: Rc<dyn Indication> = Rc::new(NoIndicationKind);
}

impl Indication for FlatStateLayerIndication {
    fn as_node_factory(&self) -> Option<&dyn IndicationNodeFactory> {
        Some(self)
    }
}

impl IndicationNodeFactory for FlatStateLayerIndication {
    fn create(&self, interaction_source: InteractionSource) -> Box<dyn ModifierNode> {
        Box::new(FlatStateLayerNode { interaction_source, alpha: Rc::new(Cell::new(0.0)), handle: None })
    }
}

/// The node of [`FlatStateLayerIndication`]: it counts its source's presses, focuses and hovers, and draws the
/// overlay of the strongest.
struct FlatStateLayerNode {
    interaction_source: InteractionSource,
    alpha: Rc<Cell<f32>>,
    /// Its place, for the content color where it is drawn.
    handle: Option<NodeHandle>,
}

impl ModifierNode for FlatStateLayerNode {
    node_capabilities!(draw);

    fn on_attach(&mut self, cx: &NodeCx) {
        self.handle = Some(cx.handle());
        let (flow, alpha, handle) = (self.interaction_source.interactions(), self.alpha.clone(), cx.handle());
        let (mut presses, mut focuses, mut hovers) = (0, 0, 0);
        cx.coroutine_scope().launch(flow.collect(move |interaction: Arc<dyn Interaction>| {
            let i = &*interaction;
            if i.is::<PressInteraction::Press>() {
                presses += 1;
            } else if i.is::<PressInteraction::Release>() || i.is::<PressInteraction::Cancel>() {
                presses -= 1;
            } else if i.is::<FocusInteraction::Focus>() {
                focuses += 1;
            } else if i.is::<FocusInteraction::Unfocus>() {
                focuses -= 1;
            } else if i.is::<HoverInteraction::Enter>() {
                hovers += 1;
            } else if i.is::<HoverInteraction::Exit>() {
                hovers -= 1;
            }
            let shown = if presses > 0 {
                0.16
            } else if focuses > 0 {
                0.10
            } else if hovers > 0 {
                0.08
            } else {
                0.0
            };
            if alpha.replace(shown) != shown {
                handle.invalidate_draw();
            }
        }));
    }
}

impl DrawModifierNode for FlatStateLayerNode {
    fn draw(&mut self, scope: &mut ContentDrawScope) {
        scope.draw_content();
        let alpha = self.alpha.get();
        if alpha > 0.0
            && let Some(cx) = self.handle.as_ref().and_then(NodeHandle::cx)
        {
            let color = cx.current_value_of(&LocalContentColor);
            scope.draw_rect(Color::new(color.red(), color.green(), color.blue(), alpha));
        }
    }
}

/// [`NoIndication`]'s kind: a node factory (what clickables take from LocalIndication) whose nodes draw the content
/// alone.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NoIndicationKind;

impl Indication for NoIndicationKind {
    fn as_node_factory(&self) -> Option<&dyn IndicationNodeFactory> {
        Some(self)
    }
}

impl IndicationNodeFactory for NoIndicationKind {
    fn create(&self, _: InteractionSource) -> Box<dyn ModifierNode> {
        Box::new(NoIndicationNode)
    }
}

/// The node of [`NoIndicationKind`]: the content as it is.
struct NoIndicationNode;

impl ModifierNode for NoIndicationNode {
    node_capabilities!(draw);
}

impl DrawModifierNode for NoIndicationNode {
    fn draw(&mut self, scope: &mut ContentDrawScope) {
        scope.draw_content();
    }
}
