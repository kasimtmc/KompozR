//! `SelectionContainer` and `DisableSelection` (Compose's SelectionContainer.kt, SimpleLayout.kt and the selection
//! magnifier of SelectionMagnifier.kt / SelectionManager.android.kt).

use super::handles::{OffsetProvider, SelectionHandle};
use super::manager::{SelectionManager, calculate_selection_magnifier_center};
use super::registrar::{LocalSelectionRegistrar, SelectionRegistrar, SelectionRegistrarImpl, SelectionRegistrarRef};
use super::selection::Selection;
use super::state::{SelectionState, remember_selection_state};
use crate::magnifier::MagnifierModifiers;
use crate::text::long_press_text_drag_observer::detect_down_and_drag_gestures_with_observer;
use kompoz_animation::{Animatable, AnimationSpec, AnimationVector2D, Spring, TwoWayConverter, spring};
use kompoz_coroutines::{LaunchedEffect, coroutine_scope, remember_coroutine_scope, snapshot_flow};
use kompoz_graphics::{Density, Dp, DpSize, IntSize, Offset};
use kompoz_runtime::{
    DisposableEffect, KompozitionLocalProvider, MutableState, State, derived_state_of, key, kompozable,
    mutable_state_of, on_dispose, remember, remember_keyed, remember_saveable_with,
};
use kompoz_text::AnnotatedString;
use kompoz_ui::input::PointerInputScope;
use kompoz_ui::layout::layout_into;
use kompoz_ui::platform::{LocalClipboard, LocalDensity, LocalHapticFeedback, LocalTextToolbar};
use kompoz_ui::{Constraints, Measurable, MeasurePolicy, MeasureResult, MeasureScope, Modifier};
use std::rc::Rc;
use std::sync::Arc;

/// Makes the text in `content` selectable, one selection across all of it (Compose's `SelectionContainer`): a
/// finger's long press selects a word, with handles to drag (from text to text) and the text toolbar (copy,
/// select all); a mouse's press and drag selects, its double click a word, its triple click a paragraph, shift
/// extends; Ctrl+C copies, Ctrl+A selects all, Escape clears; a press elsewhere in it clears it.
///
/// Texts of a lazy layout that aren't composed aren't selected, copied or selected by select all; selected ones
/// are kept composed.
#[kompozable]
pub fn SelectionContainer(#[default(Modifier)] modifier: Modifier, content: impl Fn() + 'static) {
    let state = remember_selection_state();
    SelectionContainerWithState(state = state, modifier = modifier, content = content);
}

/// A [`SelectionContainer`] whose selection `state` holds (Compose's `SelectionContainer(state, …)`): the app reads
/// the selected texts and selects all, clears, extends or selects a range through it. A state is one container's.
#[kompozable]
pub fn SelectionContainerWithState(
    state: SelectionState,
    #[default(Modifier)] modifier: Modifier,
    content: impl Fn() + 'static,
) {
    let selection = state.selection();
    let on_change = state.clone();
    selection_container_impl(
        modifier,
        Some(state),
        selection,
        Rc::new(move |s| on_change.set_selection(s)),
        Rc::new(content),
    );
}

/// A [`SelectionContainer`] whose selection the app holds (Compose's earlier `SelectionContainer(modifier,
/// selection, onSelectionChange, children)`): it shows `selection` and tells `on_selection_change` the selection
/// gestures come to (which should become the `selection` given next).
#[kompozable]
pub fn SelectionContainerWithSelection(
    selection: Option<Selection>,
    on_selection_change: impl Fn(Option<Selection>) + 'static,
    #[default(Modifier)] modifier: Modifier,
    content: impl Fn() + 'static,
) {
    selection_container_impl(modifier, None, selection, Rc::new(on_selection_change), Rc::new(content));
}

/// Makes the text in `content` not selectable in a selection container (Compose's `DisableSelection`): labels,
/// buttons.
#[kompozable(inline)]
pub fn DisableSelection(content: impl FnOnce()) {
    KompozitionLocalProvider(&[LocalSelectionRegistrar.provides(None)], content);
}

/// Compose's internal SelectionContainer: the registrar (saved), the manager, its clipboard, toolbar and
/// selection, the layout over the content with the handles, and its state bound while it lives.
#[track_caller]
fn selection_container_impl(
    modifier: Modifier,
    state: Option<SelectionState>,
    selection: Option<Selection>,
    on_selection_change: Rc<dyn Fn(Option<Selection>)>,
    content: Rc<dyn Fn()>,
) {
    kompoz_runtime::kompozable_block!({
        let registrar =
            remember_saveable_with(&SelectionRegistrarImpl::saver(), || Rc::new(SelectionRegistrarImpl::new()));
        let manager_registrar = registrar.clone();
        let manager = remember(move || SelectionManager::new(manager_registrar));
        let clipboard = LocalClipboard.current();
        let coroutine_scope = remember_coroutine_scope();
        manager.set_haptic_feedback(Some(LocalHapticFeedback.current()));
        let copy_handler = remember_keyed(clipboard.clone(), move || {
            Rc::new(move |text: AnnotatedString| clipboard.set_text(text.text())) as Rc<dyn Fn(AnnotatedString)>
        });
        manager.set_on_copy_handler(Some(copy_handler));
        manager.set_text_toolbar(Some(LocalTextToolbar.current()));
        manager.sync_selection(selection);
        let (weak, bound) = (Rc::downgrade(&manager), state.clone());
        manager.set_on_selection_change(move |new_selection| {
            on_selection_change(new_selection);
            if let (Some(state), Some(m)) = (&bound, weak.upgrade()) {
                state.update_selected_texts(m.get_selected_texts());
            }
        });
        manager.set_coroutine_scope(Some(coroutine_scope));

        let magnifier = selection_magnifier(&manager);
        let inner_manager = manager.clone();
        let inner_registrar = SelectionRegistrarRef(registrar.clone() as Rc<dyn SelectionRegistrar>);
        // The layout over what spans several texts: the top one, so `modifier` (a weight) is its own.
        SimpleLayout(modifier.then(manager.modifier()).then(magnifier), move || {
            let manager = inner_manager.clone();
            let content = content.clone();
            KompozitionLocalProvider(&[LocalSelectionRegistrar.provides(Some(inner_registrar.clone()))], move || {
                content();
                // The handles, while a finger's selection has the focus.
                if manager.is_in_touch_mode()
                    && manager.has_focus()
                    && !manager.is_trivially_collapsed_selection()
                    && let Some(selection) = manager.selection()
                {
                    for is_start_handle in [true, false] {
                        let manager = manager.clone();
                        key(&is_start_handle, move || selection_handle(&manager, &selection, is_start_handle));
                    }
                }
            });
        });

        let manager_key = Rc::as_ptr(&manager) as usize;
        if let Some(state) = state {
            let bound_manager = manager.clone();
            DisposableEffect((state.clone(), manager_key), move || {
                state.set_manager(Some(&bound_manager));
                on_dispose(move || {
                    if state.manager().is_some_and(|m| Rc::ptr_eq(&m, &bound_manager)) {
                        state.set_manager(None);
                        state.set_selection(None);
                    }
                })
            });
        }
        let released = manager.clone();
        DisposableEffect(manager_key, move || {
            on_dispose(move || {
                released.on_release();
                released.set_has_focus(false);
            })
        });
    })
}

/// One handle of the selection, dragged through the manager's observer.
#[track_caller]
fn selection_handle(manager: &Rc<SelectionManager>, selection: &Selection, is_start_handle: bool) {
    kompoz_runtime::kompozable_block!({
        let observer_manager = manager.clone();
        let observer = remember_keyed(is_start_handle, move || observer_manager.handle_drag_observer(is_start_handle));
        let provider_manager = Rc::downgrade(manager);
        let position_provider = remember_keyed(is_start_handle, move || {
            OffsetProvider::new(move || {
                let Some(m) = provider_manager.upgrade() else { return Offset::Unspecified };
                let p = if is_start_handle { m.start_handle_position() } else { m.end_handle_position() };
                p.unwrap_or(Offset::Unspecified)
            })
        });
        let direction = if is_start_handle { selection.start.direction } else { selection.end.direction };
        let line_height =
            if is_start_handle { manager.start_handle_line_height() } else { manager.end_handle_line_height() };
        let drag_observer = observer.clone();
        let drag_key = Rc::as_ptr(&observer) as *const () as usize;
        SelectionHandle(
            offset_provider = position_provider,
            is_start_handle = is_start_handle,
            direction = direction,
            handles_crossed = selection.handles_crossed,
            line_height = line_height,
            modifier = Modifier.pointer_input(drag_key, move |scope: PointerInputScope| {
                let observer = drag_observer.clone();
                async move { detect_down_and_drag_gestures_with_observer(scope, observer).await }
            }),
        );
    })
}

/// The magnifier of a dragged handle (Compose's `selectionMagnifier`, Android's, through its
/// `animatedSelectionMagnifier`): over the container while a finger drags a handle, its source where
/// [`calculate_selection_magnifier_center`] says — along the line at once, eased between lines by a spring —
/// hidden when dragged too far from the line (by half its size, told once shown).
#[track_caller]
fn selection_magnifier(manager: &Rc<SelectionManager>) -> Modifier {
    kompoz_runtime::kompozable_block!({
        let density = LocalDensity.current();
        let magnifier_size: MutableState<IntSize> = remember(|| mutable_state_of(IntSize::Zero));
        let (center_manager, size) = (Rc::downgrade(manager), magnifier_size.clone());
        let animated_center = remember_animated_magnifier_position(move || match center_manager.upgrade() {
            Some(m) => calculate_selection_magnifier_center(&m, size.get()),
            None => Offset::Unspecified,
        });
        if !manager.should_show_magnifier() {
            return Modifier;
        }
        let source_center = move |_: &Density| animated_center.get();
        let on_size_changed: Rc<dyn Fn(DpSize)> = Rc::new(move |s: DpSize| {
            magnifier_size.set(IntSize::new(density.round_to_px(s.width), density.round_to_px(s.height)));
        });
        Modifier.magnifier_with(
            source_center,
            None,
            Some(on_size_changed),
            f32::NAN,
            DpSize::Unspecified,
            Dp::Unspecified,
            Dp::Unspecified,
            true,
        )
    })
}

/// `Offset.VectorConverter` passing `Offset::Unspecified` on (Compose's UnspecifiedSafeOffsetVectorConverter).
fn unspecified_safe_offset_vector_converter() -> TwoWayConverter<Offset, AnimationVector2D> {
    TwoWayConverter(
        |offset: Offset| {
            if offset.is_specified() {
                AnimationVector2D { v1: offset.x, v2: offset.y }
            } else {
                AnimationVector2D { v1: f32::NAN, v2: f32::NAN }
            }
        },
        |v: AnimationVector2D| Offset::new(v.v1, v.v2),
    )
}

/// How close to its target the magnifier's spring ends (Compose's OffsetDisplacementThreshold).
const OffsetDisplacementThreshold: Offset =
    Offset::new(Spring::DefaultDisplacementThreshold, Spring::DefaultDisplacementThreshold);

/// The magnifier's position, following what `target_calculation` makes of the states it reads: at once along a
/// line (an animation would only lag behind the finger), by a spring to another line (a new one launched from the
/// running one's velocity, keeping its inertia), at once to or from none (Compose's
/// `rememberAnimatedMagnifierPosition`).
#[kompozable]
fn remember_animated_magnifier_position(target_calculation: impl Fn() -> Offset + 'static) -> State<Offset> {
    let target_value = remember(|| derived_state_of(target_calculation));
    let animatable = {
        let target_value = target_value.clone();
        remember(move || {
            Animatable::new(
                target_value.get(),
                unspecified_safe_offset_vector_converter(),
                Some(OffsetDisplacementThreshold),
                "Animatable",
            )
        })
    };
    let (target, a) = (target_value.clone(), animatable.clone());
    LaunchedEffect((), async move {
        coroutine_scope(move |animation_scope| async move {
            snapshot_flow(move || target.get())
                .collect(move |target_value: Offset| {
                    let a = a.clone();
                    let current = a.peek();
                    if current.is_specified() && target_value.is_specified() && current.y != target_value.y {
                        let spec: Arc<dyn AnimationSpec<Offset>> = Arc::new(spring(
                            Spring::DampingRatioNoBouncy,
                            Spring::StiffnessMedium,
                            Some(OffsetDisplacementThreshold),
                        ));
                        animation_scope.launch(async move {
                            kompoz_runtime::named_args!(a.animate_to(target_value, spec)).await;
                        });
                    } else {
                        // (no suspension: done as it is launched, in the collector's order)
                        animation_scope.launch(async move { a.snap_to(target_value).await });
                    }
                })
                .await;
        })
        .await;
    });
    animatable.as_state()
}

/// A layout passing its constraints to each child, at its top left, as big as the biggest (Compose's
/// SimpleLayout: selection takes no room of its own).
#[kompozable(inline)]
pub fn SimpleLayout(#[default(Modifier)] modifier: Modifier, content: impl FnOnce()) {
    let policy = remember(|| Rc::new(SimpleLayoutPolicy) as Rc<dyn MeasurePolicy>);
    layout_into(modifier, policy, false, content);
}

struct SimpleLayoutPolicy;

impl MeasurePolicy for SimpleLayoutPolicy {
    fn measure(
        &self,
        scope: &MeasureScope,
        measurables: &[&dyn Measurable],
        constraints: Constraints,
    ) -> MeasureResult {
        let placeables: Vec<_> = measurables.iter().map(|m| m.measure(constraints)).collect();
        let width = placeables.iter().map(|p| p.width()).max().unwrap_or(0);
        let height = placeables.iter().map(|p| p.height()).max().unwrap_or(0);
        scope.layout(width, height, move |p| {
            for placeable in &placeables {
                p.place(placeable, 0, 0);
            }
        })
    }
}
