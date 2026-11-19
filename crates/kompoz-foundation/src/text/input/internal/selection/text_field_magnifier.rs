//! A field's magnifier (Compose's TextFieldMagnifier and Android's TextFieldMagnifierNodeImpl28): while a handle is
//! dragged, a loupe over the text around it — along a line at once, between lines by a spring.

use super::text_field_selection_state::TextFieldSelectionState;
use crate::magnifier::MagnifierNode;
use crate::text::input::internal::{TextLayoutState, TransformedTextFieldState};
use crate::text::long_press_text_drag_observer::Handle;
use crate::text::selection::container::{OffsetDisplacementThreshold, unspecified_safe_offset_vector_converter};
use crate::text::selection::visible_bounds;
use kompoz_animation::{Animatable, AnimationSpec, AnimationVector2D, Spring, spring};
use kompoz_coroutines::{CoroutineStart, Job, coroutine_scope, snapshot_flow};
use kompoz_graphics::{Density, DpSize, IntSize, Offset, Rect, Size};
use kompoz_runtime::{MutableState, mutable_state_of, named_args};
use kompoz_ui::modifier::DelegatedNode;
use kompoz_ui::{ModifierNode, ModifierNodeElement, NodeCx, NodeHandle};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

/// Where a dragged handle's magnifier centers, in the text layout node's coordinates (Compose's
/// `calculateSelectionMagnifierCenterAndroid`, before its conversion to the core node's): at the drag's x kept
/// within the handle's line, at the line's middle, within the visible text; unspecified when nothing is dragged,
/// the text is empty or not laid out, or the drag went further from the line than half the magnifier's width.
pub fn calculate_selection_magnifier_center_android(
    text_field_state: &TransformedTextFieldState,
    selection_state: &TextFieldSelectionState,
    text_layout_state: &TextLayoutState,
    magnifier_size: IntSize,
) -> Offset {
    let local_drag_position = selection_state.handle_drag_position();
    let visual = text_field_state.visual_text();
    if local_drag_position.is_unspecified() || visual.is_empty() {
        return Offset::Unspecified;
    }
    let selection = visual.selection();
    let text_offset = match selection_state.dragging_handle() {
        None => return Offset::Unspecified,
        Some(Handle::Cursor) | Some(Handle::SelectionStart) => selection.start,
        Some(Handle::SelectionEnd) => selection.end,
    };
    let Some(layout) = text_layout_state.layout_result() else { return Offset::Unspecified };
    let drag_x = local_drag_position.x;
    let line = layout.get_line_for_offset(text_offset);
    let (line_start, line_end) = (layout.get_line_left(line), layout.get_line_right(line));
    let center_x = drag_x.clamp(line_start.min(line_end), line_start.max(line_end));
    if magnifier_size != IntSize::Zero && (drag_x - center_x).abs() > (magnifier_size.width / 2) as f32 {
        return Offset::Unspecified;
    }
    let (top, bottom) = (layout.get_line_top(line), layout.get_line_bottom(line));
    let mut offset = Offset::new(center_x, (bottom - top) / 2.0 + top);
    if let Some(inner) = text_layout_state.text_layout_node_coordinates().filter(|c| c.is_attached()) {
        offset = crate::text::input::internal::text_layout_state::coerce_in(offset, visible_bounds(&inner));
    }
    offset
}

/// What the node and its coroutine share.
struct Shared {
    text_field_state: RefCell<TransformedTextFieldState>,
    selection_state: RefCell<Rc<TextFieldSelectionState>>,
    text_layout_state: RefCell<Rc<TextLayoutState>>,
    visible: Cell<bool>,
    magnifier_size: MutableState<IntSize>,
    animatable: Animatable<Offset, AnimationVector2D>,
    handle: RefCell<Option<NodeHandle>>,
    animation_job: RefCell<Option<Job>>,
}

impl Shared {
    fn target(&self) -> Offset {
        let (state, selection, layout) = (
            self.text_field_state.borrow().clone(),
            self.selection_state.borrow().clone(),
            self.text_layout_state.borrow().clone(),
        );
        calculate_selection_magnifier_center_android(&state, &selection, &layout, self.magnifier_size.get())
    }

    /// Compose's `restartAnimationJob`: a hidden magnifier snaps to none; a shown one follows the target.
    fn restart_animation_job(self: &Rc<Self>) {
        if let Some(job) = self.animation_job.borrow_mut().take() {
            job.cancel();
        }
        let Some(scope) = self.handle.borrow().as_ref().and_then(NodeHandle::coroutine_scope) else { return };
        if !self.visible.get() {
            if self.animatable.peek().is_specified() {
                let a = self.animatable.clone();
                named_args!(
                    scope.launch(
                        start = CoroutineStart::UNDISPATCHED,
                        async move { a.snap_to(Offset::Unspecified).await }
                    )
                );
            }
            return;
        }
        let shared = self.clone();
        let job = scope.launch(async move {
            coroutine_scope(move |animation_scope| async move {
                let (target, a) = (shared.clone(), shared.animatable.clone());
                snapshot_flow(move || target.target())
                    .collect_async(move |target_value: Offset| {
                        let (a, animation_scope) = (a.clone(), animation_scope.clone());
                        async move {
                            let current = a.peek();
                            // Along a line it follows the finger; between lines a spring eases it (a new one
                            // launched from the running one, keeping its velocity).
                            if current.is_specified() && target_value.is_specified() && current.y != target_value.y {
                                let spec: Arc<dyn AnimationSpec<Offset>> = Arc::new(named_args!(spring(
                                    Spring::DampingRatioNoBouncy,
                                    Spring::StiffnessMedium,
                                    Some(OffsetDisplacementThreshold),
                                )));
                                animation_scope.launch(async move {
                                    named_args!(a.animate_to(target_value, spec)).await;
                                });
                            } else {
                                a.snap_to(target_value).await;
                            }
                        }
                    })
                    .await;
            })
            .await;
        });
        *self.animation_job.borrow_mut() = Some(job);
    }
}

/// The node of a field's magnifier (Compose's TextFieldMagnifierNodeImpl28): it delegates to a magnifier whose
/// source is its animated center, in the decoration box's coordinates.
///
/// Compose delegates it from the core node, the loupe being the platform's window; KompozR draws the loupe in the
/// layout (see [`crate::magnifier`]), so the field puts this node on its decoration box, outside the core's clip,
/// and converts the center from the text layout to it. (Compose's `restartAnimationJob` also returns early when the
/// magnifier isn't visible; Compose launches the collection anyway after the snap, which collects nothing new.)
pub struct TextFieldMagnifierNode {
    shared: Rc<Shared>,
    magnifier: Option<DelegatedNode<MagnifierNode>>,
}

impl TextFieldMagnifierNode {
    fn new(
        text_field_state: TransformedTextFieldState,
        selection_state: Rc<TextFieldSelectionState>,
        text_layout_state: Rc<TextLayoutState>,
        visible: bool,
    ) -> TextFieldMagnifierNode {
        let magnifier_size = mutable_state_of(IntSize::Zero);
        let initial = calculate_selection_magnifier_center_android(
            &text_field_state,
            &selection_state,
            &text_layout_state,
            magnifier_size.peek(),
        );
        let animatable = Animatable::new(
            initial,
            unspecified_safe_offset_vector_converter(),
            Some(OffsetDisplacementThreshold),
            "Animatable",
        );
        TextFieldMagnifierNode {
            shared: Rc::new(Shared {
                text_field_state: RefCell::new(text_field_state),
                selection_state: RefCell::new(selection_state),
                text_layout_state: RefCell::new(text_layout_state),
                visible: Cell::new(visible),
                magnifier_size,
                animatable,
                handle: RefCell::new(None),
                animation_job: RefCell::new(None),
            }),
            magnifier: None,
        }
    }

    /// New states or visibility restart its animation (Compose's `update`).
    pub fn update(
        &mut self,
        text_field_state: TransformedTextFieldState,
        selection_state: Rc<TextFieldSelectionState>,
        text_layout_state: Rc<TextLayoutState>,
        visible: bool,
    ) {
        let s = &self.shared;
        let changed = *s.text_field_state.borrow() != text_field_state
            || !Rc::ptr_eq(&s.selection_state.borrow(), &selection_state)
            || !Rc::ptr_eq(&s.text_layout_state.borrow(), &text_layout_state)
            || s.visible.get() != visible;
        *s.text_field_state.borrow_mut() = text_field_state;
        *s.selection_state.borrow_mut() = selection_state;
        *s.text_layout_state.borrow_mut() = text_layout_state;
        s.visible.set(visible);
        if changed {
            s.restart_animation_job();
        }
    }
}

impl ModifierNode for TextFieldMagnifierNode {
    fn on_attach(&mut self, cx: &NodeCx) {
        *self.shared.handle.borrow_mut() = Some(cx.handle());
        if self.magnifier.is_none() {
            let (center, size) = (self.shared.clone(), self.shared.clone());
            let source_center = move |_: &Density| {
                let in_text = center.animatable.value();
                if in_text.is_unspecified() {
                    return Offset::Unspecified;
                }
                let layout = center.text_layout_state.borrow().clone();
                layout.from_text_layout_to_decoration(Rect::from_offset_size(in_text, Size::Zero)).top_left()
            };
            let on_size_changed: Rc<dyn Fn(DpSize)> = Rc::new(move |s: DpSize| {
                let density = size.handle.borrow().as_ref().map_or_else(Density::default, NodeHandle::density);
                size.magnifier_size.set(IntSize::new(density.round_to_px(s.width), density.round_to_px(s.height)));
            });
            self.magnifier = Some(cx.delegate(MagnifierNode::new(source_center, Some(on_size_changed))));
        }
        self.shared.restart_animation_job();
    }

    fn on_detach(&mut self, _cx: &NodeCx) {
        self.shared.animation_job.borrow_mut().take();
    }
}

/// The element of a field's magnifier node.
#[derive(Clone)]
pub(crate) struct TextFieldMagnifierElement {
    pub(crate) text_field_state: TransformedTextFieldState,
    pub(crate) selection_state: Rc<TextFieldSelectionState>,
    pub(crate) text_layout_state: Rc<TextLayoutState>,
    pub(crate) visible: bool,
}

impl PartialEq for TextFieldMagnifierElement {
    fn eq(&self, other: &Self) -> bool {
        self.text_field_state == other.text_field_state
            && Rc::ptr_eq(&self.selection_state, &other.selection_state)
            && Rc::ptr_eq(&self.text_layout_state, &other.text_layout_state)
            && self.visible == other.visible
    }
}

impl ModifierNodeElement for TextFieldMagnifierElement {
    type Node = TextFieldMagnifierNode;
    fn create(&self) -> TextFieldMagnifierNode {
        TextFieldMagnifierNode::new(
            self.text_field_state.clone(),
            self.selection_state.clone(),
            self.text_layout_state.clone(),
            self.visible,
        )
    }
    fn update(&self, node: &mut TextFieldMagnifierNode) {
        node.update(
            self.text_field_state.clone(),
            self.selection_state.clone(),
            self.text_layout_state.clone(),
            self.visible,
        );
    }
    fn name(&self) -> &'static str {
        "textFieldMagnifier"
    }
}
