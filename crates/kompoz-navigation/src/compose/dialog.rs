//! Dialog destinations (navigation-compose's `DialogNavigator` with its `Destination`, `DialogHost`): a dialog
//! destination is shown in a `Dialog` over the destination below it (a floating window: that one stays started),
//! dismissing it popping its entry.

use crate::destination::{NavDestination, NavDestinationKind};
use crate::entry::NavBackStackEntry;
use crate::navigator::{Navigator, NavigatorBase, NavigatorExtras};
use crate::options::NavOptions;
use kompoz_coroutines::{LaunchedEffect, StateFlow};
use kompoz_lifecycle::{LifecycleEvent, LifecycleObserver, LifecycleOwner, LifecycleState};
use kompoz_runtime::{
    DisposableEffect, SnapshotStateList, kompozable, mutable_confined_state_list_of, on_dispose, remember,
    remember_keyed, remember_saveable_state_holder,
};
use kompoz_ui::platform::LocalInspectionMode;
use kompoz_ui::window::{Dialog, DialogProperties};
use std::fmt;
use std::ops::Deref;
use std::rc::Rc;

/// The navigator of dialog destinations (Compose's `DialogNavigator`, named `"dialog"`): the NavHost's
/// [`DialogHost`] shows each of its entries in a `Dialog`.
#[derive(Default)]
pub struct DialogNavigator {
    base: NavigatorBase,
}

impl DialogNavigator {
    /// Its name (Kotlin's `DialogNavigator.NAME`).
    pub const NAME: &'static str = "dialog";

    /// Kotlin's `DialogNavigator()`.
    pub fn new() -> DialogNavigator {
        DialogNavigator::default()
    }

    /// Its back stack (Kotlin's internal `backStack`).
    pub(crate) fn back_stack(&self) -> StateFlow<Vec<NavBackStackEntry>> {
        self.state().back_stack()
    }

    /// Its entries in a transition (Kotlin's internal `transitionInProgress`).
    pub(crate) fn transition_in_progress(&self) -> StateFlow<Vec<NavBackStackEntry>> {
        self.state().transitions_in_progress()
    }

    /// Dismisses the dialog of `back_stack_entry`: pops it (Kotlin's internal `dismiss`).
    pub(crate) fn dismiss(&self, back_stack_entry: &NavBackStackEntry) {
        self.pop_back_stack_to(back_stack_entry, false);
    }

    /// `entry`'s dialog has left the composition (Kotlin's internal `onTransitionComplete`).
    pub(crate) fn on_transition_complete(&self, entry: &NavBackStackEntry) {
        self.state().mark_transition_complete(entry);
    }
}

impl Navigator for DialogNavigator {
    fn name(&self) -> String {
        DialogNavigator::NAME.into()
    }

    fn base(&self) -> &NavigatorBase {
        &self.base
    }

    fn create_destination(&self) -> NavDestination {
        DialogNavigatorDestination::new(self, DialogProperties::default(), |_: NavBackStackEntry| {}).into()
    }

    fn navigate(
        &self,
        entries: Vec<NavBackStackEntry>,
        _nav_options: Option<&NavOptions>,
        _navigator_extras: Option<&dyn NavigatorExtras>,
    ) {
        for entry in &entries {
            self.state().push(entry);
        }
    }

    fn pop_back_stack_to(&self, pop_up_to: &NavBackStackEntry, saved_state: bool) {
        self.state().pop_with_transition(pop_up_to, saved_state);
        let in_progress = self.state().transitions_in_progress().value();
        // (Kotlin's indexOf: -1 when it isn't there, every entry then after it)
        let pop_index = in_progress.iter().position(|e| e == pop_up_to).map_or(-1, |i| i as isize);
        for (index, entry) in in_progress.iter().enumerate() {
            if index as isize > pop_index {
                self.on_transition_complete(entry);
            }
        }
    }
}

/// What a dialog destination keeps (Kotlin's `DialogNavigator.Destination`'s fields).
pub(crate) struct DialogDestinationData {
    pub(crate) dialog_properties: DialogProperties,
    pub(crate) content: Rc<dyn Fn(NavBackStackEntry)>,
}

/// A dialog destination (Compose's `DialogNavigator.Destination`, a `FloatingWindow`): its content, shown in a
/// `Dialog` of `dialog_properties`. A [`NavDestination`] it derefs to.
///
/// Kotlin: `DialogNavigator.Destination`
#[derive(Clone)]
pub struct DialogNavigatorDestination(NavDestination);

impl DialogNavigatorDestination {
    /// A dialog destination of `navigator` (Kotlin's `Destination(navigator, dialogProperties, content)`).
    ///
    /// Kotlin: `DialogNavigator.Destination(navigator: DialogNavigator, dialogProperties: DialogProperties = DialogProperties(), content: @Composable (NavBackStackEntry) -> Unit)`
    pub fn new(
        navigator: &DialogNavigator,
        dialog_properties: DialogProperties,
        content: impl Fn(NavBackStackEntry) + 'static,
    ) -> DialogNavigatorDestination {
        let data = DialogDestinationData { dialog_properties, content: Rc::new(content) };
        DialogNavigatorDestination(NavDestination::of_kind(
            navigator.name(),
            NavDestinationKind {
                class_name: "Destination",
                floating_window: true,
                extra: Some(Rc::new(data)),
                ..Default::default()
            },
        ))
    }

    /// `destination` as a dialog destination, if it is one (Kotlin's `as? DialogNavigator.Destination`).
    pub fn from_destination(destination: &NavDestination) -> Option<DialogNavigatorDestination> {
        destination.extra::<DialogDestinationData>().map(|_| DialogNavigatorDestination(destination.clone()))
    }

    fn data(&self) -> Rc<DialogDestinationData> {
        self.0.extra::<DialogDestinationData>().expect("a dialog destination")
    }

    /// Its dialog's properties (Kotlin's internal `dialogProperties`).
    pub fn dialog_properties(&self) -> DialogProperties {
        self.data().dialog_properties
    }
}

impl Deref for DialogNavigatorDestination {
    type Target = NavDestination;
    fn deref(&self) -> &NavDestination {
        &self.0
    }
}

impl From<DialogNavigatorDestination> for NavDestination {
    fn from(d: DialogNavigatorDestination) -> NavDestination {
        d.0
    }
}

impl fmt::Debug for DialogNavigatorDestination {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

/// Shows each entry of `dialog_navigator`'s back stack that is at least started in a `Dialog` (Compose's
/// `DialogHost`; a NavHost shows its dialog destinations with it): dismissing one pops it; its content gets the
/// entry's owners ([`NavBackStackEntry::LocalOwnersProvider`]).
#[kompozable]
pub fn DialogHost(dialog_navigator: Rc<DialogNavigator>) {
    let saveable_state_holder = remember_saveable_state_holder();
    let dialog_back_stack = dialog_navigator.back_stack().collect_as_confined_state().get();
    let visible_back_stack = remember_visible_list(&dialog_back_stack);
    PopulateVisibleList(visible_back_stack.clone(), dialog_back_stack);
    let transition_in_progress = dialog_navigator.transition_in_progress().collect_as_confined_state().get();
    let dialogs_to_dispose = remember(|| mutable_confined_state_list_of(Vec::<NavBackStackEntry>::new()));
    for back_stack_entry in visible_back_stack.to_vec() {
        let destination = DialogNavigatorDestination::from_destination(&back_stack_entry.destination())
            .expect("a dialog destination");
        let (navigator, entry) = (dialog_navigator.clone(), back_stack_entry.clone());
        let (navigator2, holder, to_dispose) =
            (dialog_navigator.clone(), saveable_state_holder.clone(), dialogs_to_dispose.clone());
        let data = destination.data();
        let properties = data.dialog_properties;
        Dialog(
            on_dismiss_request = move || navigator.dismiss(&entry),
            properties = properties,
            content = move || {
                let (entry, navigator, to_dispose) = (back_stack_entry.clone(), navigator2.clone(), to_dispose.clone());
                DisposableEffect(back_stack_entry.clone(), move || {
                    to_dispose.add(entry.clone());
                    on_dispose(move || {
                        navigator.on_transition_complete(&entry);
                        to_dispose.retain(|e| e != &entry);
                    })
                });
                let content = data.content.clone();
                let e = back_stack_entry.clone();
                back_stack_entry.LocalOwnersProvider(&holder, move || content(e.clone()));
            },
        );
    }
    let navigator = dialog_navigator.clone();
    let to_dispose = dialogs_to_dispose.clone();
    LaunchedEffect((transition_in_progress.clone(), dialogs_to_dispose), async move {
        for entry in &transition_in_progress {
            let shown = to_dispose.with(|d| d.contains(entry));
            if !navigator.back_stack().value().contains(entry) && !shown {
                navigator.on_transition_complete(entry);
            }
        }
    });
}

/// Keeps `this` the entries of `back_stack` that are started (Compose's internal `PopulateVisibleList`): an entry
/// is added as it starts, removed as it stops (all of them while inspecting).
#[kompozable]
#[allow(non_snake_case)]
pub(crate) fn PopulateVisibleList(this: SnapshotStateList<NavBackStackEntry>, back_stack: Vec<NavBackStackEntry>) {
    let is_inspecting = LocalInspectionMode.current();
    for entry in back_stack {
        let list = this.clone();
        DisposableEffect(entry.lifecycle(), move || {
            let e = entry.clone();
            let observer: Rc<dyn LifecycleObserver> = Rc::new(move |_: &dyn LifecycleOwner, event: LifecycleEvent| {
                let contains = || list.with(|l| l.contains(&e));
                if is_inspecting && !contains() {
                    list.add(e.clone());
                }
                if event == LifecycleEvent::OnStart && !contains() {
                    list.add(e.clone());
                }
                if event == LifecycleEvent::OnStop {
                    list.retain(|x| x != &e);
                }
            });
            entry.lifecycle().add_observer(observer.clone());
            on_dispose(move || entry.lifecycle().remove_observer(observer))
        });
    }
}

/// The entries of `back_stack` that are started (all of them while inspecting), remembered while it is the same
/// (Compose's internal `rememberVisibleList`).
#[kompozable]
pub(crate) fn remember_visible_list(back_stack: &[NavBackStackEntry]) -> SnapshotStateList<NavBackStackEntry> {
    let is_inspecting = LocalInspectionMode.current();
    let stack = back_stack.to_vec();
    remember_keyed(stack.clone(), move || {
        mutable_confined_state_list_of(
            stack
                .into_iter()
                .filter(|e| is_inspecting || e.lifecycle().current_state().is_at_least(LifecycleState::Started)),
        )
    })
}
