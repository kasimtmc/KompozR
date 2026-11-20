//! Labels, as Compose's Label.kt: a label shown above a component (a slider's value) while it is pressed, dragged
//! or hovered — its interactions, not the label's own input —, or all the time when persistent; placed as a plain
//! tooltip is.

use crate::internal::basic_tooltip::{BasicTooltipBox, remember_basic_tooltip_state};
use crate::tooltip::{
    AnchorBounds, SpacingBetweenTooltipAndAnchor, TooltipAnchorPosition, TooltipDefaults, TooltipScope,
    TooltipScopeImpl, TooltipState,
};
use kompoz_animation::{MutableTransitionState, MutatePriority, MutatorMutex};
use kompoz_coroutines::LaunchedEffect;
use kompoz_foundation::gestures::LocalFuture;
use kompoz_foundation::interaction::{
    DragInteraction, HoverInteraction, Interaction, MutableInteractionSource, PressInteraction,
};
use kompoz_foundation::layout::Box;
use kompoz_runtime::{kompozable, remember};
use kompoz_ui::Modifier;
use std::rc::Rc;
use std::sync::Arc;

/// A label of `content` (Compose's `Label`): `label` (in a [`TooltipScope`]) shown above it — below where there is
/// no room — while `interaction_source` (one of its own where none) says it is pressed, dragged or hovered; all
/// the time when `is_persistent`. The label takes no input of its own and doesn't take the focus.
#[kompozable]
pub fn Label(
    label: impl Fn(&dyn TooltipScope) + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    #[default(false)] is_persistent: bool,
    content: impl Fn() + 'static,
) {
    let interaction_source = interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));
    // Has the same positioning logic as PlainTooltips
    let position_provider = TooltipDefaults::remember_tooltip_position_provider(
        TooltipAnchorPosition::Above,
        SpacingBetweenTooltipAndAnchor,
    );
    let state: Rc<dyn TooltipState> = if is_persistent {
        remember(|| Rc::new(LabelStateImpl::new(true, true)) as Rc<dyn TooltipState>)
    } else {
        remember_basic_tooltip_state(mutator_mutex = MutatorMutex())
    };

    let anchor_bounds = remember(AnchorBounds::new);
    let (bounds, provider) = (anchor_bounds.clone(), position_provider.clone());
    let scope = remember(move || Rc::new(TooltipScopeImpl::new(move || bounds.get(), provider)));

    let content = Rc::new(content);
    let wrapped_content = move || {
        let (anchor_bounds, content) = (anchor_bounds.clone(), content.clone());
        Box(
            modifier = Modifier.on_globally_positioned(move |it| anchor_bounds.set(Some(it.clone()))),
            content = move || content(),
        );
    };

    let label = Rc::new(label);
    BasicTooltipBox(
        position_provider = position_provider,
        tooltip = move || label(&*scope),
        state = state.clone(),
        modifier = modifier,
        focusable = false,
        enable_user_input = false,
        content = wrapped_content,
    );
    HandleInteractions(!is_persistent, state, interaction_source);
}

/// Shows the label while `interaction_source` says the component is pressed, dragged or hovered; its release,
/// stop or exit dismisses it (Compose's private `HandleInteractions`), each interaction taking over from the one
/// before.
#[kompozable]
fn HandleInteractions(enabled: bool, state: Rc<dyn TooltipState>, interaction_source: MutableInteractionSource) {
    if enabled {
        let interactions = interaction_source.interactions();
        LaunchedEffect(
            interaction_source,
            interactions.collect_latest(move |interaction: Arc<dyn Interaction>| {
                let state = state.clone();
                async move {
                    if interaction.is::<PressInteraction::Press>()
                        || interaction.is::<DragInteraction::Start>()
                        || interaction.is::<HoverInteraction::Enter>()
                    {
                        state.show(MutatePriority::UserInput).await;
                    } else if interaction.is::<PressInteraction::Release>()
                        || interaction.is::<DragInteraction::Stop>()
                        || interaction.is::<HoverInteraction::Exit>()
                    {
                        state.dismiss();
                    }
                }
            }),
        );
    }
}

/// A persistent label's state (Compose's private `LabelStateImpl`): always visible, and persistent; showing,
/// dismissing and disposing of it do nothing.
struct LabelStateImpl {
    is_visible: bool,
    is_persistent: bool,
    transition: MutableTransitionState<bool>,
}

impl LabelStateImpl {
    /// Compose's `LabelStateImpl(isVisible = true, isPersistent = true)`.
    fn new(is_visible: bool, is_persistent: bool) -> LabelStateImpl {
        LabelStateImpl { is_visible, is_persistent, transition: MutableTransitionState::new(false) }
    }
}

impl TooltipState for LabelStateImpl {
    fn transition(&self) -> MutableTransitionState<bool> {
        self.transition.clone()
    }

    fn is_visible(&self) -> bool {
        self.is_visible
    }

    fn is_persistent(&self) -> bool {
        self.is_persistent
    }

    fn show(&self, _mutate_priority: MutatePriority) -> LocalFuture<'static, ()> {
        std::boxed::Box::pin(async {})
    }

    fn dismiss(&self) {}

    fn on_dispose(&self) {}
}
