//! `Popup` (Compose's Popup.kt and AndroidPopup's PopupProperties): content shown above the window, placed by a
//! position provider from the bounds of the layout it is composed in.

use super::{PopupKind, PopupSpec};
use crate::alignment::Alignment;
use crate::back::{BackScope, LocalBackScope};
use crate::input::KeyHandler;
use crate::layout::{Measurable, MeasurePolicy, MeasureResult, MeasureScope, Placeable};
use crate::modifier::Modifier;
use crate::node::LayoutNode;
use crate::platform::{LocalDensity, LocalLayoutDirection};
use crate::unit::Constraints;
use kompoz_graphics::{IntOffset, IntRect, IntSize, LayoutDirection};
use kompoz_runtime::{
    KompozitionLocalProvider, current_composite_key_hash, current_kompozition_local_map, emit, kompozable,
    kompozable_block, remember,
};
use std::rc::Rc;

/// What a popup does (Compose's PopupProperties): whether it takes the focus, keys and back while on top (else
/// input outside it passes through), whether back or a press outside dismisses it, whether it is kept in the
/// window, whether its width is the platform's default (the window's here, as without).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct PopupProperties {
    pub focusable: bool,
    pub dismiss_on_back_press: bool,
    pub dismiss_on_click_outside: bool,
    pub clipping_enabled: bool,
    pub use_platform_default_width: bool,
}

impl Default for PopupProperties {
    fn default() -> Self {
        PopupProperties {
            focusable: false,
            dismiss_on_back_press: true,
            dismiss_on_click_outside: true,
            clipping_enabled: true,
            use_platform_default_width: false,
        }
    }
}

/// Compose's `PopupProperties(focusable = false, dismissOnBackPress = true, dismissOnClickOutside = true,
/// clippingEnabled = true, usePlatformDefaultWidth = false)`, named arguments in a kompozable body (and Kotlin's
/// syntax); plain code writes the struct.
#[kompoz_runtime::named(type)]
pub fn PopupProperties(
    #[default(false)] focusable: bool,
    #[default(true)] dismiss_on_back_press: bool,
    #[default(true)] dismiss_on_click_outside: bool,
    #[default(true)] clipping_enabled: bool,
    #[default(false)] use_platform_default_width: bool,
) -> PopupProperties {
    PopupProperties {
        focusable,
        dismiss_on_back_press,
        dismiss_on_click_outside,
        clipping_enabled,
        use_platform_default_width,
    }
}

/// Where a popup goes (Compose's PopupPositionProvider): from its anchor's bounds in the window (the layout it is
/// composed in), the window's size, the layout direction and its own size, its top left in the window, px.
pub trait PopupPositionProvider {
    fn calculate_position(
        &self,
        anchor_bounds: IntRect,
        window_size: IntSize,
        layout_direction: LayoutDirection,
        popup_content_size: IntSize,
    ) -> IntOffset;
}

/// Places a popup at a point of its anchor's bounds `alignment` gives, aligned to it the same way, moved by
/// `offset` (mirrored right to left) (Compose's AlignmentOffsetPositionProvider).
#[derive(Clone, PartialEq, Debug)]
pub struct AlignmentOffsetPositionProvider {
    pub alignment: Alignment,
    pub offset: IntOffset,
}

impl PopupPositionProvider for AlignmentOffsetPositionProvider {
    fn calculate_position(
        &self,
        anchor_bounds: IntRect,
        _window_size: IntSize,
        layout_direction: LayoutDirection,
        popup_content_size: IntSize,
    ) -> IntOffset {
        let anchor_point = self.alignment.align(IntSize::Zero, anchor_bounds.size(), layout_direction);
        let popup_point = self.alignment.align(IntSize::Zero, popup_content_size, layout_direction);
        let x_sign = if layout_direction == LayoutDirection::Ltr { 1 } else { -1 };
        IntOffset::new(
            anchor_bounds.left + anchor_point.x - popup_point.x + self.offset.x * x_sign,
            anchor_bounds.top + anchor_point.y - popup_point.y + self.offset.y,
        )
    }
}

/// Content shown above the window (Compose's `Popup`): placed by `popup_position_provider` if given, else at the
/// point of the anchor's bounds `alignment` gives, aligned to it, moved by `offset`. `on_dismiss_request` runs on
/// back (a focusable popup) and on a press outside, as `properties` say. Composed where it is called, so it gets
/// the locals there; its content has the IsPopup semantics. A focusable popup's keys go to
/// `on_preview_key_event` before its content, and to `on_key_event` after it (Compose for desktop's).
#[kompozable]
pub fn Popup(
    #[default(Alignment::TopStart)] alignment: Alignment,
    #[default(IntOffset::Zero)] offset: IntOffset,
    #[default(None)] popup_position_provider: Option<Rc<dyn PopupPositionProvider>>,
    #[default(|| {})] on_dismiss_request: impl Fn() + 'static,
    #[default(PopupProperties::default())] properties: PopupProperties,
    #[default(None)] on_preview_key_event: Option<KeyHandler>,
    #[default(None)] on_key_event: Option<KeyHandler>,
    content: impl Fn() + 'static,
) {
    let provider = match popup_position_provider {
        Some(p) => p,
        None => Rc::new(AlignmentOffsetPositionProvider { alignment, offset }),
    };
    let back_scope = remember(BackScope::new);
    let spec = Rc::new(PopupSpec {
        kind: PopupKind::Popup { provider, properties },
        on_dismiss_request: Rc::new(on_dismiss_request),
        back_scope: back_scope.clone(),
        layout_direction: LocalLayoutDirection.current(),
    });
    let scope = properties.focusable.then_some(back_scope);
    emit_popup_root(
        spec,
        scope,
        stack_policy(),
        Modifier.semantics(|c| c.popup()),
        (on_preview_key_event, on_key_event),
        content,
    );
}

/// Emits a popup's root: its spec, the window root's modifiers (its own root focus target and key node), its
/// `root_policy` (measured in the window), then `content` in a stack with `content_modifier`, the back scope
/// provided to it (a focusable one's).
#[track_caller]
pub(crate) fn emit_popup_root(
    spec: Rc<PopupSpec>,
    back_scope: Option<BackScope>,
    root_policy: Rc<dyn MeasurePolicy>,
    content_modifier: Modifier,
    key_listeners: (Option<KeyHandler>, Option<KeyHandler>),
    content: impl FnOnce(),
) {
    let density = LocalDensity.current();
    let layout_direction = LocalLayoutDirection.current();
    let locals = current_kompozition_local_map();
    let window_root = crate::focus::window_root_modifier_with_keys(key_listeners.0, key_listeners.1)
        .then(crate::layout::window_insets_rulers(crate::platform::LocalSystemInsets.current()));
    let key = current_composite_key_hash();
    emit(
        LayoutNode::new,
        move |node: &LayoutNode| {
            node.set_popup_spec(spec);
            node.set_measure_policy(root_policy);
            node.set_density(density);
            node.set_layout_direction(layout_direction);
            node.set_locals(locals);
            node.modifier_set(window_root);
            node.set_composite_key(key);
            node.note_composed();
        },
        move || {
            let values = [LocalBackScope.provides(back_scope)];
            kompozable_block!({
                KompozitionLocalProvider(&values, move || {
                    crate::layout::layout_into(content_modifier, stack_policy(), false, content)
                })
            })
        },
    );
}

thread_local! {
    static STACK: Rc<dyn MeasurePolicy> = Rc::new(SimpleStack);
}

/// The stack policy (one for all: a node given it again isn't measured again).
pub(crate) fn stack_policy() -> Rc<dyn MeasurePolicy> {
    STACK.with(Rc::clone)
}

/// Compose's Popup SimpleStack: its children one over the other at its top left, as big as the biggest.
struct SimpleStack;

impl MeasurePolicy for SimpleStack {
    fn measure(&self, scope: &MeasureScope, measurables: &[&dyn Measurable], c: Constraints) -> MeasureResult {
        let placeables: Vec<Placeable> = measurables.iter().map(|m| m.measure(c)).collect();
        let width = placeables.iter().map(|p| p.width()).max().unwrap_or(0);
        let height = placeables.iter().map(|p| p.height()).max().unwrap_or(0);
        scope.layout(width, height, move |p| {
            for placeable in &placeables {
                p.place_relative(placeable, 0, 0);
            }
        })
    }
}
