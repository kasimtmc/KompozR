//! A lazy layout's semantics (Compose's LazyLayoutSemantics.kt): a traversal group with its scroll axis' range (a
//! pseudo offset: the sizes of the items out of view aren't known), its collection's size, the index of an item's
//! key, and scrolling to an index; the length of its viewport past its padding.

use super::LazyKey;
use super::item_provider::LazyLayoutItemProvider;
use super::lazy_layout::ItemProviderLambda;
use crate::gestures::{LocalFuture, Orientation};
use kompoz_ui::semantics::{CollectionInfo, ScrollAxisRange, SemanticsConfiguration, SemanticsModifierNode};
use kompoz_ui::{Modifier, ModifierNode, ModifierNodeElement, NodeCx, node_capabilities};
use std::rc::Rc;

/// What a lazy layout's semantics read of its state (Compose's LazyLayoutSemanticState).
pub(crate) trait LazyLayoutSemanticState {
    /// The viewport's length along the axis.
    fn viewport(&self) -> i32;
    /// The content padding's length along the axis, both ends.
    fn content_padding(&self) -> i32;
    fn scroll_offset(&self) -> f32;
    fn max_scroll_offset(&self) -> f32;
    fn collection_info(&self) -> CollectionInfo;
    /// Scrolls to item `index`.
    fn scroll_to_item(&self, index: i32) -> LocalFuture<'static, ()>;
}

/// A best-effort scroll offset for accessibility (Compose's `estimatedLazyScrollOffset`): 500 px an item, enough to
/// change with each scroll, small enough to stay exact in a float for realistic lists.
pub(crate) fn estimated_lazy_scroll_offset(
    first_visible_item_index: i32,
    first_visible_item_scroll_offset: i32,
) -> f32 {
    (first_visible_item_scroll_offset + first_visible_item_index * 500) as f32
}

/// The maximum to go with it (Compose's `estimatedLazyMaxScrollOffset`): a bit more while it can scroll on.
pub(crate) fn estimated_lazy_max_scroll_offset(
    first_visible_item_index: i32,
    first_visible_item_scroll_offset: i32,
    can_scroll_forward: bool,
) -> f32 {
    let offset = estimated_lazy_scroll_offset(first_visible_item_index, first_visible_item_scroll_offset);
    if can_scroll_forward { offset + 100.0 } else { offset }
}

/// A lazy layout's semantics (Compose's `lazyLayoutSemantics`).
pub(crate) fn lazy_layout_semantics(
    modifier: Modifier,
    item_provider_lambda: ItemProviderLambda,
    state: Rc<dyn LazyLayoutSemanticState>,
    orientation: Orientation,
    user_scroll_enabled: bool,
    reverse_scrolling: bool,
) -> Modifier {
    modifier.then_element(LazyLayoutSemanticsModifier {
        item_provider_lambda,
        state,
        orientation,
        user_scroll_enabled,
        reverse_scrolling,
    })
}

struct LazyLayoutSemanticsModifier {
    item_provider_lambda: ItemProviderLambda,
    state: Rc<dyn LazyLayoutSemanticState>,
    orientation: Orientation,
    user_scroll_enabled: bool,
    reverse_scrolling: bool,
}

impl PartialEq for LazyLayoutSemanticsModifier {
    fn eq(&self, o: &Self) -> bool {
        Rc::ptr_eq(&self.item_provider_lambda, &o.item_provider_lambda)
            && Rc::ptr_eq(&self.state, &o.state)
            && self.orientation == o.orientation
            && self.user_scroll_enabled == o.user_scroll_enabled
            && self.reverse_scrolling == o.reverse_scrolling
    }
}

impl ModifierNodeElement for LazyLayoutSemanticsModifier {
    type Node = LazyLayoutSemanticsModifierNode;
    fn create(&self) -> LazyLayoutSemanticsModifierNode {
        let mut node = LazyLayoutSemanticsModifierNode {
            item_provider_lambda: self.item_provider_lambda.clone(),
            state: self.state.clone(),
            orientation: self.orientation,
            user_scroll_enabled: self.user_scroll_enabled,
            reverse_scrolling: self.reverse_scrolling,
            scroll_axis_range: None,
            cx: None,
        };
        node.update_cached_semantics_values();
        node
    }
    fn update(&self, node: &mut LazyLayoutSemanticsModifierNode) {
        // (read when asked: no new semantics for them)
        node.item_provider_lambda = self.item_provider_lambda.clone();
        node.state = self.state.clone();
        let mut invalidate = false;
        if node.orientation != self.orientation {
            node.orientation = self.orientation;
            invalidate = true;
        }
        if node.user_scroll_enabled != self.user_scroll_enabled || node.reverse_scrolling != self.reverse_scrolling {
            node.user_scroll_enabled = self.user_scroll_enabled;
            node.reverse_scrolling = self.reverse_scrolling;
            node.update_cached_semantics_values();
            invalidate = true;
        }
        if invalidate && let Some(cx) = &node.cx {
            cx.invalidate_semantics();
        }
    }
    fn name(&self) -> &'static str {
        "lazyLayoutSemantics"
    }
}

/// Compose's LazyLayoutSemanticsModifierNode.
struct LazyLayoutSemanticsModifierNode {
    item_provider_lambda: ItemProviderLambda,
    state: Rc<dyn LazyLayoutSemanticState>,
    orientation: Orientation,
    user_scroll_enabled: bool,
    reverse_scrolling: bool,
    scroll_axis_range: Option<ScrollAxisRange>,
    cx: Option<NodeCx>,
}

impl ModifierNode for LazyLayoutSemanticsModifierNode {
    node_capabilities!(semantics);

    fn on_attach(&mut self, cx: &NodeCx) {
        self.cx = Some(cx.clone());
    }

    fn on_detach(&mut self, _: &NodeCx) {
        self.cx = None;
    }

    fn should_auto_invalidate(&self) -> bool {
        false
    }
}

impl LazyLayoutSemanticsModifierNode {
    fn update_cached_semantics_values(&mut self) {
        let (s1, s2) = (self.state.clone(), self.state.clone());
        self.scroll_axis_range = Some(ScrollAxisRange::new(
            move || s1.scroll_offset(),
            move || s2.max_scroll_offset(),
            self.reverse_scrolling,
        ));
    }
}

impl SemanticsModifierNode for LazyLayoutSemanticsModifierNode {
    fn apply_semantics(&mut self, config: &mut SemanticsConfiguration) {
        config.set_is_traversal_group(true);
        let items = self.item_provider_lambda.clone();
        config.index_for_key(move |needle| {
            let item_provider: Rc<dyn LazyLayoutItemProvider> = items();
            (0..item_provider.item_count())
                .find(|&index| key_matches(&item_provider.get_key(index), needle))
                .unwrap_or(-1)
        });
        if let Some(range) = self.scroll_axis_range.clone() {
            if self.orientation == Orientation::Vertical {
                config.set_vertical_scroll_axis_range(range);
            } else {
                config.set_horizontal_scroll_axis_range(range);
            }
        }
        if self.user_scroll_enabled {
            let items = self.item_provider_lambda.clone();
            let state = self.state.clone();
            let cx = self.cx.clone();
            config.scroll_to_index(move |index| {
                let item_provider = items();
                assert!(
                    index >= 0 && index < item_provider.item_count(),
                    "Can't scroll to index {index}, it is out of bounds [0, {})",
                    item_provider.item_count()
                );
                if let Some(cx) = &cx {
                    cx.coroutine_scope().launch(state.scroll_to_item(index));
                }
                true
            });
        }
        let state = self.state.clone();
        config.get_scroll_viewport_length(None, move || Some((state.viewport() - state.content_padding()) as f32));
        config.set_collection_info(self.state.collection_info());
    }
}

fn key_matches(key: &LazyKey, needle: &dyn std::any::Any) -> bool {
    key.matches(needle)
}
