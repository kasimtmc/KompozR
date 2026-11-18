//! `TooltipArea`, as Compose for desktop's TooltipArea.desktop.kt: a tooltip shown in a popup after the mouse rested
//! over the area for a while — by the cursor (following it) or by the area's bounds ([`TooltipPlacement`]) —, hidden
//! by a press, or once the mouse left both the area and the tooltip.

use crate::layout::Box;
use kompoz_coroutines::{Job, LaunchedEffect, delay_ms, remember_coroutine_scope, snapshot_flow};
use kompoz_graphics::{Dp, DpOffset, Offset};
use kompoz_runtime::{MutableState, kompozable, kompozable_block, mutable_state_of, remember};
use kompoz_ui::Modifier;
use kompoz_ui::alignment::Alignment;
use kompoz_ui::input::{PointerEvent, PointerEventPass, PointerEventType};
use kompoz_ui::layout::LayoutBoundsHolder;
use kompoz_ui::window::{
    Popup, PopupPositionProvider, remember_component_rect_position_provider,
    remember_popup_position_provider_at_position,
};
use std::cell::RefCell;
use std::rc::Rc;

/// Where a tooltip goes (Compose for desktop's `interface TooltipPlacement`): by the mouse cursor
/// ([`TooltipPlacement::cursor_point`]), by the area's bounds ([`TooltipPlacement::component_rect`]), or as one's
/// own composable function says ([`TooltipPlacement::new`]). Cheap to clone.
#[derive(Clone)]
pub struct TooltipPlacement {
    kind: Placement,
}

#[derive(Clone)]
enum Placement {
    CursorPoint { offset: DpOffset, alignment: Alignment, window_margin: Dp },
    ComponentRect { anchor: Alignment, alignment: Alignment, offset: DpOffset },
    Custom(Rc<dyn Fn(Offset) -> Rc<dyn PopupPositionProvider>>),
}

impl TooltipPlacement {
    /// At the mouse cursor, moved by `offset`, aligned to it as `alignment` says, kept `window_margin` within the
    /// window (Compose for desktop's `TooltipPlacement.CursorPoint`).
    /// Kotlin: `TooltipPlacement.CursorPoint(offset: DpOffset = DpOffset.Zero, alignment: Alignment = Alignment.BottomEnd, windowMargin: Dp = 4.dp)`
    pub fn cursor_point(offset: DpOffset, alignment: Alignment, window_margin: Dp) -> TooltipPlacement {
        TooltipPlacement { kind: Placement::CursorPoint { offset, alignment, window_margin } }
    }

    /// At the point of the area's bounds `anchor` gives, aligned to it as `alignment` says, moved by `offset`
    /// (Compose for desktop's `TooltipPlacement.ComponentRect`).
    /// Kotlin: `TooltipPlacement.ComponentRect(anchor: Alignment = Alignment.BottomCenter, alignment: Alignment = Alignment.BottomCenter, offset: DpOffset = DpOffset.Zero)`
    pub fn component_rect(anchor: Alignment, alignment: Alignment, offset: DpOffset) -> TooltipPlacement {
        TooltipPlacement { kind: Placement::ComponentRect { anchor, alignment, offset } }
    }

    /// One's own (Kotlin's `object : TooltipPlacement { @Composable override fun positionProvider(…) }`):
    /// `position_provider` is called in the composition, given the cursor's position in the area.
    pub fn new(position_provider: impl Fn(Offset) -> Rc<dyn PopupPositionProvider> + 'static) -> TooltipPlacement {
        TooltipPlacement { kind: Placement::Custom(Rc::new(position_provider)) }
    }

    /// The popup position provider for a cursor at `cursor_position` in the area (Compose's composable
    /// `positionProvider`).
    #[track_caller]
    pub fn position_provider(&self, cursor_position: Offset) -> Rc<dyn PopupPositionProvider> {
        let kind = self.kind.clone();
        kompozable_block!({
            match kind {
                Placement::CursorPoint { offset, alignment, window_margin } => {
                    remember_popup_position_provider_at_position(
                        position_px = cursor_position,
                        offset = offset,
                        alignment = alignment,
                        window_margin = window_margin,
                    )
                }
                Placement::ComponentRect { anchor, alignment, offset } => {
                    remember_component_rect_position_provider(anchor = anchor, alignment = alignment, offset = offset)
                }
                Placement::Custom(f) => f(cursor_position),
            }
        })
    }
}

impl Default for TooltipPlacement {
    /// TooltipArea's: 16 dp below the cursor.
    fn default() -> Self {
        TooltipPlacement::cursor_point(DpOffset::new(Dp(0.0), Dp(16.0)), Alignment::BottomEnd, Dp(4.0))
    }
}

/// What TooltipArea's handlers share (Compose's locals of the function, its `remember`ed states).
struct AreaState {
    cursor_position: MutableState<Offset>,
    is_visible: MutableState<bool>,
    show_tooltip_job: RefCell<Option<Job>>,
    tooltip_hiding_scheduled: MutableState<bool>,
    parent_bounds: LayoutBoundsHolder,
}

impl AreaState {
    fn start_showing(self: &Rc<Self>, scope: &kompoz_coroutines::CoroutineScope, delay_millis: i32) {
        // (the job isn't started again while it waits)
        if self.show_tooltip_job.borrow().as_ref().is_some_and(Job::is_active) {
            return;
        }
        let me = self.clone();
        let job = scope.launch(async move {
            delay_ms(delay_millis.max(0) as u64).await;
            me.is_visible.set(true);
        });
        *self.show_tooltip_job.borrow_mut() = Some(job);
    }

    fn hide(&self) {
        if let Some(job) = self.show_tooltip_job.borrow_mut().take() {
            job.cancel();
        }
        self.is_visible.set(false);
    }

    /// Hides it when the pointer, over the tooltip at `local_pointer_position`, is out of the area.
    fn hide_if_not_hovered(&self, local_bounds: &LayoutBoundsHolder, local_pointer_position: Offset) {
        if !self.is_visible.peek() {
            return;
        }
        let Some(offset_in_root) = local_bounds.bounds().map(|b| b.position_in_root()) else { return };
        let pointer_position_in_root = offset_in_root + local_pointer_position.round();
        let Some(parent_bounds_in_root) = self.parent_bounds.bounds().map(|b| b.bounds_in_root()) else { return };
        if !parent_bounds_in_root.contains(pointer_position_in_root) {
            self.hide();
        }
    }

    /// Hiding asked for (even when not showing: one may be waiting to show), done unless cancelled first — the
    /// pointer moving from the area into the tooltip, or back.
    fn schedule_cancelable_hiding(&self) {
        self.tooltip_hiding_scheduled.set(true);
    }

    fn cancel_scheduled_hiding(&self) {
        self.tooltip_hiding_scheduled.set(false);
    }
}

fn position(event: &PointerEvent) -> Offset {
    event.changes[0].position
}

/// Shows `tooltip` in a popup after the mouse rests over `content` for `delay_millis`, placed as
/// `tooltip_placement` says (by default 16 dp below the cursor, following it); a press hides it, and so does the
/// mouse leaving the area (unless it goes over the tooltip) (Compose for desktop's `TooltipArea`). `modifier` is
/// the area's.
#[kompozable]
pub fn TooltipArea(
    tooltip: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(500)] delay_millis: i32,
    #[default(TooltipPlacement::default())] tooltip_placement: TooltipPlacement,
    content: impl Fn() + 'static,
) {
    let state = remember(|| {
        Rc::new(AreaState {
            cursor_position: mutable_state_of(Offset::Zero),
            is_visible: mutable_state_of(false),
            show_tooltip_job: RefCell::new(None),
            tooltip_hiding_scheduled: mutable_state_of(false),
            parent_bounds: LayoutBoundsHolder::new(),
        })
    });
    let scope = remember_coroutine_scope();
    let hiding = state.clone();
    LaunchedEffect((), async move {
        let flag = hiding.tooltip_hiding_scheduled.clone();
        snapshot_flow(move || flag.get())
            .collect_latest(move |scheduled| {
                let hiding = hiding.clone();
                async move {
                    if scheduled {
                        hiding.hide();
                        hiding.tooltip_hiding_scheduled.set(false);
                    }
                }
            })
            .await;
    });
    let (enter, moved, exit, press) = (state.clone(), state.clone(), state.clone(), state.clone());
    let (enter_scope, move_scope) = (scope.clone(), scope.clone());
    let area_modifier = modifier
        .layout_bounds(state.parent_bounds.clone())
        .on_pointer_event(PointerEventType::Enter, PointerEventPass::Main, move |_, event: &PointerEvent| {
            enter.cancel_scheduled_hiding();
            enter.cursor_position.set(position(event));
            if !enter.is_visible.peek() && !event.buttons.are_any_pressed() {
                enter.start_showing(&enter_scope, delay_millis);
            }
        })
        .on_pointer_event(PointerEventType::Move, PointerEventPass::Main, move |_, event: &PointerEvent| {
            moved.cursor_position.set(position(event));
            if !moved.is_visible.peek() && !event.buttons.are_any_pressed() {
                moved.start_showing(&move_scope, delay_millis);
            }
        })
        .on_pointer_event(PointerEventType::Exit, PointerEventPass::Main, move |_, _: &PointerEvent| {
            exit.schedule_cancelable_hiding();
        })
        .on_pointer_event(PointerEventType::Press, PointerEventPass::Initial, move |_, _: &PointerEvent| {
            press.hide();
        });
    let tooltip = Rc::new(tooltip);
    Box(
        modifier = area_modifier,
        content = move || {
            content();
            if state.is_visible.get() {
                let provider = tooltip_placement.position_provider(state.cursor_position.get());
                let dismissed = state.clone();
                let (s, tooltip) = (state.clone(), tooltip.clone());
                Popup(
                    popup_position_provider = Some(provider),
                    on_dismiss_request = move || dismissed.is_visible.set(false),
                    content = move || {
                        let popup_layout_bounds = remember(LayoutBoundsHolder::new);
                        let (enter, moved, exit) = (s.clone(), s.clone(), s.clone());
                        let bounds = popup_layout_bounds.clone();
                        let tooltip = tooltip.clone();
                        Box(
                            modifier = Modifier
                                .layout_bounds(popup_layout_bounds.clone())
                                .on_pointer_event(
                                    PointerEventType::Enter,
                                    PointerEventPass::Main,
                                    move |_, _: &PointerEvent| enter.cancel_scheduled_hiding(),
                                )
                                .on_pointer_event(
                                    PointerEventType::Move,
                                    PointerEventPass::Main,
                                    // (the pointer could be moving out of the area while over the tooltip)
                                    move |_, event: &PointerEvent| moved.hide_if_not_hovered(&bounds, position(event)),
                                )
                                .on_pointer_event(
                                    PointerEventType::Exit,
                                    PointerEventPass::Main,
                                    move |_, _: &PointerEvent| exit.schedule_cancelable_hiding(),
                                ),
                            content = move || tooltip(),
                        );
                    },
                );
            }
        },
    );
}

/// The deprecated name of [`TooltipArea`] (Compose for desktop's `BoxWithTooltip`, "Use TooltipArea"): its
/// `content_alignment` and `propagate_min_constraints` aren't used (Compose's aren't either).
#[kompozable]
#[allow(unused_variables)]
pub fn BoxWithTooltip(
    tooltip: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(Alignment::TopStart)] content_alignment: Alignment,
    #[default(false)] propagate_min_constraints: bool,
    #[default(500)] delay: i32,
    #[default(TooltipPlacement::default())] tooltip_placement: TooltipPlacement,
    content: impl Fn() + 'static,
) {
    TooltipArea(
        tooltip = tooltip,
        modifier = modifier,
        delay_millis = delay,
        tooltip_placement = tooltip_placement,
        content = content,
    );
}
